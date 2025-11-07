//! This example connects via Bluetooth LE to the radio and prints out all received packets.
use std::collections::{BTreeMap, HashMap};
use std::io::{self, BufRead};
use std::path::{Display, Path};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Error, Result};
use meshtastic::Message;
use meshtastic::api::{ConnectedStreamApi, StreamApi, state};
use meshtastic::packet::{PacketDestination, PacketRouter};
use meshtastic::protobufs::mesh_packet;
use meshtastic::protobufs::{Data, User};
use meshtastic::protobufs::{FromRadio, PortNum};
use meshtastic::protobufs::{MeshPacket, from_radio};
use meshtastic::types::{MeshChannel, NodeId};
use meshtastic::utils;
use meshtastic::utils::stream::BleId;
use serde::{Deserialize, Serialize};
use teloxide::{prelude::*, types::ChatId};
use time::OffsetDateTime;

const STORAGE_FILE: &str = "storage.json";

#[derive(Debug, Serialize, Deserialize)]
struct Storage {
    users: HashMap<u32, User>,
    stats: BTreeMap<String, (u32, u64)>,
}

impl Storage {
    fn new() -> Self {
        Storage {
            stats: BTreeMap::new(),
            users: HashMap::new(),
        }
    }
    fn long_name_of(&self, user_id: u32) -> String {
        if let Some(user) = self.users.get(&user_id) {
            user.long_name.clone()
        } else {
            format!("{}", user_id)
        }
    }
    fn nodeid_by_name(&self, name: &str) -> Option<NodeId> {
        self.users
            .iter()
            .find(|(k, user)| user.long_name == name)
            .map(|(k, _)| NodeId::new(*k))
    }
    fn load(path: &Path) -> Result<Self> {
        if let Ok(fs) = std::fs::File::open(path) {
            let storage = serde_json::from_reader(fs)?;
            Ok(storage)
        } else {
            Ok(Storage::new())
        }
    }
    fn save(&self, path: &Path) -> Result<()> {
        let mut fs = std::fs::File::create(path)?;
        serde_json::to_writer(&mut fs, self)?;
        Ok(())
    }
    fn insert_stat(&mut self, mesh_packet: &MeshPacket, info: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(); // u64 seconds

        let key = format!("{}:{}", self.long_name_of(mesh_packet.from), info);
        if let Some(entry) = self.stats.get_mut(&key) {
            entry.0 += 1;
            entry.1 = now;
        } else {
            self.stats.insert(key.clone(), (1, now));
        }
    }

    fn print_stats(&self) {
        let datetime_format =
            time::format_description::parse("[month]-[day] [hour]:[minute]:[second]").unwrap();

        println!("Stats -------------------------------------");
        for (k, (count, ts)) in &self.stats {
            let dt = OffsetDateTime::from_unix_timestamp(*ts as i64).unwrap();
            let dt = dt.format(&datetime_format).unwrap();
            println!("{:>10}: {} [{}]", k, count, dt);
        }
        println!("------------------------------------------");
    }
}

async fn send_to_meshtastic(stream_api: &mut ConnectedStreamApi, text: &str) -> Result<()> {
    // Create a text message data payload
    let data = Data {
        portnum: PortNum::TextMessageApp as i32,
        payload: text.as_bytes().to_vec(),
        want_response: false,
        ..Default::default()
    };

    // Create mesh packet for broadcast
    let mesh_packet = MeshPacket {
        to: 0xffffffff, // Broadcast address
        from: 0,        // Will be filled by the device
        channel: 0,
        id: 0, // Will be assigned by the device
        priority: mesh_packet::Priority::Default as i32,
        payload_variant: Some(mesh_packet::PayloadVariant::Decoded(data)),
        ..Default::default()
    };

    // Create the payload variant
    let payload_variant = Some(meshtastic::protobufs::to_radio::PayloadVariant::Packet(
        mesh_packet,
    ));

    // Send using the stream API's send_to_radio_packet method
    println!("Attempting to send packet to Meshtastic radio...");
    match stream_api.send_to_radio_packet(payload_variant).await {
        Ok(_) => {
            println!("Successfully sent to Meshtastic: {}", text);
            Ok(())
        }
        Err(e) => {
            println!("Failed to send to Meshtastic: {}", e);
            Err(anyhow::anyhow!("Failed to send message: {}", e))
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let telegram_bot_token =
        std::env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set in .env");
    let telegram_bot_chatid = std::env::var("TELEGRAM_BOT_CHATID")
        .expect("TELEGRAM_BOT_CHATID not set in .env")
        .parse()
        .expect("UNABLE TO PARSE TELEGRAM_BOTID");
    let ble_device = std::env::var("BLE_DEVICE");

    // let (chat_id,secret) = token.split_once(':').unwrap();
    let bot = Bot::new(telegram_bot_token);
    //     teloxide::repl(bot, |bot: Bot, msg: teloxide::prelude::Message| async move {
    //        println!("Chat ID: {}", msg.chat.id);
    //        bot.send_message(msg.chat.id, "Got your message ✅").await?;
    //        Ok(())
    //   })
    //   .await;
    let chat_id = ChatId(telegram_bot_chatid);
    bot.send_message(chat_id, "Started").await?;

    let stream_api = StreamApi::new();
    let mut storage = Storage::load(Path::new(STORAGE_FILE))?;
    storage.print_stats();

    let ble_device = if let Some(ble_device) = std::env::args().nth(1) {
        ble_device
    } else if let Ok(ble_device) = ble_device {
        ble_device
    } else {
        println!("Scanning BLE devices...");
        let devices =
            meshtastic::utils::stream::available_ble_devices(Duration::from_secs(5)).await?;
        for device in devices {
            println!(
                "Found BLE device: name={:?} mac={}",
                device.name, device.mac_address
            );
        }
        return Ok(());
    };

    // You can also use `BleId::from_mac_address(..)` instead of `BleId::from_name(..)` to
    // search for a MAC address.
    println!("Opening BLE stream...");
    let ble_stream =
        utils::stream::build_ble_stream(&BleId::from_name(&ble_device), Duration::from_secs(5))
            .await?;
    let (mut decoded_listener, stream_api) = stream_api.connect(ble_stream).await;

    let config_id = utils::generate_rand_id();
    let mut stream_api = stream_api.configure(config_id).await?;

    //    send_to_meshtastic(&mut stream_api, "Meshtastic BBS test").await?;

    let mut last_print = std::time::Instant::now();

    // This loop can be broken with ctrl+c, by disabling bluetooth or by turning off the radio.
    println!("Start looping");
    while let Some(decoded) = decoded_listener.recv().await {
        let Some(packet) = decoded.payload_variant else {
            continue;
        };

        match packet {
            from_radio::PayloadVariant::NodeInfo(node_info) => {
                if let Some(user) = node_info.user {
                    storage.users.insert(node_info.num, user);
                }
            }
            from_radio::PayloadVariant::Packet(mesh_packet) => {
                let (bot_msg, info) = if let Some(ref pv) = mesh_packet.payload_variant {
                    match pv {
                        mesh_packet::PayloadVariant::Decoded(decoded) => {
                            let port_num =
                                PortNum::try_from(decoded.portnum).unwrap_or(PortNum::UnknownApp);
                            match port_num {
                                PortNum::TextMessageApp => {
                                    let msg = String::from_utf8(decoded.payload.clone())
                                        .unwrap_or("Non-utf8 msg".into());
                                    (Some(msg.clone()), format!("TextMessageApp: {}", msg))
                                }
                                PortNum::NodeinfoApp => {
                                    if let Ok(user) = User::decode(decoded.payload.as_slice()) {
                                        storage.users.insert(mesh_packet.from, user.clone());
                                        (None, format!("NodeinfoApp: {}", user.long_name))
                                    } else {
                                        eprintln!("{:?}", mesh_packet);
                                        (None, format!("NodeInfoDecodeErr"))
                                    }
                                }
                                _ => (None, format!("{}", port_num.as_str_name())),
                            }
                        }

                        mesh_packet::PayloadVariant::Encrypted(_) => (None, format!("Encrypted")),
                    }
                } else {
                    (None, format!(""))
                };

                storage.insert_stat(&mesh_packet, &info);

                let from = storage.long_name_of(mesh_packet.from);
                let to = if mesh_packet.to == 0xffffffff {
                    format!("BROADCAST")
                } else {
                    storage.long_name_of(mesh_packet.to)
                };

                let log = format!(
                    "{} -> {} snr:{} hop:{}({}) {}",
                    from,
                    to,
                    mesh_packet.rx_snr,
                    mesh_packet.hop_limit,
                    mesh_packet.hop_start,
                    info
                );

                if let Some(bot_msg) = bot_msg {
                    bot.send_message(chat_id, format!("💬 {}: {}", from, bot_msg))
                        .await?;
                } else if log.contains("nidra") || log.contains("Encrypted") {
                    bot.send_message(chat_id, format!("ℹ️ {}", &log)).await?;
                }

                println!("network1> {}", log);

                if last_print.elapsed() > std::time::Duration::from_secs(60) {
                    last_print = std::time::Instant::now();
                    storage.print_stats();
                }

                storage.save(Path::new(STORAGE_FILE))?;
            }
            _ => {}
        }
    }

    // Note that in this specific example, this will only be called when
    // the radio is disconnected, as the above loop will never exit.
    // Typically, you would allow the user to manually kill the loop,
    // for example, with tokio::select!.
    let _stream_api = stream_api.disconnect().await?;

    Ok(())
}

#[test]
fn test_decode() {
    let payload: Vec<u8> = vec![
        10, 9, 33, 100, 97, 101, 100, 54, 57, 100, 101, 18, 9, 240, 159, 147, 161, 95, 54, 57, 100,
        101, 26, 4, 240, 159, 147, 161, 34, 6, 232, 142, 218, 237, 105, 222, 40, 95, 66, 32, 86,
        239, 246, 241, 23, 143, 78, 97, 90, 32, 91, 124, 186, 255, 183, 25, 55, 254, 144, 234, 207,
        132, 215, 127, 215, 239, 80, 141, 204, 171, 191, 85, 72, 1,
    ];
    let ni =
        meshtastic::protobufs::User::decode(payload.as_slice()).expect("Failed to decode payload");
    println!("{:?}", ni);
    unreachable!();

    // Add your test code here
}
