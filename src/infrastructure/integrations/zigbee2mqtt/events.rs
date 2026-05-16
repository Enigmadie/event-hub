use rumqttc::Publish;
use serde_json::Value;

use crate::{
    application::device_event::{DeviceEvent, DeviceReportedValue, IncomingDeviceEvent},
    domain::{DeviceAvailability, DeviceId, DeviceState},
};

#[derive(Debug, PartialEq, Clone)]
pub enum Z2mEvent {
    DeviceDiscovered {
        device: String,
        name: String,
        raw: Value,
    },
    DeviceState {
        device: String,
        state: DeviceState,
        raw: Value,
    },
    Availability {
        device: String,
        online: bool,
    },
    DeviceReport {
        device: String,
        values: Vec<DeviceReportedValue>,
        raw: Value,
    },
}

impl Z2mEvent {
    pub fn into_device_event(self) -> DeviceEvent {
        match self {
            Self::DeviceDiscovered { device, name, .. } => DeviceEvent::DeviceDiscovered {
                device_id: DeviceId::new(device),
                name,
            },
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
            Self::DeviceReport { device, values, .. } => DeviceEvent::DeviceReported {
                device_id: DeviceId::new(device),
                values,
            },
        }
    }

    pub fn into_incoming_device_event(self, source_topic: String) -> IncomingDeviceEvent {
        let payload = self.payload();
        IncomingDeviceEvent::new(self.into_device_event(), source_topic, payload)
    }

    fn payload(&self) -> Value {
        match self {
            Self::DeviceDiscovered { raw, .. } => raw.clone(),
            Self::DeviceState { raw, .. } => raw.clone(),
            Self::DeviceReport { raw, .. } => raw.clone(),
            Self::Availability { online, .. } => {
                Value::String(if *online { "online" } else { "offline" }.to_string())
            }
        }
    }
}

pub fn parse(p: Publish) -> Vec<(String, Z2mEvent)> {
    let topic = p.topic;
    let parts: Vec<String> = topic.split('/').map(str::to_string).collect();

    let Some(prefix) = parts.first() else {
        return Vec::new();
    };
    let Some(device) = parts.get(1).cloned() else {
        return Vec::new();
    };
    let sub = parts.get(2).map(String::as_str);

    if prefix != "zigbee2mqtt" {
        return Vec::new();
    }

    if device == "bridge" {
        return parse_bridge_message(topic, sub, &p.payload);
    }

    match sub {
        None => parse_device_message(topic, device, &p.payload),
        Some("availability") => parse_availability(topic, device, &p.payload),
        _ => Vec::new(),
    }
}

fn parse_device_message(topic: String, device: String, payload: &[u8]) -> Vec<(String, Z2mEvent)> {
    let Some(json) = serde_json::from_slice::<Value>(payload).ok() else {
        return Vec::new();
    };

    let mut events = Vec::new();

    if let Some(state) = match json.get("state").and_then(|value| value.as_str()) {
        Some("ON") => Some(DeviceState::On),
        Some("OFF") => Some(DeviceState::Off),
        _ => None,
    } {
        events.push((
            topic.clone(),
            Z2mEvent::DeviceState {
                device: device.clone(),
                state,
                raw: json.clone(),
            },
        ));
    }

    if let Some(values) = reported_values(&json) {
        events.push((
            topic,
            Z2mEvent::DeviceReport {
                device,
                values,
                raw: json,
            },
        ));
    }

    events
}

fn reported_values(json: &Value) -> Option<Vec<DeviceReportedValue>> {
    let values = json.as_object()?;

    let values = values
        .iter()
        .map(|(property, value)| DeviceReportedValue {
            property: property.clone(),
            value: value.clone(),
        })
        .collect::<Vec<_>>();

    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

fn parse_availability(topic: String, device: String, payload: &[u8]) -> Vec<(String, Z2mEvent)> {
    let Some(value) = std::str::from_utf8(payload).ok().map(str::trim) else {
        return Vec::new();
    };

    vec![(
        topic,
        Z2mEvent::Availability {
            device,
            online: value == "online",
        },
    )]
}

fn parse_bridge_message(
    topic: String,
    sub: Option<&str>,
    payload: &[u8],
) -> Vec<(String, Z2mEvent)> {
    if sub != Some("devices") {
        return Vec::new();
    }

    let Some(devices) = serde_json::from_slice::<Value>(payload)
        .ok()
        .and_then(|value| value.as_array().cloned())
    else {
        return Vec::new();
    };

    devices
        .into_iter()
        .filter_map(|raw| {
            let friendly_name = raw.get("friendly_name")?.as_str()?;
            if friendly_name == "Coordinator" {
                return None;
            }

            Some((
                topic.clone(),
                Z2mEvent::DeviceDiscovered {
                    device: friendly_name.to_string(),
                    name: friendly_name.to_string(),
                    raw,
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use rumqttc::{Publish, QoS};
    use serde_json::json;

    use super::{Z2mEvent, parse};
    use crate::application::device_event::DeviceReportedValue;
    use crate::domain::DeviceState;

    #[test]
    fn parses_device_state() {
        let publish = Publish::new(
            "zigbee2mqtt/plug_plant",
            QoS::AtLeastOnce,
            r#"{"state":"ON"}"#,
        );

        let events = parse(publish);

        assert_eq!(
            events,
            vec![
                (
                    "zigbee2mqtt/plug_plant".to_string(),
                    Z2mEvent::DeviceState {
                        device: "plug_plant".to_string(),
                        state: DeviceState::On,
                        raw: json!({ "state": "ON" }),
                    }
                ),
                (
                    "zigbee2mqtt/plug_plant".to_string(),
                    Z2mEvent::DeviceReport {
                        device: "plug_plant".to_string(),
                        values: vec![DeviceReportedValue {
                            property: "state".to_string(),
                            value: json!("ON"),
                        }],
                        raw: json!({ "state": "ON" }),
                    }
                )
            ]
        );
    }

    #[test]
    fn parses_device_report() {
        let publish = Publish::new(
            "zigbee2mqtt/window_opener",
            QoS::AtLeastOnce,
            r#"{"position":75,"status":"off","linkquality":240}"#,
        );

        let events = parse(publish);

        assert_eq!(
            events,
            vec![(
                "zigbee2mqtt/window_opener".to_string(),
                Z2mEvent::DeviceReport {
                    device: "window_opener".to_string(),
                    values: vec![
                        DeviceReportedValue {
                            property: "linkquality".to_string(),
                            value: json!(240),
                        },
                        DeviceReportedValue {
                            property: "position".to_string(),
                            value: json!(75),
                        },
                        DeviceReportedValue {
                            property: "status".to_string(),
                            value: json!("off"),
                        },
                    ],
                    raw: json!({ "position": 75, "status": "off", "linkquality": 240 }),
                }
            )]
        );
    }

    #[test]
    fn parses_availability() {
        let publish = Publish::new(
            "zigbee2mqtt/plug_plant/availability",
            QoS::AtLeastOnce,
            "online",
        );

        let events = parse(publish);

        assert_eq!(
            events,
            vec![(
                "zigbee2mqtt/plug_plant/availability".to_string(),
                Z2mEvent::Availability {
                    device: "plug_plant".to_string(),
                    online: true,
                }
            )]
        );
    }

    #[test]
    fn parses_bridge_devices() {
        let publish = Publish::new(
            "zigbee2mqtt/bridge/devices",
            QoS::AtLeastOnce,
            r#"[
                {"friendly_name":"Coordinator","type":"Coordinator"},
                {"friendly_name":"plug_plant","type":"Router"}
            ]"#,
        );

        let events = parse(publish);

        assert_eq!(
            events,
            vec![(
                "zigbee2mqtt/bridge/devices".to_string(),
                Z2mEvent::DeviceDiscovered {
                    device: "plug_plant".to_string(),
                    name: "plug_plant".to_string(),
                    raw: json!({
                        "friendly_name": "plug_plant",
                        "type": "Router"
                    }),
                }
            )]
        );
    }

    #[test]
    fn ignores_other_bridge_messages() {
        let publish = Publish::new("zigbee2mqtt/bridge/state", QoS::AtLeastOnce, "online");

        assert!(parse(publish).is_empty());
    }
}
