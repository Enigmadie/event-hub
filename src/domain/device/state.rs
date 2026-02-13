use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub enum DeviceState {
    #[serde(rename = "ON")]
    On,
    #[serde(rename = "OFF")]
    Off,
}
