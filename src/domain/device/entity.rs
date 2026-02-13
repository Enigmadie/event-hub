use crate::domain::device::{id::DeviceId, name::DeviceName, state::DeviceState};

#[derive(Debug, Clone)]
pub struct Device {
    id: DeviceId,
    name: DeviceName,
    status: DeviceState,
}
