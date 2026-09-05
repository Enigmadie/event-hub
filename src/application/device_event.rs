use crate::domain::{DeviceAvailability, DeviceId, DeviceState};

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceEvent {
    DeviceDiscovered {
        device_id: DeviceId,
        name: String,
        supported_commands: Option<Vec<super::recurring_command::DeviceCommand>>,
    },
    StateChanged {
        device_id: DeviceId,
        state: DeviceState,
    },
    AvailabilityChanged {
        device_id: DeviceId,
        availability: DeviceAvailability,
    },
    DeviceReported {
        device_id: DeviceId,
        values: Vec<DeviceReportedValue>,
    },
}

impl DeviceEvent {
    pub fn device_id(&self) -> &DeviceId {
        match self {
            Self::DeviceDiscovered { device_id, .. } => device_id,
            Self::StateChanged { device_id, .. } => device_id,
            Self::AvailabilityChanged { device_id, .. } => device_id,
            Self::DeviceReported { device_id, .. } => device_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceReportedValue {
    pub property: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceEventKind {
    DeviceDiscovered,
    StateChanged,
    AvailabilityChanged,
    DeviceReported,
}

#[derive(Debug, Clone)]
pub struct IncomingDeviceEvent {
    pub event: DeviceEvent,
    pub source_topic: String,
    pub payload: serde_json::Value,
}

impl IncomingDeviceEvent {
    pub fn new(event: DeviceEvent, source_topic: String, payload: serde_json::Value) -> Self {
        Self {
            event,
            source_topic,
            payload,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeviceEventLogEntry {
    pub id: i64,
    pub device_id: DeviceId,
    pub kind: DeviceEventKind,
    pub name: Option<String>,
    pub state: Option<DeviceState>,
    pub availability: Option<DeviceAvailability>,
    pub values: Option<serde_json::Map<String, serde_json::Value>>,
    pub source_topic: String,
    pub payload: serde_json::Value,
    pub occurred_at: String,
}
