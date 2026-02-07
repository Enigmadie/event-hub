use std::time::Duration;

use rumqttc::{Client, MqttOptions};

pub struct MqttConfig {
    pub client_id: String,
    pub host: String,
    pub port: u16,
}

pub struct MqttRuntime {
    pub client: rumqttc::Client,
    pub connection: rumqttc::Connection,
}

impl MqttRuntime {
    pub fn connect(config: MqttConfig) -> Self {
        let mut mqttoptions = MqttOptions::new(config.client_id, config.host, config.port);
        mqttoptions.set_keep_alive(Duration::from_secs(30));
        mqttoptions.set_max_packet_size(1024 * 1024, 1024 * 1024);

        let (client, connection) = Client::new(mqttoptions, 10);

        Self { client, connection }
    }

    pub fn publish(&mut self, topic: &str, payload: &[u8]) {
        self.client
            .publish(topic, rumqttc::QoS::AtLeastOnce, false, payload)
            .expect("Failed to publish MQTT message");
    }
}
