use std::{sync::Arc, time::Duration};

use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;

use crate::{
    application::device_event::{
        DeviceEvent, DeviceEventLogEntry, DeviceReportedValue, IncomingDeviceEvent,
    },
    application::recurring_command::{DeviceCommand, DueRecurringCommand, RecurringCommand},
    application::recurring_schedule::{
        DueRecurringScheduleCommand, RecurringSchedule, RecurringScheduleCommand,
    },
    application::scheduled_command::{
        DueScheduledCommandJob, ScheduledCommand, ScheduledCommandJob,
    },
    domain::{Device, DeviceAvailability, DeviceId, DeviceName, DeviceState},
};

#[derive(Debug, Clone)]
pub struct DeviceSummary {
    pub device: Device,
    pub latest_values: serde_json::Map<String, serde_json::Value>,
}

#[async_trait]
pub trait DeviceRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<DeviceSummary>>;
    async fn upsert(&self, id: DeviceId, name: DeviceName) -> Result<()>;
    async fn update_state(&self, id: DeviceId, state: DeviceState) -> Result<()>;
    async fn update_availability(
        &self,
        id: DeviceId,
        availability: DeviceAvailability,
    ) -> Result<()>;
    async fn update_latest_values(
        &self,
        id: DeviceId,
        values: Vec<DeviceReportedValue>,
    ) -> Result<()>;
    async fn mark_stale_offline(&self, stale_after: Duration) -> Result<Vec<DeviceId>>;
}

#[async_trait]
pub trait DeviceEventRepository: Send + Sync {
    async fn append(&self, event: &IncomingDeviceEvent) -> Result<()>;
    async fn list_for_device(&self, id: DeviceId, limit: i64) -> Result<Vec<DeviceEventLogEntry>>;
}

#[async_trait]
pub trait ScheduledCommandRepository: Send + Sync {
    async fn create(
        &self,
        device_id: DeviceId,
        command: ScheduledCommand,
        run_at: String,
    ) -> Result<ScheduledCommandJob>;
    async fn list_for_device(&self, device_id: DeviceId) -> Result<Vec<ScheduledCommandJob>>;
    async fn claim_due(&self, limit: i64) -> Result<Vec<DueScheduledCommandJob>>;
    async fn cancel(&self, id: i64) -> Result<()>;
    async fn mark_succeeded(&self, id: i64) -> Result<()>;
    async fn mark_failed(&self, id: i64, error: String) -> Result<()>;
}

#[async_trait]
pub trait RecurringScheduleRepository: Send + Sync {
    async fn create(
        &self,
        device_id: DeviceId,
        start_time: String,
        end_time: String,
    ) -> Result<RecurringSchedule>;
    async fn list_for_device(&self, device_id: DeviceId) -> Result<Vec<RecurringSchedule>>;
    async fn claim_due(&self, limit: i64) -> Result<Vec<DueRecurringScheduleCommand>>;
    async fn set_enabled(&self, id: i64, enabled: bool) -> Result<()>;
    async fn mark_succeeded(&self, id: i64) -> Result<()>;
    async fn mark_failed(&self, id: i64, error: String) -> Result<()>;
}

#[async_trait]
pub trait RecurringCommandRepository: Send + Sync {
    async fn create(
        &self,
        device_id: DeviceId,
        command: DeviceCommand,
        payload: serde_json::Value,
        local_time: String,
    ) -> Result<RecurringCommand>;
    async fn list_for_device(&self, device_id: DeviceId) -> Result<Vec<RecurringCommand>>;
    async fn claim_due(&self, limit: i64) -> Result<Vec<DueRecurringCommand>>;
    async fn set_enabled(&self, id: i64, enabled: bool) -> Result<()>;
    async fn mark_succeeded(&self, id: i64) -> Result<()>;
    async fn mark_failed(&self, id: i64, error: String) -> Result<()>;
}

pub trait DeviceCommandGateway: Send + Sync {
    fn turn_on(&self, id: &DeviceId) -> Result<()>;
    fn turn_off(&self, id: &DeviceId) -> Result<()>;
    fn open_cover(&self, id: &DeviceId) -> Result<()>;
    fn close_cover(&self, id: &DeviceId) -> Result<()>;
    fn stop_cover(&self, id: &DeviceId) -> Result<()>;
    fn set_cover_position(&self, id: &DeviceId, position: u8) -> Result<()>;
}

pub struct AppService {
    repository: Arc<dyn DeviceRepository>,
    events: Arc<dyn DeviceEventRepository>,
    scheduled_commands: Arc<dyn ScheduledCommandRepository>,
    recurring_schedules: Arc<dyn RecurringScheduleRepository>,
    recurring_commands: Arc<dyn RecurringCommandRepository>,
    commands: Arc<dyn DeviceCommandGateway>,
}

impl AppService {
    pub fn new(
        repository: Arc<dyn DeviceRepository>,
        events: Arc<dyn DeviceEventRepository>,
        scheduled_commands: Arc<dyn ScheduledCommandRepository>,
        recurring_schedules: Arc<dyn RecurringScheduleRepository>,
        recurring_commands: Arc<dyn RecurringCommandRepository>,
        commands: Arc<dyn DeviceCommandGateway>,
    ) -> Self {
        Self {
            repository,
            events,
            scheduled_commands,
            recurring_schedules,
            recurring_commands,
            commands,
        }
    }

    pub async fn list_devices(&self) -> Result<Vec<DeviceSummary>> {
        self.repository.list().await
    }

    pub async fn list_device_events(
        &self,
        id: &str,
        limit: i64,
    ) -> Result<Vec<DeviceEventLogEntry>> {
        let limit = limit.clamp(1, 200);
        self.events
            .list_for_device(DeviceId::new(id.to_string()), limit)
            .await
    }

    pub async fn schedule_command(
        &self,
        device_id: &str,
        command: ScheduledCommand,
        run_at: String,
    ) -> Result<ScheduledCommandJob> {
        self.scheduled_commands
            .create(DeviceId::new(device_id.to_string()), command, run_at)
            .await
    }

    pub async fn list_scheduled_commands(
        &self,
        device_id: &str,
    ) -> Result<Vec<ScheduledCommandJob>> {
        self.scheduled_commands
            .list_for_device(DeviceId::new(device_id.to_string()))
            .await
    }

    pub async fn cancel_scheduled_command(&self, id: i64) -> Result<()> {
        self.scheduled_commands.cancel(id).await
    }

    pub async fn create_recurring_schedule(
        &self,
        device_id: &str,
        start_time: String,
        end_time: String,
    ) -> Result<RecurringSchedule> {
        self.recurring_schedules
            .create(DeviceId::new(device_id.to_string()), start_time, end_time)
            .await
    }

    pub async fn list_recurring_schedules(
        &self,
        device_id: &str,
    ) -> Result<Vec<RecurringSchedule>> {
        self.recurring_schedules
            .list_for_device(DeviceId::new(device_id.to_string()))
            .await
    }

    pub async fn set_recurring_schedule_enabled(&self, id: i64, enabled: bool) -> Result<()> {
        self.recurring_schedules.set_enabled(id, enabled).await
    }

    pub async fn create_recurring_command(
        &self,
        device_id: &str,
        command: DeviceCommand,
        payload: serde_json::Value,
        local_time: String,
    ) -> Result<RecurringCommand> {
        self.recurring_commands
            .create(
                DeviceId::new(device_id.to_string()),
                command,
                payload,
                local_time,
            )
            .await
    }

    pub async fn list_recurring_commands(&self, device_id: &str) -> Result<Vec<RecurringCommand>> {
        self.recurring_commands
            .list_for_device(DeviceId::new(device_id.to_string()))
            .await
    }

    pub async fn set_recurring_command_enabled(&self, id: i64, enabled: bool) -> Result<()> {
        self.recurring_commands.set_enabled(id, enabled).await
    }

    pub fn turn_on(&self, id: &str) -> Result<()> {
        let id = DeviceId::new(id.to_string());
        self.commands.turn_on(&id)
    }

    pub fn turn_off(&self, id: &str) -> Result<()> {
        let id = DeviceId::new(id.to_string());
        self.commands.turn_off(&id)
    }

    pub fn open_cover(&self, id: &str) -> Result<()> {
        let id = DeviceId::new(id.to_string());
        self.commands.open_cover(&id)
    }

    pub fn close_cover(&self, id: &str) -> Result<()> {
        let id = DeviceId::new(id.to_string());
        self.commands.close_cover(&id)
    }

    pub fn stop_cover(&self, id: &str) -> Result<()> {
        let id = DeviceId::new(id.to_string());
        self.commands.stop_cover(&id)
    }

    pub fn set_cover_position(&self, id: &str, position: u8) -> Result<()> {
        let id = DeviceId::new(id.to_string());
        self.commands.set_cover_position(&id, position)
    }

    pub async fn handle_device_event(&self, event: DeviceEvent) -> Result<()> {
        match event {
            DeviceEvent::DeviceDiscovered { device_id, name } => {
                self.repository
                    .upsert(device_id, DeviceName::new(name))
                    .await?;
            }
            DeviceEvent::StateChanged { device_id, state } => {
                self.repository.update_state(device_id, state).await?;
            }
            DeviceEvent::AvailabilityChanged {
                device_id,
                availability,
            } => {
                self.repository
                    .update_availability(device_id, availability)
                    .await?;
            }
            DeviceEvent::DeviceReported { device_id, values } => {
                self.repository
                    .update_latest_values(device_id, values)
                    .await?;
            }
        }

        Ok(())
    }

    pub async fn handle_incoming_device_event(&self, incoming: IncomingDeviceEvent) -> Result<()> {
        self.events.append(&incoming).await?;
        self.handle_device_event(incoming.event).await
    }

    pub async fn mark_stale_devices_offline(&self, stale_after: Duration) -> Result<usize> {
        let stale_devices = self.repository.mark_stale_offline(stale_after).await?;

        for device_id in &stale_devices {
            let event = IncomingDeviceEvent::new(
                DeviceEvent::AvailabilityChanged {
                    device_id: device_id.clone(),
                    availability: DeviceAvailability::Offline,
                },
                "event-hub/watchdog".to_string(),
                json!({
                    "reason": "stale",
                    "stale_after_secs": stale_after.as_secs(),
                }),
            );
            self.events.append(&event).await?;
        }

        Ok(stale_devices.len())
    }

    pub async fn run_due_scheduled_commands(&self, limit: i64) -> Result<usize> {
        let jobs = self
            .scheduled_commands
            .claim_due(limit.clamp(1, 100))
            .await?;
        let count = jobs.len();

        for job in jobs {
            let result = match job.command {
                ScheduledCommand::TurnOn => self.commands.turn_on(&job.device_id),
                ScheduledCommand::TurnOff => self.commands.turn_off(&job.device_id),
            };

            match result {
                Ok(()) => {
                    self.scheduled_commands.mark_succeeded(job.id).await?;
                }
                Err(error) => {
                    self.scheduled_commands
                        .mark_failed(job.id, format!("{error:#}"))
                        .await?;
                }
            }
        }

        Ok(count)
    }

    pub async fn run_due_recurring_schedules(&self, limit: i64) -> Result<usize> {
        let jobs = self
            .recurring_schedules
            .claim_due(limit.clamp(1, 100))
            .await?;
        let count = jobs.len();

        for job in jobs {
            let result = match job.command {
                RecurringScheduleCommand::TurnOn => self.commands.turn_on(&job.device_id),
                RecurringScheduleCommand::TurnOff => self.commands.turn_off(&job.device_id),
            };

            match result {
                Ok(()) => {
                    self.recurring_schedules
                        .mark_succeeded(job.schedule_id)
                        .await?;
                }
                Err(error) => {
                    self.recurring_schedules
                        .mark_failed(job.schedule_id, format!("{error:#}"))
                        .await?;
                }
            }
        }

        Ok(count)
    }

    pub async fn run_due_recurring_commands(&self, limit: i64) -> Result<usize> {
        let jobs = self
            .recurring_commands
            .claim_due(limit.clamp(1, 100))
            .await?;
        let count = jobs.len();

        for job in jobs {
            let result = self.run_device_command(&job.device_id, job.command, &job.payload);

            match result {
                Ok(()) => {
                    self.recurring_commands.mark_succeeded(job.id).await?;
                }
                Err(error) => {
                    self.recurring_commands
                        .mark_failed(job.id, format!("{error:#}"))
                        .await?;
                }
            }
        }

        Ok(count)
    }

    fn run_device_command(
        &self,
        device_id: &DeviceId,
        command: DeviceCommand,
        payload: &serde_json::Value,
    ) -> Result<()> {
        match command {
            DeviceCommand::TurnOn => self.commands.turn_on(device_id),
            DeviceCommand::TurnOff => self.commands.turn_off(device_id),
            DeviceCommand::Open => self.commands.open_cover(device_id),
            DeviceCommand::Close => self.commands.close_cover(device_id),
            DeviceCommand::Stop => self.commands.stop_cover(device_id),
            DeviceCommand::SetPosition => {
                let position = payload
                    .get("position")
                    .and_then(|value| value.as_u64())
                    .filter(|position| *position <= 100)
                    .ok_or_else(|| anyhow::anyhow!("set_position requires position 0..100"))?;
                self.commands.set_cover_position(device_id, position as u8)
            }
        }
    }
}

pub fn discovered_device(id: DeviceId) -> Device {
    let name = DeviceName::new(id.as_str().to_string());
    Device::new(id, name)
}
