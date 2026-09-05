use crate::domain::DeviceId;

#[derive(Debug, Clone)]
pub struct RecurringCommand {
    pub id: i64,
    pub device_id: DeviceId,
    pub command: DeviceCommand,
    pub payload: serde_json::Value,
    pub local_time: String,
    pub enabled: bool,
    pub last_run_on: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DueRecurringCommand {
    pub id: i64,
    pub device_id: DeviceId,
    pub command: DeviceCommand,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceCommand {
    TurnOn,
    TurnOff,
    Open,
    Close,
    Stop,
    SetPosition,
}
