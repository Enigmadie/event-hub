use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::http::{HeaderValue, Method};
use event_hub::{
    application::app_service::AppService,
    infrastructure::{
        db::postgres::{PostgresConfig, connect as connect_postgres},
        integrations::zigbee2mqtt::{
            client::Z2mClient, events::parse, subscriptions::subscriptions,
        },
        repositories::device_event_repository::PostgresDeviceEventRepository,
        repositories::device_repository::PostgresDeviceRepository,
        repositories::recurring_schedule_repository::PostgresRecurringScheduleRepository,
        repositories::scheduled_command_repository::PostgresScheduledCommandRepository,
        transport::mqtt::client::{MqttConfig, MqttRuntime},
        workers::{availability_watchdog, recurring_schedules, scheduled_commands},
    },
    presentation::http::{routes::create_router, state::AppState},
};
use rumqttc::{Event, Packet};
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
        postgres,
        app_time_zone.clone(),
    ));

    let mqtt = MqttRuntime::connect(MqttConfig {
        client_id: env("MQTT_CLIENT_ID", "event-hub"),
        host: env("MQTT_HOST", "127.0.0.1"),
        port: env("MQTT_PORT", "1883").parse()?,
    });

    for topic in subscriptions() {
        log::info!("subscribing to {topic}");
        mqtt.subscribe(&topic)?;
    }
    let commands = Arc::new(Z2mClient::new(mqtt.client.clone()));
    let app_service = Arc::new(AppService::new(
        repository,
        event_repository,
        scheduled_command_repository,
        recurring_schedule_repository,
        commands,
    ));
    let app = create_router(
        AppState {
            app_service: app_service.clone(),
        },
        cors_layer()?,
    );

    availability_watchdog::spawn(
        app_service.clone(),
        Duration::from_secs(env("DEVICE_STALE_AFTER_SECS", "300").parse()?),
        Duration::from_secs(env("DEVICE_WATCHDOG_INTERVAL_SECS", "60").parse()?),
    );
    scheduled_commands::spawn(
        app_service.clone(),
        Duration::from_secs(env("SCHEDULED_COMMAND_INTERVAL_SECS", "5").parse()?),
        env("SCHEDULED_COMMAND_BATCH_SIZE", "25").parse()?,
    );
    recurring_schedules::spawn(
        app_service.clone(),
        Duration::from_secs(env("RECURRING_SCHEDULE_INTERVAL_SECS", "5").parse()?),
        env("RECURRING_SCHEDULE_BATCH_SIZE", "25").parse()?,
    );

    let mut connection = mqtt.connection;
    let runtime = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || {
        for event in connection.iter() {
            match event {
                Ok(Event::Incoming(Packet::Publish(p))) => {
                    for (topic, event) in parse(p) {
                        log::info!("device event from {topic}: {event:?}");
                        let app_service = app_service.clone();
                        runtime.spawn(async move {
                            if let Err(error) = app_service
                                .handle_incoming_device_event(
                                    event.into_incoming_device_event(topic),
                                )
                                .await
                            {
                                log::error!("failed to handle device event: {error:#}");
                            }
                        });
                    }
                }
                Ok(_) => {}
                Err(error) => {
                    log::error!("MQTT connection error: {error:?}");
                    break;
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
