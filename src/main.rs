use event_hub::{
    integrations::zigbee2mqtt::{
        client::Z2mClient,
        events::{Z2mEvent, parse},
        subscriptions::subscriptions,
    },
    transport::mqtt::client::{MqttConfig, MqttRuntime},
};
use rumqttc::{Event, Packet, QoS};

fn main() {
    env_logger::init();

    let mut mqtt = MqttRuntime::connect(MqttConfig {
        client_id: "home".to_string(),
        host: "192.168.0.219".to_string(),
        port: 1883,
    });

    for topic in subscriptions() {
        println!("Subscribing: {}", topic);
        mqtt.client
            .subscribe(topic, QoS::AtLeastOnce)
            .expect("subscribe failed");
    }

    println!("Home is listening...");
    let mut z2m = Z2mClient::new(&mut mqtt);
    z2m.turn_off("test_device");

    for event in mqtt.connection.iter() {
        match event {
            Ok(Event::Incoming(Packet::Publish(p))) => {
                println!("RAW {} ({} bytes)", p.topic, p.payload.len());
                if let Some((topic, event)) = parse(p) {
                    match event {
                        Z2mEvent::DeviceState { device, on, raw } => {
                            println!("RAW JSON: {}", raw);
                            println!("Device {device} state: {on}");
                        }
                        Z2mEvent::Availability { device, online } => {
                            println!("Device {device} is {online}");
                        }
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("MQTT error: {:?}", e);
                break;
            }
        }
    }
}
