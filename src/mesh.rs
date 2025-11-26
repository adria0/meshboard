use anyhow::{Result, bail};
use tokio::task;

mod router;
mod service;
mod types;

pub async fn mesh_example() -> Result<()> {
    let (mut handler, service) = service::Service::from_ble("NID3_ab2c").await?;
    tokio::spawn(service.start());

    loop {
        tokio::select! {
            _ = handler.cancel.cancelled() => break,
            status = handler.status_rx.recv() => {
                let Some(status) = status else { bail!("Channel closed"); };
                match status {
                    service::Status::NewMessage(id) => {
                        let msg = handler.msg(id).await.unwrap();
                        println!("{}", handler.format_msg(&msg).await);
                        if handler.me().await == msg.to {
                            handler.send_text(format!("Got {}", msg.text), msg.from).await?;
                        }
                    } ,
                    service::Status::UpdatedMessage(id) => {
                        let msg = handler.msg(id).await.unwrap();
                        println!("{}", handler.format_msg(&msg).await);
                    },
                    service::Status::PacketCount(status) => {
                        println!("Packet count: {}", status);
                    },
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
