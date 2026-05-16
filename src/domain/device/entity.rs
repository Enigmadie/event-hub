use crate::domain::device::{availability::DeviceAvailability, id::DeviceId, name::DeviceName};

#[derive(Debug, Clone)]
pub struct Device {
    id: DeviceId,
    name: DeviceName,
    availability: DeviceAvailability,
}

impl Device {
    pub fn new(id: DeviceId, name: DeviceName) -> Self {
        Self {
            id,
            name,
            availability: DeviceAvailability::Unknown,
        }
    }

    pub fn id(&self) -> &DeviceId {
        &self.id
    }

    pub fn name(&self) -> &DeviceName {
        &self.name
    }

    pub fn availability(&self) -> DeviceAvailability {
        self.availability
    }

    pub fn set_availability(&mut self, availability: DeviceAvailability) {
        self.availability = availability;
    }
}
