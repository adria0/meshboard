use std::path::Path;

use anyhow::{Result, bail};
use log::info;

use crate::mesh::service::Destination;

// pub mod repl;
pub mod service;
pub mod storage;

pub(crate) async fn run_bbs() -> Result<()> {
    info!("Starting MeshBoard");

    let storage = storage::Storage::open(Path::new("./meshboard.db"))?;
    let mut bbs = service::BBS::new(storage);
    bbs.init().await?;

    let ble_device = std::env::var("BLE_DEVICE")?;
    info!("Connecting to device {ble_device}...");

    let mut handler = crate::mesh::service::Service::from_ble(&ble_device).await?;
    info!("Booting...");
    if let Err(err) = handler.wait_for_boot_ready(30).await {
        println!("Error: {}", err);
    }
    loop {
        tokio::select! {
            status = handler.status_rx.recv() => {
                use crate::mesh::service::Status;
                let Some(status) = status else { bail!("Channel closed"); };
                match status {
                    Status::NewMessage(id) => {
                        let state = handler.state.read().await;
                        let msg = state.messages.get(&id).unwrap();
                        if msg.to != state.my_node_num().await {
                            continue;
                        }
                        let short_name = state.get_short_name_by_node_id(msg.from).unwrap_or("?".to_string());
                        let pk_hash = msg.pk_hash;
                        let response_msgs = bbs.handle(pk_hash,&short_name, &msg.text).await?;
                        info!("{}:{}", short_name, hex::encode(pk_hash));
                        info!("> {}", msg.text);
                        for response_msg in response_msgs {
                            info!("< {}", response_msg);
                            handler.send_text(response_msg, Destination::Node(msg.from)).await?;
                        }
                    },
                    Status::UpdatedMessage(_msg) => {},
                    Status::Heartbeat(_packet_count) => {
                        info!("Alive.");
                    },
                    Status::FromRadio(_) => {},
                    Status::Ready => {},
                }
            }
            _ = handler.cancel.cancelled() => break,
        }
    }

    Ok(())
}
