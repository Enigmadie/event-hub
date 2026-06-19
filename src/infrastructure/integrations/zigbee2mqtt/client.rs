use super::commands::{SetPositionPayload, SetStatePayload, set_topic};
use crate::{
    application::app_service::DeviceCommandGateway, domain::DeviceId,
    observability::metrics::Metrics,
};

pub struct Z2mClient {
    client: rumqttc::Client,
    metrics: Metrics,
}

impl Z2mClient {
    pub fn new(client: rumqttc::Client, metrics: Metrics) -> Self {
        Self { client, metrics }
    }

    fn set_state(&self, device: &DeviceId, state: &'static str) -> anyhow::Result<()> {
        let topic = set_topic(device.as_str());
        let payload = serde_json::to_vec(&SetStatePayload { state })?;

        let result = self
            .client
            .publish(topic, rumqttc::QoS::AtLeastOnce, false, payload);
        self.metrics.record_device_command_publish(result.is_ok());
        result?;
        Ok(())
    }

    fn set_position(&self, device: &DeviceId, position: u8) -> anyhow::Result<()> {
        let topic = set_topic(device.as_str());
        let payload = serde_json::to_vec(&SetPositionPayload { position })?;

        let result = self
            .client
            .publish(topic, rumqttc::QoS::AtLeastOnce, false, payload);
        self.metrics.record_device_command_publish(result.is_ok());
        result?;
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

    fn open_cover(&self, id: &DeviceId) -> anyhow::Result<()> {
        self.set_state(id, "OPEN")
    }

    fn close_cover(&self, id: &DeviceId) -> anyhow::Result<()> {
        self.set_state(id, "CLOSE")
    }

    fn stop_cover(&self, id: &DeviceId) -> anyhow::Result<()> {
        self.set_state(id, "STOP")
    }

    fn set_cover_position(&self, id: &DeviceId, position: u8) -> anyhow::Result<()> {
        self.set_position(id, position)
    }
}
