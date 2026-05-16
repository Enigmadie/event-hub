use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use sqlx::Row;

use crate::{
    application::{
        app_service::DeviceEventRepository,
        device_event::{DeviceEvent, DeviceEventKind, DeviceEventLogEntry, IncomingDeviceEvent},
    },
    domain::{DeviceAvailability, DeviceId, DeviceState},
};

pub struct PostgresDeviceEventRepository {
    pool: sqlx::PgPool,
    time_zone: String,
}

impl PostgresDeviceEventRepository {
    pub fn new(pool: sqlx::PgPool, time_zone: String) -> Self {
        Self { pool, time_zone }
    }
}

#[async_trait]
impl DeviceEventRepository for PostgresDeviceEventRepository {
    async fn append(&self, incoming: &IncomingDeviceEvent) -> Result<()> {
        let device_id = incoming.event.device_id().as_str();
        let (kind, state, availability) = event_columns(&incoming.event);

        sqlx::query(
            r#"
            insert into devices (id, name, state, availability)
            values ($1, $1, 'OFF', 'Unknown')
            on conflict (id) do nothing
            "#,
        )
        .bind(device_id)
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to ensure device row for {device_id}"))?;

        sqlx::query(
            r#"
            insert into device_events (
                device_id,
                kind,
                state,
                availability,
                source_topic,
                payload
            )
            values ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(device_id)
        .bind(kind_to_db(kind))
        .bind(state.map(state_to_db))
        .bind(availability.map(availability_to_db))
        .bind(&incoming.source_topic)
        .bind(&incoming.payload)
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to append device event for {device_id}"))?;

        Ok(())
    }

    async fn list_for_device(&self, id: DeviceId, limit: i64) -> Result<Vec<DeviceEventLogEntry>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                device_id,
                kind,
                state,
                availability,
                source_topic,
                payload,
                to_char(timezone($3, occurred_at), 'YYYY-MM-DD HH24:MI:SS') as occurred_at
            from device_events
            where device_id = $1
            order by occurred_at desc, id desc
            limit $2
            "#,
        )
        .bind(id.as_str())
        .bind(limit)
        .bind(&self.time_zone)
        .fetch_all(&self.pool)
        .await
        .with_context(|| format!("failed to list device events for {}", id.as_str()))?;

        rows.into_iter().map(event_from_row).collect()
    }
}

fn event_columns(
    event: &DeviceEvent,
) -> (
    DeviceEventKind,
    Option<DeviceState>,
    Option<DeviceAvailability>,
) {
    match event {
        DeviceEvent::DeviceDiscovered { .. } => (DeviceEventKind::DeviceDiscovered, None, None),
        DeviceEvent::StateChanged { state, .. } => {
            (DeviceEventKind::StateChanged, Some(*state), None)
        }
        DeviceEvent::AvailabilityChanged { availability, .. } => (
            DeviceEventKind::AvailabilityChanged,
            None,
            Some(*availability),
        ),
        DeviceEvent::DeviceReported { .. } => (DeviceEventKind::DeviceReported, None, None),
    }
}

fn event_from_row(row: sqlx::postgres::PgRow) -> Result<DeviceEventLogEntry> {
    let id: i64 = row.try_get("id")?;
    let device_id: String = row.try_get("device_id")?;
    let kind: String = row.try_get("kind")?;
    let state: Option<String> = row.try_get("state")?;
    let availability: Option<String> = row.try_get("availability")?;
    let source_topic: String = row.try_get("source_topic")?;
    let payload: serde_json::Value = row.try_get("payload")?;
    let occurred_at: String = row.try_get("occurred_at")?;

    Ok(DeviceEventLogEntry {
        id,
        device_id: DeviceId::new(device_id),
        kind: kind_from_db(&kind)?,
        name: payload
            .get("friendly_name")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        state: state.as_deref().map(state_from_db).transpose()?,
        availability: availability
            .as_deref()
            .map(availability_from_db)
            .transpose()?,
        values: if kind == "DeviceReported" {
            payload.as_object().cloned()
        } else {
            None
        },
        source_topic,
        payload,
        occurred_at,
    })
}

fn kind_to_db(kind: DeviceEventKind) -> &'static str {
    match kind {
        DeviceEventKind::DeviceDiscovered => "DeviceDiscovered",
        DeviceEventKind::StateChanged => "StateChanged",
        DeviceEventKind::AvailabilityChanged => "AvailabilityChanged",
        DeviceEventKind::DeviceReported => "DeviceReported",
    }
}

fn kind_from_db(value: &str) -> Result<DeviceEventKind> {
    match value {
        "DeviceDiscovered" => Ok(DeviceEventKind::DeviceDiscovered),
        "StateChanged" => Ok(DeviceEventKind::StateChanged),
        "AvailabilityChanged" => Ok(DeviceEventKind::AvailabilityChanged),
        "DeviceReported" => Ok(DeviceEventKind::DeviceReported),
        _ => bail!("unknown device event kind {value}"),
    }
}

fn state_to_db(state: DeviceState) -> &'static str {
    match state {
        DeviceState::On => "ON",
        DeviceState::Off => "OFF",
    }
}

fn state_from_db(value: &str) -> Result<DeviceState> {
    match value {
        "ON" => Ok(DeviceState::On),
        "OFF" => Ok(DeviceState::Off),
        _ => bail!("unknown device state {value}"),
    }
}

fn availability_to_db(availability: DeviceAvailability) -> &'static str {
    match availability {
        DeviceAvailability::Unknown => "Unknown",
        DeviceAvailability::Online => "Online",
        DeviceAvailability::Offline => "Offline",
    }
}

fn availability_from_db(value: &str) -> Result<DeviceAvailability> {
    match value {
        "Unknown" => Ok(DeviceAvailability::Unknown),
        "Online" => Ok(DeviceAvailability::Online),
        "Offline" => Ok(DeviceAvailability::Offline),
        _ => bail!("unknown device availability {value}"),
    }
}
