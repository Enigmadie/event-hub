use anyhow::{Context, Result, bail};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};

#[derive(Debug, Clone)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
}

pub async fn connect(config: &PostgresConfig) -> Result<sqlx::PgPool> {
    validate_database_name(&config.database)?;
    ensure_database(config).await?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options(config, &config.database))
        .await
        .context("failed to connect to Postgres database")?;

    migrate(&pool).await?;

    Ok(pool)
}

async fn ensure_database(config: &PostgresConfig) -> Result<()> {
    let maintenance_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(connect_options(config, "postgres"))
        .await
        .context("failed to connect to Postgres maintenance database")?;

    let exists: bool =
        sqlx::query_scalar("select exists(select 1 from pg_database where datname = $1)")
            .bind(&config.database)
            .fetch_one(&maintenance_pool)
            .await
            .context("failed to check Postgres database existence")?;

    if !exists {
        let database = quote_identifier(&config.database);
        sqlx::query(&format!("create database {database}"))
            .execute(&maintenance_pool)
            .await
            .with_context(|| format!("failed to create Postgres database {}", config.database))?;
    }

    Ok(())
}

async fn migrate(pool: &sqlx::PgPool) -> Result<()> {
    sqlx::query(
        r#"
        create table if not exists devices (
            id text primary key,
            name text not null,
            state text not null check (state in ('ON', 'OFF')),
            availability text not null check (availability in ('Unknown', 'Online', 'Offline')),
            last_seen_at timestamptz,
            last_state_changed_at timestamptz,
            created_at timestamptz not null default now(),
            updated_at timestamptz not null default now()
        )
        "#,
    )
    .execute(pool)
    .await
    .context("failed to migrate devices table")?;

    sqlx::query("alter table devices add column if not exists last_seen_at timestamptz")
        .execute(pool)
        .await
        .context("failed to migrate devices.last_seen_at")?;

    sqlx::query("alter table devices add column if not exists last_state_changed_at timestamptz")
        .execute(pool)
        .await
        .context("failed to migrate devices.last_state_changed_at")?;

    sqlx::query(
        r#"
        create table if not exists device_events (
            id bigserial primary key,
            device_id text not null references devices(id) on delete cascade,
            kind text not null check (kind in ('DeviceDiscovered', 'StateChanged', 'AvailabilityChanged')),
            state text check (state in ('ON', 'OFF')),
            availability text check (availability in ('Unknown', 'Online', 'Offline')),
            source_topic text not null,
            payload jsonb not null,
            occurred_at timestamptz not null default now(),
            created_at timestamptz not null default now()
        )
        "#,
    )
    .execute(pool)
    .await
    .context("failed to migrate device_events table")?;

    sqlx::query("alter table device_events drop constraint if exists device_events_kind_check")
        .execute(pool)
        .await
        .context("failed to drop old device_events kind check")?;

    sqlx::query(
        r#"
        alter table device_events
        add constraint device_events_kind_check
        check (kind in ('DeviceDiscovered', 'StateChanged', 'AvailabilityChanged'))
        "#,
    )
    .execute(pool)
    .await
    .context("failed to migrate device_events kind check")?;

    sqlx::query(
        "create index if not exists device_events_device_id_occurred_at_idx on device_events (device_id, occurred_at desc)",
    )
    .execute(pool)
    .await
    .context("failed to migrate device_events device/time index")?;

    sqlx::query(
        r#"
        create table if not exists scheduled_commands (
            id bigserial primary key,
            device_id text not null references devices(id) on delete cascade,
            command text not null check (command in ('turn_on', 'turn_off')),
            status text not null check (status in ('pending', 'running', 'succeeded', 'failed', 'cancelled')),
            run_at timestamptz not null,
            last_error text,
            created_at timestamptz not null default now(),
            updated_at timestamptz not null default now()
        )
        "#,
    )
    .execute(pool)
    .await
    .context("failed to migrate scheduled_commands table")?;

    sqlx::query(
        "create index if not exists scheduled_commands_due_idx on scheduled_commands (status, run_at, id)",
    )
    .execute(pool)
    .await
    .context("failed to migrate scheduled_commands due index")?;

    sqlx::query(
        r#"
        create table if not exists recurring_schedules (
            id bigserial primary key,
            device_id text not null references devices(id) on delete cascade,
            start_time time not null,
            end_time time not null,
            enabled boolean not null default true,
            last_started_on date,
            last_ended_on date,
            last_error text,
            created_at timestamptz not null default now(),
            updated_at timestamptz not null default now()
        )
        "#,
    )
    .execute(pool)
    .await
    .context("failed to migrate recurring_schedules table")?;

    sqlx::query(
        "create index if not exists recurring_schedules_enabled_idx on recurring_schedules (enabled, device_id, id)",
    )
    .execute(pool)
    .await
    .context("failed to migrate recurring_schedules enabled index")?;

    Ok(())
}

fn connect_options(config: &PostgresConfig, database: &str) -> PgConnectOptions {
    PgConnectOptions::new()
        .host(&config.host)
        .port(config.port)
        .username(&config.username)
        .password(&config.password)
        .database(database)
        .ssl_mode(PgSslMode::Prefer)
}

fn validate_database_name(database: &str) -> Result<()> {
    if database.is_empty() {
        bail!("DB_NAME must not be empty");
    }

    Ok(())
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
