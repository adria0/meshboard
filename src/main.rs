//! This example connects via Bluetooth LE to the radio and prints out all received packets.
#[allow(unused)]
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use meshtastic::api::StreamApi;
use meshtastic::protobufs::MeshPacket;

mod bbs;
mod mesh;
mod repl;
mod service;
mod storage;
mod telegram;
mod utils;

use meshtastic::utils::generate_rand_id;
use meshtastic::utils::stream::{BleId, build_ble_stream};
use serde_cbor::Deserializer;
use storage::Storage;
use tokio::select;
use tokio_util::sync::CancellationToken;

use crate::mesh::service::Destination;
use crate::service::Service;
use crate::telegram::TelegramBot;

include!(concat!(env!("OUT_DIR"), "/build_info.rs"));

#[derive(Parser)]
#[command(name = "mbbs")]
#[command(version = VERSION)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/* What is does?
 *
 *
 *
 */

#[derive(Subcommand)]
enum Commands {
    /// BBS REPL
    BBSRepl,
    /// BBS Service
    BBS,
    /// Fast check
    FastCheck,
    /// Discover BLE nodes
    Repl,
    /// Start the network node
    Start,
    /// Discover peers
    Discover,
    /// Dump and pretty-print a CBOR file
    Dump {
        /// Path to the CBOR file
        file: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::BBS => bbs().await?,
        Commands::FastCheck => fast_check().await?,
        Commands::Repl => repl::repl().await?,
        Commands::BBSRepl => bbs::repl::run_repl().await?,
        Commands::Start => start().await?,
        Commands::Discover => discover().await?,
        Commands::Dump { file } => dump(file).await?,
    }

    Ok(())
}

async fn fast_check() -> Result<()> {
    log::info!("Fast check...");
    let mut devices =
        meshtastic::utils::stream::available_ble_devices(Duration::from_secs(5)).await?;
    if devices.is_empty() {
        log::warn!("No BLE devices found");
        return Ok(());
    }
    let device_name = devices.remove(0).name.unwrap();
    log::info!("Connecting to device {device_name}");

    let ble_stream = build_ble_stream(&BleId::from_name(&device_name), Duration::from_secs(5))
        .await
        .expect("Unable to build BLE stream");

    let stream_api = StreamApi::new();
    log::info!("Opening stream API");
    let (mut packet_rx, stream_api) = stream_api.connect(ble_stream).await;
    let config_id = generate_rand_id();
    log::info!("Asking for configuration");
    let _stream_api = stream_api
        .configure(config_id)
        .await
        .expect("Unable to open stream api");
    log::info!("Getting first packet");
    println!("{:?}", packet_rx.recv().await);

    Ok(())
}

async fn dump(path: PathBuf) -> Result<()> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let stream = Deserializer::from_reader(reader).into_iter::<MeshPacket>();
    for mesh_packet in stream {
        println!("{:?}", mesh_packet);
    }
    Ok(())
}

async fn discover() -> Result<()> {
    log::info!("Scanning BLE devices...");
    let devices = meshtastic::utils::stream::available_ble_devices(Duration::from_secs(5)).await?;
    for device in devices {
        log::info!(
            "Found BLE device: name={:?} mac={}",
            device.name,
            device.mac_address
        );
    }
    Ok(())
}

async fn bbs() -> Result<()> {
    let storage = bbs::storage::Storage::open(Path::new("./db"))?;
    let mut bbs = bbs::service::BBS::new(storage);
    bbs.init().await?;

    let ble_device = std::env::var("BLE_DEVICE")?;
    let mut handler = mesh::service::Service::from_ble(&ble_device).await?;
    println!("Using device: {}, booting..", ble_device);
    if let Err(err) = handler.wait_for_boot_ready(30).await {
        println!("Error: {}", err);
    }
    loop {
        tokio::select! {
            status = handler.status_rx.recv() => {
                use mesh::service::Status;
                let Some(status) = status else { bail!("Channel closed"); };
                match status {
                    Status::Ready => {
                        println!("Ready");
                    },
                    Status::NewMessage(id) => {
                        let state = handler.state.read().await;
                        let msg = state.messages.get(&id).unwrap();
                        if msg.to != state.my_node_num().await {
                            continue;
                        }
                        let short_name = state.get_short_name_by_node_id(msg.from).unwrap_or("?".to_string());
                        let pk_hash = msg.pk_hash;
                        let response_msgs = bbs.handle(pk_hash,&short_name, &msg.text).await?;
                        for response_msg in response_msgs {
                            handler.send_text(response_msg, Destination::Node(msg.from)).await?;
                        }
                    },
                    Status::UpdatedMessage(_msg) => {
                    },
                    Status::Heartbeat(_packet_count) => {
                        println!("Heartbeat.");
                    },
                    Status::FromRadio(_from_radio) => {
                    },
                }
            }
            _ = handler.cancel.cancelled() => break,
        }
    }

    Ok(())
}

async fn start() -> Result<()> {
    println!("VERSION {}", VERSION);

    let telegram_bot_token = std::env::var("TELEGRAM_BOT_TOKEN")?;
    let telegram_bot_chatid = std::env::var("TELEGRAM_BOT_CHATID")?.parse()?;
    let ble_device = std::env::var("BLE_DEVICE")?;

    let cancel = CancellationToken::new();
    let cancel_ctrl_c = cancel.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        cancel_ctrl_c.cancel();
    });

    log::info!("Connecting to telegram...");
    let mut bot = TelegramBot::new(telegram_bot_token, telegram_bot_chatid);
    let mut storage = Storage::default();

    loop {
        let mut service = Service::new(cancel.clone(), &mut bot, &mut storage, ble_device.clone());
        if let Err(err) = service.run().await {
            bot.send_message(format!("⚠️ Error running service: {}", err))
                .await?;
            log::error!("Error running service: {}", err);
        }

        select! {
            _ = cancel.cancelled() => break,
            _ = tokio::time::sleep(Duration::from_secs(5)) => {},
        };
    }
    Ok(())
}
