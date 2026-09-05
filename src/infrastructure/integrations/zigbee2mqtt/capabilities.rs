use crate::application::recurring_command::DeviceCommand;
use serde_json::Value;

/// Only advertise payloads the existing command gateway can actually send.
/// None = discovery metadata unavailable, Some([]) = no supported hub commands.
pub fn supported_commands(device: &Value) -> Option<Vec<DeviceCommand>> {
    let exposes = device.get("definition")?.get("exposes")?.as_array()?;
    let mut commands = Vec::new();
    for expose in exposes {
        visit(expose, &mut commands);
    }
    Some(commands)
}

fn visit(expose: &Value, commands: &mut Vec<DeviceCommand>) {
    // Endpoint/composite payloads need distinct gateway support; never flatten them.
    if expose.get("endpoint").is_some_and(|v| !v.is_null()) {
        return;
    }
    if matches!(
        expose.get("type").and_then(Value::as_str),
        Some("light" | "switch" | "cover" | "fan")
    ) {
        if let Some(features) = expose.get("features").and_then(Value::as_array) {
            for feature in features {
                visit(feature, commands);
            }
        }
    }
    if expose.get("access").and_then(Value::as_u64).unwrap_or(0) & 2 == 0 {
        return;
    }
    let mut add = |command| {
        if !commands.contains(&command) {
            commands.push(command);
        }
    };
    match expose.get("property").and_then(Value::as_str) {
        Some("state") if expose.get("type").and_then(Value::as_str) == Some("binary") => {
            if expose.get("value_on").and_then(Value::as_str) == Some("ON") {
                add(DeviceCommand::TurnOn);
            }
            if expose.get("value_off").and_then(Value::as_str) == Some("OFF") {
                add(DeviceCommand::TurnOff);
            }
        }
        Some("state") if expose.get("type").and_then(Value::as_str) == Some("enum") => {
            if let Some(values) = expose.get("values").and_then(Value::as_array) {
                for (value, command) in [
                    ("OPEN", DeviceCommand::Open),
                    ("CLOSE", DeviceCommand::Close),
                    ("STOP", DeviceCommand::Stop),
                ] {
                    if values.iter().any(|v| v.as_str() == Some(value)) {
                        add(command);
                    }
                }
            }
        }
        Some("position")
            if expose.get("type").and_then(Value::as_str) == Some("numeric")
                && expose.get("value_min").and_then(Value::as_f64) == Some(0.0)
                && expose.get("value_max").and_then(Value::as_f64) == Some(100.0)
                && expose
                    .get("value_step")
                    .and_then(Value::as_f64)
                    .is_none_or(|step| {
                        step > 0.0 && step <= 1.0 && (1.0 / step).fract() == 0.0
                    }) =>
        {
            add(DeviceCommand::SetPosition);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn distinguishes_unknown_and_read_only_devices() {
        assert_eq!(supported_commands(&json!({})), None);
        assert_eq!(
            supported_commands(
                &json!({"definition":{"exposes":[{"type":"numeric","property":"temperature","access":1}]}})
            ),
            Some(vec![])
        );
    }
    #[test]
    fn extracts_only_writable_supported_payloads() {
        let device = json!({"definition":{"exposes":[
            {"type":"switch","features":[{"type":"binary","property":"state","access":7,"value_on":"ON","value_off":"OFF"}]},
            {"type":"cover","features":[{"type":"enum","property":"state","access":2,"values":["OPEN","CLOSE","STOP"]},{"type":"numeric","property":"position","access":7,"value_min":0,"value_max":100}]}
        ]}});
        assert_eq!(
            supported_commands(&device),
            Some(vec![
                DeviceCommand::TurnOn,
                DeviceCommand::TurnOff,
                DeviceCommand::Open,
                DeviceCommand::Close,
                DeviceCommand::Stop,
                DeviceCommand::SetPosition
            ])
        );
    }
    #[test]
    fn does_not_invent_commands_from_measurements_or_endpoints() {
        let device = json!({"definition":{"exposes":[
            {"type":"binary","property":"state","access":1,"value_on":"ON","value_off":"OFF"},
            {"type":"switch","endpoint":"left","features":[{"type":"binary","property":"state","access":7,"value_on":"ON","value_off":"OFF"}]},
            {"type":"composite","property":"custom","features":[{"type":"enum","property":"state","access":2,"values":["OPEN"]}]}
        ]}});
        assert_eq!(supported_commands(&device), Some(vec![]));
    }
}
