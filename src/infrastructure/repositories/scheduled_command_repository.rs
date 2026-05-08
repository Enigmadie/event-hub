use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use sqlx::Row;

use crate::{
    application::{
        app_service::ScheduledCommandRepository,
        scheduled_command::{
            DueScheduledCommandJob, ScheduledCommand, ScheduledCommandJob, ScheduledCommandStatus,
        },
    },
    domain::DeviceId,
};

pub struct PostgresScheduledCommandRepository {
    pool: sqlx::PgPool,
    time_zone: String,
}

impl PostgresScheduledCommandRepository {
    pub fn new(pool: sqlx::PgPool, time_zone: String) -> Self {
        Self { pool, time_zone }
    }
}

#[async_trait]
impl ScheduledCommandRepository for PostgresScheduledCommandRepository {
    async fn create(
        &self,
        device_id: DeviceId,
        command: ScheduledCommand,
        run_at: String,
    ) -> Result<ScheduledCommandJob> {
        sqlx::query(
            r#"
            insert into devices (id, name, state, availability)
            values ($1, $1, 'OFF', 'Unknown')
            on conflict (id) do nothing
            "#,
        )
        .bind(device_id.as_str())
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to ensure device row for {}", device_id.as_str()))?;

        let row = sqlx::query(
            r#"
            insert into scheduled_commands (device_id, command, status, run_at)
            values ($1, $2, 'pending', ($3::timestamp at time zone $4))
            returning
                id,
                device_id,
                command,
                status,
                to_char(timezone($4, run_at), 'YYYY-MM-DD HH24:MI:SS') as run_at,
                last_error
            "#,
        )
        .bind(device_id.as_str())
        .bind(command_to_db(command))
        .bind(run_at)
        .bind(&self.time_zone)
        .fetch_one(&self.pool)
        .await
        .with_context(|| {
            format!(
                "failed to create scheduled command for {}",
                device_id.as_str()
            )
        })?;

        scheduled_command_from_row(row)
    }

    async fn list_for_device(&self, device_id: DeviceId) -> Result<Vec<ScheduledCommandJob>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                device_id,
                command,
                status,
                to_char(timezone($2, run_at), 'YYYY-MM-DD HH24:MI:SS') as run_at,
                last_error
            from scheduled_commands
            where device_id = $1
            order by run_at desc, id desc
            "#,
        )
        .bind(device_id.as_str())
        .bind(&self.time_zone)
        .fetch_all(&self.pool)
        .await
        .with_context(|| {
            format!(
                "failed to list scheduled commands for {}",
                device_id.as_str()
            )
        })?;

        rows.into_iter().map(scheduled_command_from_row).collect()
    }

    async fn claim_due(&self, limit: i64) -> Result<Vec<DueScheduledCommandJob>> {
        let rows = sqlx::query(
            r#"
            update scheduled_commands
            set
                status = 'running',
                updated_at = now()
            where id in (
                select id
                from scheduled_commands
                where status = 'pending' and run_at <= now()
                order by run_at, id
                limit $1
                for update skip locked
            )
            returning id, device_id, command
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("failed to claim due scheduled commands")?;

        rows.into_iter().map(due_command_from_row).collect()
    }

    async fn cancel(&self, id: i64) -> Result<()> {
        sqlx::query(
            r#"
            update scheduled_commands
            set status = 'cancelled', updated_at = now()
            where id = $1 and status = 'pending'
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to cancel scheduled command {id}"))?;

        Ok(())
    }

    async fn mark_succeeded(&self, id: i64) -> Result<()> {
        sqlx::query(
            r#"
            update scheduled_commands
            set status = 'succeeded', last_error = null, updated_at = now()
            where id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to mark scheduled command {id} succeeded"))?;

        Ok(())
    }

    async fn mark_failed(&self, id: i64, error: String) -> Result<()> {
        sqlx::query(
            r#"
            update scheduled_commands
            set status = 'failed', last_error = $2, updated_at = now()
            where id = $1
            "#,
        )
        .bind(id)
        .bind(error)
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to mark scheduled command {id} failed"))?;

        Ok(())
    }
}

fn scheduled_command_from_row(row: sqlx::postgres::PgRow) -> Result<ScheduledCommandJob> {
    let id: i64 = row.try_get("id")?;
    let device_id: String = row.try_get("device_id")?;
    let command: String = row.try_get("command")?;
    let status: String = row.try_get("status")?;
    let run_at: String = row.try_get("run_at")?;
    let last_error: Option<String> = row.try_get("last_error")?;

    Ok(ScheduledCommandJob {
        id,
        device_id: DeviceId::new(device_id),
        command: command_from_db(&command)?,
        status: status_from_db(&status)?,
        run_at,
        last_error,
    })
}

fn due_command_from_row(row: sqlx::postgres::PgRow) -> Result<DueScheduledCommandJob> {
    let id: i64 = row.try_get("id")?;
    let device_id: String = row.try_get("device_id")?;
    let command: String = row.try_get("command")?;

    Ok(DueScheduledCommandJob {
        id,
        device_id: DeviceId::new(device_id),
        command: command_from_db(&command)?,
    })
}

fn command_to_db(command: ScheduledCommand) -> &'static str {
    match command {
        ScheduledCommand::TurnOn => "turn_on",
        ScheduledCommand::TurnOff => "turn_off",
    }
}

fn command_from_db(value: &str) -> Result<ScheduledCommand> {
    match value {
        "turn_on" => Ok(ScheduledCommand::TurnOn),
        "turn_off" => Ok(ScheduledCommand::TurnOff),
        _ => bail!("unknown scheduled command {value}"),
    }
}

fn status_from_db(value: &str) -> Result<ScheduledCommandStatus> {
    match value {
        "pending" => Ok(ScheduledCommandStatus::Pending),
        "running" => Ok(ScheduledCommandStatus::Running),
        "succeeded" => Ok(ScheduledCommandStatus::Succeeded),
        "failed" => Ok(ScheduledCommandStatus::Failed),
        "cancelled" => Ok(ScheduledCommandStatus::Cancelled),
        _ => bail!("unknown scheduled command status {value}"),
    }
}
