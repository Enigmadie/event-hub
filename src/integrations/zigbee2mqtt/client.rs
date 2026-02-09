use super::commands::{SetStatePayload, set_topic};
use crate::transport::mqtt::client::MqttRuntime;

pub struct Z2mClient<'a> {
    mqtt: &'a mut MqttRuntime,
}

impl<'a> Z2mClient<'a> {
    pub fn new(mqtt: &'a mut MqttRuntime) -> Self {
        Self { mqtt }
    }

    pub fn turn_on(&mut self, device: &str) {
        let topic = set_topic(device);
        let payload = serde_json::to_vec(&SetStatePayload { state: "ON" }).unwrap();

        self.mqtt.publish(&topic, payload);
    }

    pub fn turn_off(&mut self, device: &str) {
        let topic = set_topic(device);
        let payload = serde_json::to_vec(&SetStatePayload { state: "OFF" }).unwrap();

        self.mqtt.publish(&topic, payload);
    }
}
