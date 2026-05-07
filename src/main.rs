use std::{net::SocketAddr, sync::Arc};

use event_hub::{
    application::app_service::AppService,
    infrastructure::{
        integrations::zigbee2mqtt::{
            client::Z2mClient, events::parse, subscriptions::subscriptions,
        },
        repositories::memory_device_repository::MemoryDeviceRepository,
        transport::mqtt::client::{MqttConfig, MqttRuntime},
    },
    presentation::http::{routes::create_router, state::AppState},
};
use rumqttc::{Event, Packet};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let mqtt = MqttRuntime::connect(MqttConfig {
        client_id: env("MQTT_CLIENT_ID", "event-hub"),
        host: env("MQTT_HOST", "127.0.0.1"),
        port: env("MQTT_PORT", "1883").parse()?,
    });

    for topic in subscriptions() {
        log::info!("subscribing to {topic}");
        mqtt.subscribe(&topic)?;
    }

    let repository = Arc::new(MemoryDeviceRepository::with_demo_devices());
    let commands = Arc::new(Z2mClient::new(mqtt.client.clone()));
    let app_service = Arc::new(AppService::new(repository, commands));
    let app = create_router(AppState {
        app_service: app_service.clone(),
    });

    let mut connection = mqtt.connection;
    tokio::task::spawn_blocking(move || {
        for event in connection.iter() {
            match event {
                Ok(Event::Incoming(Packet::Publish(p))) => {
                    if let Some((topic, event)) = parse(p) {
                        log::info!("device event from {topic}: {event:?}");
                        app_service.handle_device_event(event.into_device_event());
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
