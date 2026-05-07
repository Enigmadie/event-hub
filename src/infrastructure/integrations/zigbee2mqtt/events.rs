use rumqttc::Publish;
use serde_json::Value;

use crate::{
    application::device_event::DeviceEvent,
    domain::{DeviceAvailability, DeviceId, DeviceState},
};

#[derive(Debug, PartialEq, Clone)]
pub enum Z2mEvent {
    DeviceState {
        device: String,
        state: DeviceState,
        raw: Value,
    },
    Availability {
        device: String,
        online: bool,
    },
}

impl Z2mEvent {
    pub fn into_device_event(self) -> DeviceEvent {
        match self {
            Self::DeviceState { device, state, .. } => DeviceEvent::StateChanged {
                device_id: DeviceId::new(device),
                state,
            },
            Self::Availability { device, online } => DeviceEvent::AvailabilityChanged {
                device_id: DeviceId::new(device),
                availability: if online {
                    DeviceAvailability::Online
                } else {
                    DeviceAvailability::Offline
                },
            },
        }
    }
}

pub fn parse(p: Publish) -> Option<(String, Z2mEvent)> {
    let topic = p.topic;
    let mut parts = topic.split('/');

    let prefix = parts.next()?;
    let device = parts.next()?.to_string();
    let sub = parts.next();

    if prefix != "zigbee2mqtt" {
        return None;
    }

    if device == "bridge" {
        // service messages
        return None;
    }

    match sub {
        None => {
            let json: Value = serde_json::from_slice(&p.payload).ok()?;
            let state = match json.get("state").and_then(|v| v.as_str())? {
                "ON" => DeviceState::On,
                "OFF" => DeviceState::Off,
                _ => return None,
            };

            Some((
                topic.clone(),
                Z2mEvent::DeviceState {
                    device: device.to_string(),
                    state,
                    raw: json,
                },
            ))
        }
        Some("availability") => {
            let s = std::str::from_utf8(&p.payload).ok()?.trim();
            Some((
                topic.clone(),
                Z2mEvent::Availability {
                    device: device.to_string(),
                    online: s == "online",
                },
            ))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use rumqttc::{Publish, QoS};
    use serde_json::json;

    use super::{Z2mEvent, parse};
    use crate::domain::DeviceState;

    #[test]
    fn parses_device_state() {
        let publish = Publish::new(
            "zigbee2mqtt/plug_plant",
            QoS::AtLeastOnce,
            r#"{"state":"ON"}"#,
        );

        let (_, event) = parse(publish).expect("expected device event");

        assert_eq!(
            event,
            Z2mEvent::DeviceState {
                device: "plug_plant".to_string(),
                state: DeviceState::On,
                raw: json!({ "state": "ON" }),
            }
        );
    }

    #[test]
    fn parses_availability() {
        let publish = Publish::new(
            "zigbee2mqtt/plug_plant/availability",
            QoS::AtLeastOnce,
            "online",
        );

        let (_, event) = parse(publish).expect("expected availability event");

        assert_eq!(
            event,
            Z2mEvent::Availability {
                device: "plug_plant".to_string(),
                online: true,
            }
        );
    }

    #[test]
    fn ignores_bridge_messages() {
        let publish = Publish::new("zigbee2mqtt/bridge/state", QoS::AtLeastOnce, "online");

        assert_eq!(parse(publish), None);
    }
}
