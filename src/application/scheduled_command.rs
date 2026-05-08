use crate::domain::DeviceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledCommand {
    TurnOn,
    TurnOff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledCommandStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct ScheduledCommandJob {
    pub id: i64,
    pub device_id: DeviceId,
    pub command: ScheduledCommand,
    pub status: ScheduledCommandStatus,
    pub run_at: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DueScheduledCommandJob {
    pub id: i64,
    pub device_id: DeviceId,
    pub command: ScheduledCommand,
}
