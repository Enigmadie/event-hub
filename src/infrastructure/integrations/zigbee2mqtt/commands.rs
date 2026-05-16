use serde::Serialize;

#[derive(Serialize)]
pub struct SetStatePayload {
    pub state: &'static str,
}

#[derive(Serialize)]
pub struct SetPositionPayload {
    pub position: u8,
}

pub fn set_topic(device: &str) -> String {
    format!("zigbee2mqtt/{device}/set")
}
