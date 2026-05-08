use std::{sync::Arc, time::Duration};

use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;

use crate::{
    application::device_event::{DeviceEvent, DeviceEventLogEntry, IncomingDeviceEvent},
    application::scheduled_command::{
        DueScheduledCommandJob, ScheduledCommand, ScheduledCommandJob,
    },
    domain::{Device, DeviceAvailability, DeviceId, DeviceName, DeviceState},
};

#[async_trait]
pub trait DeviceRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<Device>>;
    async fn upsert(&self, id: DeviceId, name: DeviceName) -> Result<()>;
    async fn update_state(&self, id: DeviceId, state: DeviceState) -> Result<()>;
    async fn update_availability(
        &self,
        id: DeviceId,
        availability: DeviceAvailability,
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

pub trait DeviceCommandGateway: Send + Sync {
    fn turn_on(&self, id: &DeviceId) -> Result<()>;
    fn turn_off(&self, id: &DeviceId) -> Result<()>;
}

pub struct AppService {
    repository: Arc<dyn DeviceRepository>,
    events: Arc<dyn DeviceEventRepository>,
    scheduled_commands: Arc<dyn ScheduledCommandRepository>,
    commands: Arc<dyn DeviceCommandGateway>,
}

impl AppService {
    pub fn new(
        repository: Arc<dyn DeviceRepository>,
        events: Arc<dyn DeviceEventRepository>,
        scheduled_commands: Arc<dyn ScheduledCommandRepository>,
        commands: Arc<dyn DeviceCommandGateway>,
    ) -> Self {
        Self {
            repository,
            events,
            scheduled_commands,
            commands,
        }
    }

    pub async fn list_devices(&self) -> Result<Vec<Device>> {
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

    pub fn turn_on(&self, id: &str) -> Result<()> {
        let id = DeviceId::new(id.to_string());
        self.commands.turn_on(&id)
    }

    pub fn turn_off(&self, id: &str) -> Result<()> {
        let id = DeviceId::new(id.to_string());
        self.commands.turn_off(&id)
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
}

pub fn discovered_device(id: DeviceId) -> Device {
    let name = DeviceName::new(id.as_str().to_string());
    Device::new(id, name, DeviceState::Off)
}
