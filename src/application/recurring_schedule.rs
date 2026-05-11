use crate::domain::DeviceId;

#[derive(Debug, Clone)]
pub struct RecurringSchedule {
    pub id: i64,
    pub device_id: DeviceId,
    pub start_time: String,
    pub end_time: String,
    pub enabled: bool,
    pub last_started_on: Option<String>,
    pub last_ended_on: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DueRecurringScheduleCommand {
    pub schedule_id: i64,
    pub device_id: DeviceId,
    pub command: RecurringScheduleCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecurringScheduleCommand {
    TurnOn,
    TurnOff,
}
