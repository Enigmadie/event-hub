use event_hub::transport::mqqt::client::{MqttConfig, MqttRuntime};
use rumqttc::{Event, Packet, QoS};

fn main() {
    env_logger::init();

    let mut mqtt = MqttRuntime::connect(MqttConfig {
        client_id: "home".to_string(),
        host: "192.168.0.219".to_string(),
        port: 1883,
    });

    mqtt.client
        .subscribe("zigbee2mqtt/+/state", QoS::AtLeastOnce);

    mqtt.client
        .subscribe("zigbee2mqtt/+/availability", QoS::AtLeastOnce);

    mqtt.client
        .subscribe("zigbee2mqtt/+/power", QoS::AtLeastOnce);

    println!("Home is listening...");

    for event in mqtt.connection.iter() {
        match event {
            Ok(Event::Incoming(Packet::Publish(p))) => {
                let topic = p.topic;
                let payload = String::from_utf8_lossy(&p.payload);

                println!("{} → {}", topic, payload);
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("MQTT error: {:?}", e);
                break;
            }
        }
    }
}
