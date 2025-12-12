use std::path::Path;

use anyhow::{Result, bail};
use log::info;

use crate::mesh::service::Destination;
use crate::screen::Screen;

// pub mod repl;
pub mod service;
pub mod storage;

const SPINNER: [&str; 12] = ["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷", "⣾", "⣽", "⣻", "⢿"];

fn info<D: Screen>(display: &mut D, row: usize, message: &str) {
    info!("{}", message);
    display.draw_text_at(message, 0, row as i32);
    let _ = display.refresh();
}

pub(crate) async fn run_bbs<D: Screen>(mut display: D) -> Result<()> {
    let mut spinner = 0;
    let mut packet_count = 0;

    info(&mut display, 0, "Starting MeshBoard");

    let storage = storage::Storage::open(Path::new("./meshboard.db"))?;
    let mut bbs = service::BBS::new(storage);
    bbs.init().await?;

    let ble_device = std::env::var("BLE_DEVICE")?;
    info(&mut display, 0, &format!("Connect {ble_device}..."));

    let mut handler = crate::mesh::service::Service::from_ble(&ble_device).await?;
    info(&mut display, 0, "Booting...");
    if let Err(err) = handler.wait_for_boot_ready(30).await {
        println!("Error: {}", err);
    }
    info(&mut display, 0, "Ready");
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
                        info(&mut display, 1, &format!("{}:{}", short_name, hex::encode(pk_hash)));
                        info(&mut display, 2, &format!("> {}", msg.text));
                        for (n, response_msg) in response_msgs.iter().enumerate() {
                            info(&mut display, 3+n, &format!("< {}", response_msg));
                            handler.send_text(response_msg, Destination::Node(msg.from)).await?;
                        }
                    },
                    Status::UpdatedMessage(_msg) => {},
                    Status::Heartbeat(_packet_count) => {
                        info(&mut display, 0, &format!("Stats {} {} ", SPINNER[spinner], packet_count));
                        spinner = (spinner + 1) % SPINNER.len();
                    },
                    Status::FromRadio(_) => {
                        packet_count += 1;
                    },
                    Status::Ready => {},
                }
            }
            _ = handler.cancel.cancelled() => break,
        }
    }

    Ok(())
}
