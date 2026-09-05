use serde::Serialize;

/// Resource invalidation, not a durable event log or physical command acknowledgement.
#[derive(Debug, Clone, Serialize)]
pub struct HubChange {
    pub kind: ChangeKind,
    pub device_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    DevicesChanged,
    SchedulesChanged,
    CommandAccepted,
}

pub trait ChangePublisher: Send + Sync {
    fn publish(&self, change: HubChange);
}

pub struct NoopChangePublisher;
impl ChangePublisher for NoopChangePublisher {
    fn publish(&self, _change: HubChange) {}
}
