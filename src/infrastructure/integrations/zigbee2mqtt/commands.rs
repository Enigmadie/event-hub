use serde::Serialize;

#[derive(Serialize)]
pub struct SetStatePayload {
    pub state: &'static str,
}

pub fn set_topic(device: &str) -> String {
    format!("zigbee2mqtt/{device}/set")
}
