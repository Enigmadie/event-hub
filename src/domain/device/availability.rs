use serde::Serialize;

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceAvailability {
    Unknown,
    Online,
    Offline,
}
