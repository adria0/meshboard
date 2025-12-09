use mini_moka::sync::Cache;
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, bail};

use crate::bbs::storage::ChannelMessage;
use crate::bbs::storage::Storage;
use crate::bbs::storage::User;
use crate::bbs::storage::UserPkHash;

const HELP: &str = "h(elp) | c(hannels)  | j(oin) ch | p(ost) msg  | l(list)";

pub enum Command {
    Help,
    Channels,
    Join { ch: String },
    Post { msg: String },
    List,
}
impl Command {
    pub fn parse(command: &str) -> Result<Self> {
        let mut parts = command.split_whitespace();
        match parts.next() {
            Some("h") | Some("help") => Ok(Command::Help),
            Some("c") | Some("channels") => Ok(Command::Channels),
            Some("j") | Some("join") => Ok(Command::Join {
                ch: parts
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("Missing channel name"))?
                    .to_string(),
            }),
            Some("p") | Some("post") => Ok(Command::Post {
                msg: parts.collect::<Vec<_>>().join(" "),
            }),
            Some("l") | Some("list") => Ok(Command::List),
            _ => bail!("Invalid command"),
        }
    }
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

        match Command::parse(command) {
            Ok(Command::Channels) => {
                let channels = self.storage.get_channels()?;
                let list = channels
                    .iter()
                    .map(|c| c.name.clone())
                    .collect::<Vec<String>>()
                    .join(",");
                return Ok(vec![list]);
            }
            Ok(Command::Join { ch }) => {
                let channels = self.storage.get_channels()?;
                let Some(channel) = channels.iter().find(|_ch| _ch.name == ch) else {
                    bail!("Channel not found");
                };
                session.current_channel = channel.cid;
                self.sessions.insert(user_pk_hash, session);
                return Ok(vec!["Ack".into()]);
            }
            Ok(Command::Post { msg }) => {
                let message = ChannelMessage {
                    cid_ts: (session.current_channel, now),
                    uid: session.user_id,
                    text: format!("{}: {}", user.short_name, msg),
                };

                self.storage.add_message(message)?;

                return Ok(vec!["Ack".into()]);
            }

            Ok(Command::List) => {
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
