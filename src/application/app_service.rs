use crate::{domain::DeviceStore, infrastructure::transport::mqtt::client::MqttRuntime};

pub struct AppService {
    pub store: DeviceStore,
    pub mqtt: MqttRuntime,
}

impl AppService {
    pub fn list_devices(&self) -> Vec<String> {
        self.store.list()
    }

    pub async fn turn_on(&self, id: &str) {
        self.mqtt.turn_on(id).await;
    }

    pub async fn turn_off(&self, id: &str) {
        self.mqtt.turn_off(id).await;
    }
}
