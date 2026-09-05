use event_hub::{
    application::app_service::{AppService, DeviceCommandGateway},
    domain::DeviceId,
    infrastructure::{
        notifications::ChangeBroadcast,
        repositories::{
            device_event_repository::PostgresDeviceEventRepository,
            device_repository::PostgresDeviceRepository,
            recurring_command_repository::PostgresRecurringCommandRepository,
            recurring_schedule_repository::PostgresRecurringScheduleRepository,
            scheduled_command_repository::PostgresScheduledCommandRepository,
        },
    },
    observability::metrics::Metrics,
    presentation::http::state::{AppState, MqttHealth},
};
use std::{sync::Arc, time::Duration};

pub struct TestGateway {
    pub fail: bool,
}
impl TestGateway {
    fn send(&self) -> anyhow::Result<()> {
        if self.fail {
            anyhow::bail!("private gateway diagnostic")
        }
        Ok(())
    }
}
impl DeviceCommandGateway for TestGateway {
    fn turn_on(&self, _: &DeviceId) -> anyhow::Result<()> {
        self.send()
    }
    fn turn_off(&self, _: &DeviceId) -> anyhow::Result<()> {
        self.send()
    }
    fn open_cover(&self, _: &DeviceId) -> anyhow::Result<()> {
        self.send()
    }
    fn close_cover(&self, _: &DeviceId) -> anyhow::Result<()> {
        self.send()
    }
    fn stop_cover(&self, _: &DeviceId) -> anyhow::Result<()> {
        self.send()
    }
    fn set_cover_position(&self, _: &DeviceId, _: u8) -> anyhow::Result<()> {
        self.send()
    }
}

pub fn state(pool: sqlx::PgPool, fail_commands: bool) -> AppState {
    let changes = ChangeBroadcast::default();
    let time_zone = "Europe/Moscow";
    let service = AppService::new(
        Arc::new(PostgresDeviceRepository::new(pool.clone())),
        Arc::new(PostgresDeviceEventRepository::new(
            pool.clone(),
            time_zone.into(),
        )),
        Arc::new(PostgresScheduledCommandRepository::new(
            pool.clone(),
            time_zone.into(),
        )),
        Arc::new(PostgresRecurringScheduleRepository::new(
            pool.clone(),
            time_zone.into(),
        )),
        Arc::new(PostgresRecurringCommandRepository::new(
            pool,
            time_zone.into(),
        )),
        Arc::new(TestGateway {
            fail: fail_commands,
        }),
    )
    .with_change_publisher(Arc::new(changes.clone()));
    AppState {
        app_service: Arc::new(service),
        changes,
        time_zone: time_zone.into(),
        mqtt_health: MqttHealth::default(),
        metrics: Metrics::default(),
    }
}

#[allow(dead_code)]
pub fn lazy_state(fail_commands: bool) -> AppState {
    state(
        sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(10))
            .connect_lazy("postgres://test:test@127.0.0.1:1/test")
            .unwrap(),
        fail_commands,
    )
}
