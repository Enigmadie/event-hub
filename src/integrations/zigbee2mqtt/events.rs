use rumqttc::Publish;
use serde_json::Value;

#[derive(Debug, PartialEq, Clone)]
pub enum Z2mEvent {
    DeviceState {
        device: String,
        on: bool,
        raw: Value,
    },
    Availability {
        device: String,
        online: String,
    },
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
            let on = json
                .get("state")
                .and_then(|v| v.as_str())
                .map(|s| s == "ON")
                .unwrap_or(false);

            Some((
                topic.clone(),
                Z2mEvent::DeviceState {
                    device: device.to_string(),
                    on,
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
                    online: s.to_string(),
                },
            ))
        }
        _ => None,
    }
}
