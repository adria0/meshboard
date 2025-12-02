use std::{collections::HashMap, sync::Mutex};

use crate::bbs::storage::{Channel, ChannelId, ChannelMessage, MessageId, Storage, User, UserId};
use anyhow::Result;

pub struct InMemoryStorage {
    inner: Mutex<Inner>,
}

struct Inner {
    next_cid: ChannelId,
    next_mid: MessageId,
    next_uid: UserId,
    channels: HashMap<ChannelId, Channel>,
    messages: HashMap<ChannelId, Vec<(MessageId, ChannelMessage)>>,
    users: HashMap<UserId, User>,
    users_by_pk: HashMap<[u8; 32], UserId>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                next_cid: 1,
                next_mid: 1,
                next_uid: 1,
                channels: HashMap::new(),
                messages: HashMap::new(),
                users: HashMap::new(),
                users_by_pk: HashMap::new(),
            }),
        }
    }
}

#[async_trait::async_trait]
impl Storage for InMemoryStorage {
    async fn add_channel(&self, name: &str) -> Result<ChannelId> {
        let mut i = self.inner.lock().unwrap();
        let cid = i.next_cid;
        i.next_cid += 1;
        i.channels.insert(
            cid,
            Channel {
                cid,
                name: name.to_string(),
            },
        );
        Ok(cid)
    }

    async fn get_channels(&self) -> Result<Vec<Channel>> {
        let i = self.inner.lock().unwrap();
        Ok(i.channels.values().cloned().collect())
    }

    async fn get_channel_by_name(&self, name: &str) -> Result<Channel> {
        let i = self.inner.lock().unwrap();
        Ok(i.channels
            .values()
            .find(|c| c.name == name)
            .cloned()
            .unwrap())
    }

    async fn rm_channel(&self, cid: ChannelId) -> Result<ChannelId> {
        let mut i = self.inner.lock().unwrap();
        i.channels.remove(&cid);
        i.messages.remove(&cid);
        Ok(cid)
    }

    async fn add_message(&self, channel: ChannelId, message: &ChannelMessage) -> Result<u32> {
        let mut i = self.inner.lock().unwrap();
        let mid = i.next_mid;
        i.next_mid += 1;
        i.messages
            .entry(channel)
            .or_default()
            .push((mid, message.clone()));
        Ok(mid)
    }

    async fn get_messages(
        &self,
        channel: ChannelId,
        from_ts: u32,
        to_ts: u32,
    ) -> Result<Vec<ChannelMessage>> {
        let i = self.inner.lock().unwrap();
        Ok(i.messages
            .get(&channel)
            .map(|v| {
                v.iter()
                    .filter(|(_, m)| m.ts >= from_ts as u64 && m.ts <= to_ts as u64)
                    .map(|(_, m)| m.clone())
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn add_user(&self, user: &User) -> Result<UserId> {
        let mut i = self.inner.lock().unwrap();
        let uid = i.next_uid;
        i.next_uid += 1;
        let mut u = user.clone();
        u.uid = uid;
        i.users.insert(uid, u.clone());
        i.users_by_pk.insert(u.pk_hash, uid);
        Ok(uid)
    }

    async fn update_user(&self, user: &User) -> Result<UserId> {
        let mut i = self.inner.lock().unwrap();
        i.users.insert(user.uid, user.clone());
        i.users_by_pk.insert(user.pk_hash, user.uid);
        Ok(user.uid)
    }

    async fn get_user_by_id(&self, uid: UserId) -> Result<User> {
        let i = self.inner.lock().unwrap();
        Ok(i.users.get(&uid).cloned().unwrap())
    }

    async fn get_user_by_pkhash(&self, pkhash: &[u8; 32]) -> Result<User> {
        let i = self.inner.lock().unwrap();
        let uid = *i.users_by_pk.get(pkhash).unwrap();
        Ok(i.users.get(&uid).cloned().unwrap())
    }
}

#[tokio::test]
async fn test_inmemory() -> Result<()> {
    super::test_storage(&InMemoryStorage::new()).await
}
