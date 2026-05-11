use anyhow::{Context, Result};
use async_trait::async_trait;
use sqlx::Row;

use crate::{
    application::{
        app_service::RecurringScheduleRepository,
        recurring_schedule::{
            DueRecurringScheduleCommand, RecurringSchedule, RecurringScheduleCommand,
        },
    },
    domain::DeviceId,
};

pub struct PostgresRecurringScheduleRepository {
    pool: sqlx::PgPool,
    time_zone: String,
}

impl PostgresRecurringScheduleRepository {
    pub fn new(pool: sqlx::PgPool, time_zone: String) -> Self {
        Self { pool, time_zone }
    }
}

#[async_trait]
impl RecurringScheduleRepository for PostgresRecurringScheduleRepository {
    async fn create(
        &self,
        device_id: DeviceId,
        start_time: String,
        end_time: String,
    ) -> Result<RecurringSchedule> {
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
            insert into recurring_schedules (device_id, start_time, end_time)
            values ($1, $2::time, $3::time)
            returning
                id,
                device_id,
                to_char(start_time, 'HH24:MI:SS') as start_time,
                to_char(end_time, 'HH24:MI:SS') as end_time,
                enabled,
                last_started_on::text as last_started_on,
                last_ended_on::text as last_ended_on,
                last_error
            "#,
        )
        .bind(device_id.as_str())
        .bind(start_time)
        .bind(end_time)
        .fetch_one(&self.pool)
        .await
        .with_context(|| {
            format!(
                "failed to create recurring schedule for {}",
                device_id.as_str()
            )
        })?;

        recurring_schedule_from_row(row)
    }

    async fn list_for_device(&self, device_id: DeviceId) -> Result<Vec<RecurringSchedule>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                device_id,
                to_char(start_time, 'HH24:MI:SS') as start_time,
                to_char(end_time, 'HH24:MI:SS') as end_time,
                enabled,
                last_started_on::text as last_started_on,
                last_ended_on::text as last_ended_on,
                last_error
            from recurring_schedules
            where device_id = $1
            order by start_time, end_time, id
            "#,
        )
        .bind(device_id.as_str())
        .fetch_all(&self.pool)
        .await
        .with_context(|| {
            format!(
                "failed to list recurring schedules for {}",
                device_id.as_str()
            )
        })?;

        rows.into_iter().map(recurring_schedule_from_row).collect()
    }

    async fn claim_due(&self, limit: i64) -> Result<Vec<DueRecurringScheduleCommand>> {
        let start_rows = sqlx::query(
            r#"
            with local_now as (
                select
                    timezone($2, now())::date as today,
                    timezone($2, now())::time as current_time
            ),
            due as (
                select id
                from recurring_schedules, local_now
                where enabled
                    and current_time >= start_time
                    and (start_time >= end_time or current_time < end_time)
                    and (last_started_on is null or last_started_on < today)
                order by start_time, id
                limit $1
                for update skip locked
            )
            update recurring_schedules
            set
                last_started_on = (select today from local_now),
                last_error = null,
                updated_at = now()
            where id in (select id from due)
            returning id, device_id, 'turn_on' as command
            "#,
        )
        .bind(limit)
        .bind(&self.time_zone)
        .fetch_all(&self.pool)
        .await
        .context("failed to claim due recurring schedule start commands")?;

        let remaining = limit - start_rows.len() as i64;
        let mut commands: Vec<DueRecurringScheduleCommand> = start_rows
            .into_iter()
            .map(due_command_from_row)
            .collect::<Result<_>>()?;

        if remaining <= 0 {
            return Ok(commands);
        }

        let end_rows = sqlx::query(
            r#"
            with local_now as (
                select
                    timezone($2, now())::date as today,
                    timezone($2, now())::time as current_time
            ),
            due as (
                select id
                from recurring_schedules, local_now
                where enabled
                    and current_time >= end_time
                    and (last_ended_on is null or last_ended_on < today)
                    and (
                        (start_time < end_time and last_started_on = today)
                        or
                        (start_time >= end_time and last_started_on = today - interval '1 day')
                    )
                order by end_time, id
                limit $1
                for update skip locked
            )
            update recurring_schedules
            set
                last_ended_on = (select today from local_now),
                last_error = null,
                updated_at = now()
            where id in (select id from due)
            returning id, device_id, 'turn_off' as command
            "#,
        )
        .bind(remaining)
        .bind(&self.time_zone)
        .fetch_all(&self.pool)
        .await
        .context("failed to claim due recurring schedule end commands")?;

        commands.extend(
            end_rows
                .into_iter()
                .map(due_command_from_row)
                .collect::<Result<Vec<_>>>()?,
        );

        Ok(commands)
    }

    async fn set_enabled(&self, id: i64, enabled: bool) -> Result<()> {
        sqlx::query(
            r#"
            update recurring_schedules
            set enabled = $2, updated_at = now()
            where id = $1
            "#,
        )
        .bind(id)
        .bind(enabled)
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to update recurring schedule {id}"))?;

        Ok(())
    }

    async fn mark_succeeded(&self, id: i64) -> Result<()> {
        sqlx::query(
            r#"
            update recurring_schedules
            set last_error = null, updated_at = now()
            where id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to mark recurring schedule {id} succeeded"))?;

        Ok(())
    }

    async fn mark_failed(&self, id: i64, error: String) -> Result<()> {
        sqlx::query(
            r#"
            update recurring_schedules
            set last_error = $2, updated_at = now()
            where id = $1
            "#,
        )
        .bind(id)
        .bind(error)
        .execute(&self.pool)
        .await
        .with_context(|| format!("failed to mark recurring schedule {id} failed"))?;

        Ok(())
    }
}

fn recurring_schedule_from_row(row: sqlx::postgres::PgRow) -> Result<RecurringSchedule> {
    let id: i64 = row.try_get("id")?;
    let device_id: String = row.try_get("device_id")?;
    let start_time: String = row.try_get("start_time")?;
    let end_time: String = row.try_get("end_time")?;
    let enabled: bool = row.try_get("enabled")?;
    let last_started_on: Option<String> = row.try_get("last_started_on")?;
    let last_ended_on: Option<String> = row.try_get("last_ended_on")?;
    let last_error: Option<String> = row.try_get("last_error")?;

    Ok(RecurringSchedule {
        id,
        device_id: DeviceId::new(device_id),
        start_time,
        end_time,
        enabled,
        last_started_on,
        last_ended_on,
        last_error,
    })
}

fn due_command_from_row(row: sqlx::postgres::PgRow) -> Result<DueRecurringScheduleCommand> {
    let id: i64 = row.try_get("id")?;
    let device_id: String = row.try_get("device_id")?;
    let command: String = row.try_get("command")?;

    Ok(DueRecurringScheduleCommand {
        schedule_id: id,
        device_id: DeviceId::new(device_id),
        command: match command.as_str() {
            "turn_on" => RecurringScheduleCommand::TurnOn,
            "turn_off" => RecurringScheduleCommand::TurnOff,
            _ => unreachable!("recurring schedule query returns known commands"),
        },
    })
}
