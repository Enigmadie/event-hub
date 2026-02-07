const PREFIX: &str = "zigbee2mqtt";

pub fn subscriptions() -> Vec<String> {
    vec![format!("{PREFIX}/+"), format!("{PREFIX}/+/availability")]
}
