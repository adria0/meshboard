use clap::{Parser, Subcommand};
use mini_moka::sync::Cache;
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, bail};

use crate::bbs::storage::ChannelMessage;
use crate::bbs::storage::Storage;
use crate::bbs::storage::User;
use crate::bbs::storage::UserPkHash;

const HELP: &str = "h(elp) | c(hannels)  | j(oin) ch | p(ost) msg  | l(list)";

/// Subcomandos del BBS, equivalentes a h/c/j/p/l
#[derive(Debug, Subcommand)]
pub enum BbsCommand {
    /// h(elp)
    #[command(name = "h")]
    Help,

    /// c(hannels)
    #[command(name = "c")]
    Channels,

    /// j(oin) ch
    #[command(name = "j")]
    Join {
        /// Nombre del canal
        ch: String,
    },

    /// p(ost) msg
    #[command(name = "p")]
    Post {
        /// Texto del mensaje
        msg: String,
    },

    /// l(ist)
    #[command(name = "l")]
    List,
}
#[derive(Debug, Parser)]
#[command(disable_help_flag = true)] // ya tienes tu propio "h"
pub struct BbsCli {
    #[command(subcommand)]
    pub cmd: BbsCommand,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct Session {
    created: Instant,
    user_id: u32,
    current_channel: u32,
}

pub struct BBS {
    storage: Storage,
    sessions: Cache<UserPkHash, Session>,
}

impl BBS {
    pub fn new(storage: Storage) -> Self {
        Self {
            storage,
            sessions: Cache::builder()
                .max_capacity(1024)
                .time_to_live(Duration::from_secs(3600))
                .build(),
        }
    }

    pub async fn init(&mut self) -> Result<()> {
        if self.storage.get_channels()?.is_empty() {
            self.storage.add_channel("news")?;
            self.storage.add_channel("general")?;
        }
        Ok(())
    }

    fn parse_command_line(&self, command: &str) -> Result<BbsCommand> {
        // clap espera un argv, así que inventamos el argv[0]
        let argv = std::iter::once("bbs").chain(command.split_whitespace());
        let cli = BbsCli::try_parse_from(argv).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        Ok(cli.cmd)
    }

    pub async fn handle(
        &mut self,
        user_pk_hash: [u8; 32],
        short_name: &str,
        command: &str,
    ) -> Result<Vec<String>> {
        let user_pk_hash = UserPkHash(user_pk_hash);
        let mut session = if let Some(session) = self.sessions.get(&user_pk_hash) {
            session
        } else {
            let current_channel = 0;
            let user_id = if let Ok(user) = self.storage.get_user_by_pkhash(user_pk_hash.clone()) {
                user.uid
            } else {
                self.storage.add_user(User {
                    uid: 0,
                    short_name: short_name.to_string(),
                    pk_hash: user_pk_hash.clone(),
                    last_ts: 0,
                })?
            };

            Session {
                created: Instant::now(),
                current_channel,
                user_id,
            }
        };

        let mut user = self.storage.get_user_by_id(session.user_id)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        match self.parse_command_line(command) {
            Ok(BbsCommand::Channels) => {
                let channels = self.storage.get_channels()?;
                let list = channels
                    .iter()
                    .map(|c| c.name.clone())
                    .collect::<Vec<String>>()
                    .join(",");
                return Ok(vec![list]);
            }
            Ok(BbsCommand::Join { ch }) => {
                let channels = self.storage.get_channels()?;
                let Some(channel) = channels.iter().find(|_ch| _ch.name == ch) else {
                    bail!("Channel not found");
                };
                session.current_channel = channel.cid;
                self.sessions.insert(user_pk_hash, session);
                return Ok(vec!["Ack".into()]);
            }
            Ok(BbsCommand::Post { msg }) => {
                let message = ChannelMessage {
                    cid_ts: (session.current_channel, now),
                    uid: session.user_id,
                    text: format!("{}: {}", user.short_name, msg),
                };

                self.storage.add_message(message)?;

                return Ok(vec!["Ack".into()]);
            }

            Ok(BbsCommand::List) => {
                let messages =
                    self.storage
                        .get_messages(session.current_channel, user.last_ts, now)?;
                let mut ret = vec![format!("{} Messages.", messages.len())];
                for msg in messages {
                    let days = (now - msg.cid_ts.1) / (24 * 60 * 60);
                    ret.push(format!("{}d, {}", days, msg.text));
                }
                user.last_ts = now;
                self.storage.update_user(user.uid, user)?;
                return Ok(ret);
            }
            _ => {
                return Ok(vec![HELP.into()]);
            }
        }
    }
}
