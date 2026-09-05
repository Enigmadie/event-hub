mod support;
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use event_hub::{
    application::{
        app_service::DeviceRepository, recurring_command::DeviceCommand,
        scheduled_command::ScheduledCommand,
    },
    domain::{DeviceId, DeviceName},
    infrastructure::{
        db::postgres::migrate, integrations::zigbee2mqtt::events::parse,
        repositories::device_repository::PostgresDeviceRepository,
    },
    presentation::http::routes::create_router,
};
use rumqttc::{Publish, QoS};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tower::ServiceExt;
use tower_http::cors::CorsLayer;

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to a disposable Postgres database"]
async fn capabilities_migrations_and_notifications_round_trip_through_postgres() {
    let url = std::env::var("TEST_DATABASE_URL").expect("set TEST_DATABASE_URL");
    let admin = sqlx::PgPool::connect(&url).await.unwrap();
    let schema = format!(
        "api_test_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    sqlx::query(&format!("create schema {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let search_path = Arc::new(format!("set search_path to {schema}"));
    let pool = sqlx::postgres::PgPoolOptions::new()
        .after_connect(move |connection, _| {
            let sql = search_path.clone();
            Box::pin(async move {
                sqlx::query(&sql).execute(connection).await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    let repository = PostgresDeviceRepository::new(pool.clone());
    repository
        .upsert(
            DeviceId::new("legacy".into()),
            DeviceName::new("Legacy".into()),
        )
        .await
        .unwrap();
    // Upgrading an existing database is idempotent and preserves undiscovered devices.
    migrate(&pool).await.unwrap();
    assert!(
        repository.list().await.unwrap()[0]
            .supported_commands
            .is_none()
    );
    let state = support::state(pool.clone(), false);
    let service = state.app_service.clone();
    let mut receiver = state.changes.subscribe();
    let discovery = Publish::new("zigbee2mqtt/bridge/devices", QoS::AtLeastOnce, json!([
        {"friendly_name":"plug","definition":{"exposes":[{"type":"switch","features":[{"type":"binary","property":"state","access":7,"value_on":"ON","value_off":"OFF"}]}]}},
        {"friendly_name":"sensor","definition":{"exposes":[{"type":"numeric","property":"temperature","access":1}]}}
    ]).to_string());
    for (topic, event) in parse(discovery) {
        service
            .handle_incoming_device_event(event.into_incoming_device_event(topic))
            .await
            .unwrap();
    }
    assert_eq!(
        serde_json::to_value(receiver.recv().await.unwrap()).unwrap()["kind"],
        "devices_changed"
    );
    let report = Publish::new(
        "zigbee2mqtt/plug",
        QoS::AtLeastOnce,
        r#"{"state":"ON","power":12}"#,
    );
    for (topic, event) in parse(report) {
        service
            .handle_incoming_device_event(event.into_incoming_device_event(topic))
            .await
            .unwrap();
    }
    // A fresh repository reads persisted capabilities; ordinary reports do not overwrite them.
    let devices = PostgresDeviceRepository::new(pool.clone())
        .list()
        .await
        .unwrap();
    assert_eq!(
        devices
            .iter()
            .find(|d| d.device.id().as_str() == "plug")
            .unwrap()
            .supported_commands,
        Some(vec![DeviceCommand::TurnOn, DeviceCommand::TurnOff])
    );
    let router = create_router(state, CorsLayer::new());
    let response = router
        .oneshot(Request::get("/devices").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let devices: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100_000).await.unwrap()).unwrap();
    let devices = devices.as_array().unwrap();
    let plug = devices.iter().find(|d| d["id"] == "plug").unwrap();
    assert_eq!(plug["supported_commands"], json!(["turn_on", "turn_off"]));
    assert_eq!(plug["values"]["power"], 12);
    assert_eq!(
        devices.iter().find(|d| d["id"] == "sensor").unwrap()["supported_commands"],
        json!([])
    );
    assert!(devices.iter().find(|d| d["id"] == "legacy").unwrap()["supported_commands"].is_null());
    let job = service
        .schedule_command(
            "plug",
            ScheduledCommand::TurnOff,
            "2000-01-01 10:30:00".into(),
        )
        .await
        .unwrap();
    assert_eq!(job.run_at, "2000-01-01 10:30:00");
    assert_eq!(service.run_due_scheduled_commands(10).await.unwrap(), 1);
    let status: String = sqlx::query_scalar("select status from scheduled_commands where id = $1")
        .bind(job.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "succeeded");
    let routine = service
        .create_recurring_schedule("plug", "09:00".into(), "22:00".into())
        .await
        .unwrap();
    service
        .set_recurring_schedule_enabled(routine.id, false)
        .await
        .unwrap();
    assert!(!service.list_recurring_schedules("plug").await.unwrap()[0].enabled);
    let command = service
        .create_recurring_command("plug", DeviceCommand::TurnOn, json!({}), "09:00".into())
        .await
        .unwrap();
    service
        .set_recurring_command_enabled(command.id, false)
        .await
        .unwrap();
    assert!(!service.list_recurring_commands("plug").await.unwrap()[0].enabled);
    let mut schedule_changes = 0;
    while let Ok(change) = receiver.try_recv() {
        if serde_json::to_value(change).unwrap()["kind"] == "schedules_changed" {
            schedule_changes += 1;
        }
    }
    assert!(schedule_changes >= 7);
    sqlx::query("update devices set availability = 'Online', last_seen_at = now() - interval '1 hour' where id = 'plug'").execute(&pool).await.unwrap();
    assert_eq!(
        service
            .mark_stale_devices_offline(Duration::from_secs(60))
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        serde_json::to_value(receiver.recv().await.unwrap()).unwrap(),
        json!({"kind":"devices_changed","device_id":"plug"})
    );
    pool.close().await;
    sqlx::query(&format!("drop schema {schema} cascade"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}
