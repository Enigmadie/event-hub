use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use sqlx::Row;

use crate::{
    application::{
        app_service::RecurringCommandRepository,
        recurring_command::{DeviceCommand, DueRecurringCommand, RecurringCommand},
    },
    domain::DeviceId,
};

pub struct PostgresRecurringCommandRepository {
    pool: sqlx::PgPool,
    time_zone: String,
}

impl PostgresRecurringCommandRepository {
    pub fn new(pool: sqlx::PgPool, time_zone: String) -> Self {
        Self { pool, time_zone }
    }
}

#[async_trait]
impl RecurringCommandRepository for PostgresRecurringCommandRepository {
    async fn create(
        &self,
        device_id: DeviceId,
        command: DeviceCommand,
        payload: serde_json::Value,
        local_time: String,
    ) -> Result<RecurringCommand> {
        ensure_device(&self.pool, &device_id).await?;

        let row = sqlx::query(
            r#"
            insert into recurring_device_commands (device_id, command, payload, local_time)
            values ($1, $2, $3, $4::time)
            returning
                id,
                device_id,
                command,
                payload,
                to_char(local_time, 'HH24:MI:SS') as local_time,
                enabled,
                last_run_on::text as last_run_on,
                last_error
            "#,
        )
        .bind(device_id.as_str())
        .bind(command_to_db(command))
        .bind(payload)
        .bind(local_time)
        .fetch_one(&self.pool)
        .await
        .with_context(|| {
            format!(
                "failed to create recurring command for {}",
                device_id.as_str()
            )
        })?;

        recurring_command_from_row(row)
    }

    async fn list_for_device(&self, device_id: DeviceId) -> Result<Vec<RecurringCommand>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                device_id,
                command,
                payload,
                to_char(local_time, 'HH24:MI:SS') as local_time,
                enabled,
                last_run_on::text as last_run_on,
                last_error
            from recurring_device_commands
            where device_id = $1
            order by local_time, id
            "#,
        )
        .bind(device_id.as_str())
        .fetch_all(&self.pool)
        .await
        .with_context(|| {
            format!(
                "failed to list recurring commands for {}",
                device_id.as_str()
            )
        })?;

        rows.into_iter().map(recurring_command_from_row).collect()
    }

    async fn claim_due(&self, limit: i64) -> Result<Vec<DueRecurringCommand>> {
        let rows = sqlx::query(
            r#"
            with local_now as (
                select
                    timezone($2, now())::date as today,
                    timezone($2, now())::time as current_time
            ),
            due as (
                select id
                from recurring_device_commands, local_now
                where enabled
                    and current_time >= local_time
                    and (last_run_on is null or last_run_on < today)
                order by local_time, id
                limit $1
                for update skip locked
            )
            update recurring_device_commands
            set
                last_run_on = (select today from local_now),
                last_error = null,
                updated_at = now()
            where id in (select id from due)
            returning id, device_id, command, payload
            "#,
        )
        .bind(limit)
        .bind(&self.time_zone)
        .fetch_all(&self.pool)
        .await
        .context("failed to claim due recurring device commands")?;

        rows.into_iter().map(due_command_from_row).collect()
    }

    async fn set_enabled(&self, id: i64, enabled: bool) -> Result<()> {
        sqlx::query(
            r#"
            update recurring_device_commands
            set enabled = $2, updated_at = now()
            where id = $1
            "#,
        )
        .bind(id)
        .bind(enabled)
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to update recurring command {id}"))?;

        Ok(())
    }

    async fn mark_succeeded(&self, id: i64) -> Result<()> {
        sqlx::query(
            r#"
            update recurring_device_commands
            set last_error = null, updated_at = now()
            where id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to mark recurring command {id} succeeded"))?;

        Ok(())
    }

    async fn mark_failed(&self, id: i64, error: String) -> Result<()> {
        sqlx::query(
            r#"
            update recurring_device_commands
            set last_error = $2, updated_at = now()
            where id = $1
            "#,
        )
        .bind(id)
        .bind(error)
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to mark recurring command {id} failed"))?;

        Ok(())
    }
}

async fn ensure_device(pool: &sqlx::PgPool, device_id: &DeviceId) -> Result<()> {
    sqlx::query(
        r#"
        insert into devices (id, name, state, availability)
        values ($1, $1, 'OFF', 'Unknown')
        on conflict (id) do nothing
        "#,
    )
    .bind(device_id.as_str())
    .execute(pool)
    .await
    .with_context(|| format!("failed to ensure device row for {}", device_id.as_str()))?;

    Ok(())
}

fn recurring_command_from_row(row: sqlx::postgres::PgRow) -> Result<RecurringCommand> {
    let id: i64 = row.try_get("id")?;
    let device_id: String = row.try_get("device_id")?;
    let command: String = row.try_get("command")?;
    let payload: serde_json::Value = row.try_get("payload")?;
    let local_time: String = row.try_get("local_time")?;
    let enabled: bool = row.try_get("enabled")?;
    let last_run_on: Option<String> = row.try_get("last_run_on")?;
    let last_error: Option<String> = row.try_get("last_error")?;

    Ok(RecurringCommand {
        id,
        device_id: DeviceId::new(device_id),
        command: command_from_db(&command)?,
        payload,
        local_time,
        enabled,
        last_run_on,
        last_error,
    })
}

fn due_command_from_row(row: sqlx::postgres::PgRow) -> Result<DueRecurringCommand> {
    let id: i64 = row.try_get("id")?;
    let device_id: String = row.try_get("device_id")?;
    let command: String = row.try_get("command")?;
    let payload: serde_json::Value = row.try_get("payload")?;

    Ok(DueRecurringCommand {
        id,
        device_id: DeviceId::new(device_id),
        command: command_from_db(&command)?,
        payload,
    })
}

pub fn command_to_db(command: DeviceCommand) -> &'static str {
    match command {
        DeviceCommand::TurnOn => "turn_on",
        DeviceCommand::TurnOff => "turn_off",
        DeviceCommand::Open => "open",
        DeviceCommand::Close => "close",
        DeviceCommand::Stop => "stop",
        DeviceCommand::SetPosition => "set_position",
    }
}

pub fn command_from_db(value: &str) -> Result<DeviceCommand> {
    match value {
        "turn_on" => Ok(DeviceCommand::TurnOn),
        "turn_off" => Ok(DeviceCommand::TurnOff),
        "open" => Ok(DeviceCommand::Open),
        "close" => Ok(DeviceCommand::Close),
        "stop" => Ok(DeviceCommand::Stop),
        "set_position" => Ok(DeviceCommand::SetPosition),
        _ => bail!("unknown recurring command {value}"),
    }
}
