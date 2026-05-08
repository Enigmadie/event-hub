use std::time::Duration;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use sqlx::Row;

use crate::{
    application::app_service::DeviceRepository,
    domain::{Device, DeviceAvailability, DeviceId, DeviceName, DeviceState},
};

pub struct PostgresDeviceRepository {
    pool: sqlx::PgPool,
}

impl PostgresDeviceRepository {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DeviceRepository for PostgresDeviceRepository {
    async fn list(&self) -> Result<Vec<Device>> {
        let rows = sqlx::query("select id, name, state, availability from devices order by id")
            .fetch_all(&self.pool)
            .await
            .context("failed to list devices")?;

        rows.into_iter().map(device_from_row).collect()
    }

    async fn upsert(&self, id: DeviceId, name: DeviceName) -> Result<()> {
        sqlx::query(
            r#"
            insert into devices (id, name, state, availability)
            values ($1, $2, 'OFF', 'Unknown')
            on conflict (id) do update set
                name = excluded.name,
                updated_at = now()
            "#,
        )
        .bind(id.as_str())
        .bind(name.as_str())
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to upsert device {}", id.as_str()))?;

        Ok(())
    }

    async fn update_state(&self, id: DeviceId, state: DeviceState) -> Result<()> {
        sqlx::query(
            r#"
            insert into devices (
                id,
                name,
                state,
                availability,
                last_seen_at,
                last_state_changed_at
            )
            values ($1, $1, $2, 'Unknown', now(), now())
            on conflict (id) do update set
                state = excluded.state,
                last_seen_at = now(),
                last_state_changed_at = now(),
                updated_at = now()
            "#,
        )
        .bind(id.as_str())
        .bind(state_to_db(state))
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to update device state for {}", id.as_str()))?;

        Ok(())
    }

    async fn update_availability(
        &self,
        id: DeviceId,
        availability: DeviceAvailability,
    ) -> Result<()> {
        sqlx::query(
            r#"
            insert into devices (id, name, state, availability, last_seen_at)
            values ($1, $1, 'OFF', $2, now())
            on conflict (id) do update set
                availability = excluded.availability,
                last_seen_at = now(),
                updated_at = now()
            "#,
        )
        .bind(id.as_str())
        .bind(availability_to_db(availability))
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to update device availability for {}", id.as_str()))?;

        Ok(())
    }

    async fn mark_stale_offline(&self, stale_after: Duration) -> Result<Vec<DeviceId>> {
        let stale_after_secs = i64::try_from(stale_after.as_secs())
            .context("DEVICE_STALE_AFTER_SECS does not fit into i64")?;

        let rows = sqlx::query(
            r#"
            update devices
            set
                availability = 'Offline',
                updated_at = now()
            where
                last_seen_at is not null
                and last_seen_at < now() - ($1 * interval '1 second')
                and availability <> 'Offline'
            returning id
            "#,
        )
        .bind(stale_after_secs)
        .fetch_all(&self.pool)
        .await
        .context("failed to mark stale devices offline")?;

        rows.into_iter()
            .map(|row| {
                let id: String = row.try_get("id")?;
                Ok(DeviceId::new(id))
            })
            .collect()
    }
}

fn device_from_row(row: sqlx::postgres::PgRow) -> Result<Device> {
    let id: String = row.try_get("id")?;
    let name: String = row.try_get("name")?;
    let state: String = row.try_get("state")?;
    let availability: String = row.try_get("availability")?;

    let id = DeviceId::new(id);
    let mut device = Device::new(
        id.clone(),
        DeviceName::new(name),
        state_from_db(&state).with_context(|| format!("invalid state for {}", id.as_str()))?,
    );
    device.set_availability(
        availability_from_db(&availability)
            .with_context(|| format!("invalid availability for {}", id.as_str()))?,
    );

    Ok(device)
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
