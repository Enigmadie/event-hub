use std::time::Duration;

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use sqlx::Row;

use crate::{
    application::{
        app_service::{DeviceRepository, DeviceSummary},
        device_event::DeviceReportedValue,
    },
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
    async fn list(&self) -> Result<Vec<DeviceSummary>> {
        let rows = sqlx::query(
            r#"
            select
                d.id,
                d.name,
                d.availability,
                coalesce(
                    jsonb_object_agg(v.property, v.value) filter (where v.property is not null),
                    '{}'::jsonb
                ) as latest_values
            from devices d
            left join device_latest_values v on v.device_id = d.id
            group by d.id, d.name, d.availability
            order by d.id
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to list devices")?;

        rows.into_iter().map(device_summary_from_row).collect()
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

    async fn update_latest_values(
        &self,
        id: DeviceId,
        values: Vec<DeviceReportedValue>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            insert into devices (id, name, state, availability, last_seen_at)
            values ($1, $1, 'OFF', 'Unknown', now())
            on conflict (id) do update set
                last_seen_at = now(),
                updated_at = now()
            "#,
        )
        .bind(id.as_str())
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to ensure device row for {}", id.as_str()))?;

        for value in values {
            sqlx::query(
                r#"
                insert into device_latest_values (device_id, property, value, updated_at)
                values ($1, $2, $3, now())
                on conflict (device_id, property) do update set
                    value = excluded.value,
                    updated_at = now()
                "#,
            )
            .bind(id.as_str())
            .bind(value.property)
            .bind(value.value)
            .execute(&self.pool)
            .await
            .with_context(|| format!("failed to update latest values for {}", id.as_str()))?;
        }

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

fn device_summary_from_row(row: sqlx::postgres::PgRow) -> Result<DeviceSummary> {
    let id: String = row.try_get("id")?;
    let name: String = row.try_get("name")?;
    let availability: String = row.try_get("availability")?;
    let latest_values: serde_json::Value = row.try_get("latest_values")?;

    let id = DeviceId::new(id);
    let mut device = Device::new(id.clone(), DeviceName::new(name));
    device.set_availability(
        availability_from_db(&availability)
            .with_context(|| format!("invalid availability for {}", id.as_str()))?,
    );

    let latest_values = latest_values.as_object().cloned().unwrap_or_default();

    Ok(DeviceSummary {
        device,
        latest_values,
    })
}

fn state_to_db(state: DeviceState) -> &'static str {
    match state {
        DeviceState::On => "ON",
        DeviceState::Off => "OFF",
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
