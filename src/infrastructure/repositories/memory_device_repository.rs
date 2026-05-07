use std::sync::RwLock;

use crate::{
    application::app_service::{DeviceRepository, discovered_device},
    domain::{Device, DeviceAvailability, DeviceId, DeviceName, DeviceState},
};

pub struct MemoryDeviceRepository {
    devices: RwLock<Vec<Device>>,
}

impl MemoryDeviceRepository {
    pub fn new(devices: Vec<Device>) -> Self {
        Self {
            devices: RwLock::new(devices),
        }
    }

    pub fn with_demo_devices() -> Self {
        Self::new(vec![Device::new(
            DeviceId::new("plug_plant".to_string()),
            DeviceName::new("Plant plug".to_string()),
            DeviceState::Off,
        )])
    }
}

impl DeviceRepository for MemoryDeviceRepository {
    fn list(&self) -> Vec<Device> {
        self.devices.read().expect("device store poisoned").clone()
    }

    fn update_state(&self, id: DeviceId, state: DeviceState) {
        let mut devices = self.devices.write().expect("device store poisoned");

        match devices.iter_mut().find(|device| device.id() == &id) {
            Some(device) => device.set_status(state),
            None => {
                let mut device = discovered_device(id);
                device.set_status(state);
                devices.push(device);
            }
        }
    }

    fn update_availability(&self, id: DeviceId, availability: DeviceAvailability) {
        let mut devices = self.devices.write().expect("device store poisoned");

        match devices.iter_mut().find(|device| device.id() == &id) {
            Some(device) => device.set_availability(availability),
            None => {
                let mut device = discovered_device(id);
                device.set_availability(availability);
                devices.push(device);
            }
        }
    }
}
