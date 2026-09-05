use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::http::{HeaderValue, Method};
use event_hub::{
    application::app_service::AppService,
    infrastructure::{
        db::postgres::{PostgresConfig, connect as connect_postgres},
        integrations::zigbee2mqtt::{
            client::Z2mClient, events::parse, subscriptions::subscriptions,
        },
        notifications::ChangeBroadcast,
        repositories::device_event_repository::PostgresDeviceEventRepository,
        repositories::device_repository::PostgresDeviceRepository,
        repositories::recurring_command_repository::PostgresRecurringCommandRepository,
        repositories::recurring_schedule_repository::PostgresRecurringScheduleRepository,
        repositories::scheduled_command_repository::PostgresScheduledCommandRepository,
        transport::mqtt::client::{MqttConfig, MqttRuntime},
        workers::{
            availability_watchdog, recurring_commands, recurring_schedules, scheduled_commands,
        },
    },
    observability::metrics::Metrics,
    presentation::http::{
        routes::create_router,
        state::{AppState, MqttHealth},
    },
};
use rumqttc::{Event, Packet, QoS};
use tower_http::cors::{Any, CorsLayer};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();
    let app_time_zone = env("APP_TIME_ZONE", "Europe/Moscow");

    let postgres = connect_postgres(&PostgresConfig {
        host: env("DB_HOST", "127.0.0.1"),
        port: env("DB_PORT", "5432").parse()?,
        username: env("DB_USERNAME", "postgres"),
        password: env("DB_PASS", "postgres"),
        database: env("DB_NAME", "event_hub"),
    })
    .await?;
    let repository = Arc::new(PostgresDeviceRepository::new(postgres.clone()));
    let event_repository = Arc::new(PostgresDeviceEventRepository::new(
        postgres.clone(),
        app_time_zone.clone(),
    ));
    let scheduled_command_repository = Arc::new(PostgresScheduledCommandRepository::new(
        postgres.clone(),
        app_time_zone.clone(),
    ));
    let recurring_schedule_repository = Arc::new(PostgresRecurringScheduleRepository::new(
        postgres.clone(),
        app_time_zone.clone(),
    ));
    let recurring_command_repository = Arc::new(PostgresRecurringCommandRepository::new(
        postgres,
        app_time_zone.clone(),
    ));

    let mqtt = MqttRuntime::connect(MqttConfig {
        client_id: env("MQTT_CLIENT_ID", "event-hub"),
        host: env("MQTT_HOST", "127.0.0.1"),
        port: env("MQTT_PORT", "1883").parse()?,
    });

    // Subscriptions are (re)applied on every ConnAck inside the event loop below,
    // so they survive broker reconnects: the session is clean, so the broker drops
    // them on each new connection.
    let mqtt_health = MqttHealth::default();
    let metrics = Metrics::default();
    let commands = Arc::new(Z2mClient::new(mqtt.client.clone(), metrics.clone()));
    let changes = ChangeBroadcast::default();
    let app_service = Arc::new(
        AppService::new(
            repository,
            event_repository,
            scheduled_command_repository,
            recurring_schedule_repository,
            recurring_command_repository,
            commands,
        )
        .with_change_publisher(Arc::new(changes.clone())),
    );
    let app = create_router(
        AppState {
            time_zone: app_time_zone.into(),
            changes,
            app_service: app_service.clone(),
            mqtt_health: mqtt_health.clone(),
            metrics: metrics.clone(),
        },
        cors_layer()?,
    );

    availability_watchdog::spawn(
        app_service.clone(),
        Duration::from_secs(env("DEVICE_STALE_AFTER_SECS", "300").parse()?),
        Duration::from_secs(env("DEVICE_WATCHDOG_INTERVAL_SECS", "60").parse()?),
        metrics.clone(),
    );
    scheduled_commands::spawn(
        app_service.clone(),
        Duration::from_secs(env("SCHEDULED_COMMAND_INTERVAL_SECS", "5").parse()?),
        env("SCHEDULED_COMMAND_BATCH_SIZE", "25").parse()?,
        metrics.clone(),
    );
    recurring_schedules::spawn(
        app_service.clone(),
        Duration::from_secs(env("RECURRING_SCHEDULE_INTERVAL_SECS", "5").parse()?),
        env("RECURRING_SCHEDULE_BATCH_SIZE", "25").parse()?,
        metrics.clone(),
    );
    recurring_commands::spawn(
        app_service.clone(),
        Duration::from_secs(env("RECURRING_COMMAND_INTERVAL_SECS", "5").parse()?),
        env("RECURRING_COMMAND_BATCH_SIZE", "25").parse()?,
        metrics.clone(),
    );

    let mut connection = mqtt.connection;
    let resubscribe_client = mqtt.client.clone();
    let topics = subscriptions();
    let mqtt_health_loop = mqtt_health.clone();
    let metrics_loop = metrics.clone();
    let runtime = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || {
        // rumqttc's Connection keeps reconnecting as long as we keep polling it,
        // so on error we mark ourselves disconnected, back off, and continue
        // rather than breaking out (which would drop the client and permanently
        // wedge every publish with "Failed to send mqtt requests to eventloop").
        for event in connection.iter() {
            match event {
                Ok(Event::Incoming(Packet::ConnAck(_))) => {
                    mqtt_health_loop.set_connected(true);
                    metrics_loop.record_mqtt_connect();
                    for topic in &topics {
                        log::info!("subscribing to {topic}");
                        if let Err(error) = resubscribe_client.subscribe(topic, QoS::AtLeastOnce) {
                            log::error!("failed to subscribe to {topic}: {error:?}");
                        }
                    }
                }
                Ok(Event::Incoming(Packet::Publish(p))) => {
                    metrics_loop.record_mqtt_publish_message();
                    for (topic, event) in parse(p) {
                        metrics_loop.record_device_event();
                        log::info!("device event from {topic}: {event:?}");
                        let app_service = app_service.clone();
                        let metrics = metrics_loop.clone();
                        runtime.spawn(async move {
                            if let Err(error) = app_service
                                .handle_incoming_device_event(
                                    event.into_incoming_device_event(topic),
                                )
                                .await
                            {
                                metrics.record_device_event_error();
                                log::error!("failed to handle device event: {error:#}");
                            }
                        });
                    }
                }
                Ok(_) => {}
                Err(error) => {
                    log::error!("MQTT connection error: {error:?}");
                    mqtt_health_loop.set_connected(false);
                    metrics_loop.record_mqtt_connection_error();
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
    });

    let addr: SocketAddr = env("HTTP_ADDR", "127.0.0.1:3000").parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    log::info!("HTTP server listening on http://{addr}");
    axum::serve(listener, app).await?;

    Ok(())
}

fn env(name: &str, fallback: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| fallback.to_string())
}

fn cors_layer() -> anyhow::Result<CorsLayer> {
    let allowed_origins = env("HTTP_CORS_ALLOWED_ORIGINS", "http://localhost:5173");
    let origins = allowed_origins
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(HeaderValue::from_str)
        .collect::<Result<Vec<_>, _>>()?;

    let cors = if origins.is_empty() {
        CorsLayer::new().allow_origin(Any)
    } else {
        CorsLayer::new().allow_origin(origins)
    };

    Ok(cors
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers(Any))
}
