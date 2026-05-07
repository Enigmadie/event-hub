use super::commands::{SetStatePayload, set_topic};
use crate::{application::app_service::DeviceCommandGateway, domain::DeviceId};

pub struct Z2mClient {
    client: rumqttc::Client,
}

impl Z2mClient {
    pub fn new(client: rumqttc::Client) -> Self {
        Self { client }
    }

    fn set_state(&self, device: &DeviceId, state: &'static str) -> anyhow::Result<()> {
        let topic = set_topic(device.as_str());
        let payload = serde_json::to_vec(&SetStatePayload { state })?;

        self.client
            .publish(topic, rumqttc::QoS::AtLeastOnce, false, payload)?;
        Ok(())
    }
}

impl DeviceCommandGateway for Z2mClient {
    fn turn_on(&self, id: &DeviceId) -> anyhow::Result<()> {
        self.set_state(id, "ON")
    }

    fn turn_off(&self, id: &DeviceId) -> anyhow::Result<()> {
        self.set_state(id, "OFF")
    }
}
