use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::application::app_service::AppService;
use crate::observability::metrics::Metrics;

/// Tracks whether the MQTT event loop currently has a live broker connection.
/// Updated by the event loop (true on ConnAck, false on connection error) and
/// read by the `/health` readiness probe.
#[derive(Clone, Default)]
pub struct MqttHealth(Arc<AtomicBool>);

impl MqttHealth {
    pub fn set_connected(&self, connected: bool) {
        self.0.store(connected, Ordering::Relaxed);
    }

    pub fn is_connected(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[derive(Clone)]
pub struct AppState {
    pub app_service: Arc<AppService>,
    pub mqtt_health: MqttHealth,
    pub metrics: Metrics,
}
