use crate::application::changes::{ChangePublisher, HubChange};
use tokio::sync::broadcast;

/// Bounded, live notifications. Slow subscribers resync through the HTTP API.
#[derive(Clone)]
pub struct ChangeBroadcast(broadcast::Sender<HubChange>);

impl Default for ChangeBroadcast {
    fn default() -> Self {
        Self(broadcast::channel(256).0)
    }
}

impl ChangeBroadcast {
    pub fn subscribe(&self) -> broadcast::Receiver<HubChange> {
        self.0.subscribe()
    }
}

impl ChangePublisher for ChangeBroadcast {
    fn publish(&self, change: HubChange) {
        // Having no listeners must never fail a device command or persistence operation.
        let _ = self.0.send(change);
    }
}
