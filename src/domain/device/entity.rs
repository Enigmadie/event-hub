use crate::domain::device::{
    availability::DeviceAvailability, id::DeviceId, name::DeviceName, state::DeviceState,
};

#[derive(Debug, Clone)]
pub struct Device {
    id: DeviceId,
    name: DeviceName,
    status: DeviceState,
    availability: DeviceAvailability,
}

impl Device {
    pub fn new(id: DeviceId, name: DeviceName, status: DeviceState) -> Self {
        Self {
            id,
            name,
            status,
            availability: DeviceAvailability::Unknown,
        }
    }

    pub fn id(&self) -> &DeviceId {
        &self.id
    }

    pub fn name(&self) -> &DeviceName {
        &self.name
    }

    pub fn status(&self) -> DeviceState {
        self.status
    }

    pub fn availability(&self) -> DeviceAvailability {
        self.availability
    }

    pub fn set_status(&mut self, status: DeviceState) {
        self.status = status;
    }

    pub fn set_availability(&mut self, availability: DeviceAvailability) {
        self.availability = availability;
    }
}
