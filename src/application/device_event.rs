use crate::domain::{DeviceAvailability, DeviceId, DeviceState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceEvent {
    StateChanged {
        device_id: DeviceId,
        state: DeviceState,
    },
    AvailabilityChanged {
        device_id: DeviceId,
        availability: DeviceAvailability,
    },
}
