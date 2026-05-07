use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceState {
    #[serde(rename = "ON")]
    On,
    #[serde(rename = "OFF")]
    Off,
}
