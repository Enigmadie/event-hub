use std::sync::Arc;

use anyhow::Result;

use crate::{
    application::device_event::DeviceEvent,
    domain::{Device, DeviceAvailability, DeviceId, DeviceName, DeviceState},
};

pub trait DeviceRepository: Send + Sync {
    fn list(&self) -> Vec<Device>;
    fn update_state(&self, id: DeviceId, state: DeviceState);
    fn update_availability(&self, id: DeviceId, availability: DeviceAvailability);
}

pub trait DeviceCommandGateway: Send + Sync {
    fn turn_on(&self, id: &DeviceId) -> Result<()>;
    fn turn_off(&self, id: &DeviceId) -> Result<()>;
}

pub struct AppService {
    repository: Arc<dyn DeviceRepository>,
    commands: Arc<dyn DeviceCommandGateway>,
}

impl AppService {
    pub fn new(
        repository: Arc<dyn DeviceRepository>,
        commands: Arc<dyn DeviceCommandGateway>,
    ) -> Self {
        Self {
            repository,
            commands,
        }
    }

    pub fn list_devices(&self) -> Vec<Device> {
        self.repository.list()
    }

    pub fn turn_on(&self, id: &str) -> Result<()> {
        let id = DeviceId::new(id.to_string());
        self.commands.turn_on(&id)
    }

    pub fn turn_off(&self, id: &str) -> Result<()> {
        let id = DeviceId::new(id.to_string());
        self.commands.turn_off(&id)
    }

    pub fn handle_device_event(&self, event: DeviceEvent) {
        match event {
            DeviceEvent::StateChanged { device_id, state } => {
                self.repository.update_state(device_id, state);
            }
            DeviceEvent::AvailabilityChanged {
                device_id,
                availability,
            } => {
                self.repository.update_availability(device_id, availability);
            }
        }
    }
}

pub fn discovered_device(id: DeviceId) -> Device {
    let name = DeviceName::new(id.as_str().to_string());
    Device::new(id, name, DeviceState::Off)
}
