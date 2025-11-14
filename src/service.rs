use std::{fs::File, time::Duration};

use crate::{storage::Storage, telegram::TelegramBot, utils::IncomingPacket};
use anyhow::{Result, anyhow, bail};
use chrono::Local;
use meshtastic::{
    api::StreamApi,
    protobufs::{FromRadio, from_radio},
    utils::stream::BleId,
};
use tokio::select;
use tokio_util::sync::CancellationToken;

pub struct Service<'a> {
    cancel: CancellationToken,
    bot: &'a mut TelegramBot,
    storage: &'a mut Storage,
    ble_device: String,
}

impl<'a> Service<'a> {
    pub fn new(
        cancel: CancellationToken,
        bot: &'a mut TelegramBot,
        storage: &'a mut Storage,
        ble_device: String,
    ) -> Self {
        Self {
            cancel,
            bot,
            storage,
            ble_device,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        log::info!("Opening BLE to meshtastic device {}...", &self.ble_device);
        let _ = self
            .bot
            .send_message(format!("Start get events from {}", &self.ble_device))
            .await;

        let ble_stream = meshtastic::utils::stream::build_ble_stream(
            &BleId::from_name(&self.ble_device),
            Duration::from_secs(5),
        )
        .await?;

        let stream_api = StreamApi::new();
        let (mut radio_rx, stream_api) = stream_api.connect(ble_stream).await;
        let config_id = meshtastic::utils::generate_rand_id();
        let stream_api = stream_api.configure(config_id).await?;

        let mut err = None;

        let mut idle_counter = 0;
        let mut stats_recv_count = 0;

        while err.is_none() {
            if self.bot.last_sent_message_secs() > 3600 {
                self.bot
                    .send_message(format!("Alive, recv_count: {}", stats_recv_count))
                    .await?;
            }
            select! {
                _ = self.cancel.cancelled() => break,
                from_radio = radio_rx.recv() => {
                    if from_radio.is_none() {
                        bail!("radio_rx stream finished");
                    }
                    let from_radio = from_radio.unwrap();
                    idle_counter=0;
                    stats_recv_count += 1;

                    if let Err(process_err) = self.process(from_radio).await {
                        err = Some(process_err);
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    idle_counter += 30;
                    log::info!("Nothing happened in {}s, but still alive", idle_counter);
                    if idle_counter >= 300{
                        log::warn!("Idle for more than 300s, I'm going to reset");
                        err = Some(anyhow!("Idle for more than 300s, I'm going to reset"));
                    }
                }
            }
        }

        let _ = stream_api.disconnect().await?;

        if let Some(err) = err {
            Err(err)
        } else {
            Ok(())
        }
    }

    async fn process(&mut self, from_radio: FromRadio) -> Result<()> {
        self.save_packet(&from_radio).await?;

        let incoming = from_radio.into();
        log::info!("recv {:?}", incoming);

        match incoming {
            IncomingPacket::NodeInfo(id, user) => {
                self.storage.insert_node(id, user);
            }
            IncomingPacket::TextMessage { from, to, msg } => {
                let from = self.storage.long_name_of(from);
                let msg = if to == 0xffffffff {
                    format!("💬 {} : {}", from, msg)
                } else {
                    format!("📩 {} → {} : {} ", from, self.storage.long_name_of(to), msg)
                };
                self.bot.send_message(msg).await?;
            }
            _ => {}
        }

        Ok(())
    }

    async fn save_packet(&mut self, from_radio: &FromRadio) -> Result<()> {
        let Some(ref payload) = from_radio.payload_variant else {
            return Ok(());
        };
        let from_radio::PayloadVariant::Packet(mesh_packet) = payload else {
            return Ok(());
        };

        let date = Local::now().format("%Y-%m-%d").to_string();
        let filename = format!("network.{}.cbor", date);
        let file = File::options().create(true).append(true).open(&filename)?;
        serde_cbor::to_writer(&file, &mesh_packet)?;

        Ok(())
    }
}
