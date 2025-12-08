use anyhow::Result;
use native_db::Builder;
use native_db::Database;
use native_db::Key;
use native_db::Models;
use native_db::ToKey;
use native_db::native_db;
use native_model::Model;
use native_model::native_model;
use serde::Deserialize;
use serde::Serialize;

use std::path::Path;
use std::sync::OnceLock;

static MODELS: OnceLock<Models> = OnceLock::new();

fn models() -> &'static Models {
    MODELS.get_or_init(|| {
        let mut models = Models::new();

        models.define::<User>().unwrap();
        models.define::<Channel>().unwrap();
        models.define::<ChannelMessage>().unwrap();
        models
    })
}

pub type MessageId = u32;
pub type ChannelId = u32;
pub type UserId = u32;

#[derive(Clone, Serialize, Deserialize, Default, PartialEq, Eq, Debug, Hash)]
pub struct UserPkHash(pub [u8; 32]);

impl ToKey for UserPkHash {
    fn to_key(&self) -> Key {
        Key::new(self.0.to_vec())
    }

    fn key_names() -> Vec<String> {
        vec!["pk_hash".to_string()]
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, Eq)]
#[native_model(id = 1, version = 1)]
#[native_db]
pub struct User {
    // User Id
    #[primary_key]
    pub uid: UserId,
    // Public Key Hash
    #[secondary_key(unique)]
    pub pk_hash: UserPkHash,
    // User Id
    pub short_name: String,
    // Last Seen Timestamp
    pub last_ts: u64,
}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
#[native_model(id = 2, version = 1)]
#[native_db]
pub struct Channel {
    #[primary_key]
    pub cid: ChannelId,
    pub name: String,
}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
#[native_model(id = 3, version = 1)]
#[native_db]
pub struct ChannelMessage {
    #[primary_key]
    pub cid_ts: (ChannelId, u64),
    pub uid: UserId,
    pub text: String,
}

pub struct Storage {
    db: Database<'static>,
}

impl Storage {
    pub fn memory() -> Self {
        let db = Builder::new().create_in_memory(models()).unwrap();
        Self { db }
    }
    pub fn open(path: &Path) -> Result<Self> {
        let db = Builder::new().create(models(), path)?;
        Ok(Self { db })
    }
    pub fn new(db: Database<'static>) -> Self {
        Self { db }
    }
    pub fn add_channel(&self, name: &str) -> Result<u32> {
        let rw = self.db.rw_transaction()?;
        let cid = rw.len().primary::<Channel>()? as u32;
        let channel = Channel {
            cid: cid,
            name: name.into(),
        };

        rw.insert(channel)?;
        rw.commit()?;
        Ok(cid)
    }

    pub fn get_channels(&self) -> Result<Vec<Channel>> {
        let r = self.db.r_transaction()?;
        let mut channels: Vec<Channel> = Vec::new();
        for ch in r.scan().primary()?.all()? {
            channels.push(ch?);
        }

        Ok(channels)
    }

    pub fn add_message(&self, message: ChannelMessage) -> Result<u32> {
        let rw = self.db.rw_transaction()?;
        rw.insert(message)?;
        rw.commit()?;
        Ok(0)
    }

    pub fn get_messages(
        &self,
        channel_id: u32,
        ts_start: u64,
        ts_end: u64,
    ) -> Result<Vec<ChannelMessage>> {
        let r = self.db.r_transaction()?;
        let mut messages: Vec<ChannelMessage> = Vec::new();
        for msg in r
            .scan()
            .primary()?
            .range((channel_id, ts_start)..(channel_id, ts_end))?
        {
            messages.push(msg?);
        }

        Ok(messages)
    }

    pub fn add_user(&self, mut user: User) -> Result<UserId> {
        let rw = self.db.rw_transaction()?;
        let user_id = rw.len().primary::<User>()? as u32;
        user.uid = user_id;
        rw.insert(user)?;
        rw.commit()?;
        Ok(user_id)
    }

    pub fn update_user(&self, user_id: UserId, user: User) -> Result<u32> {
        let rw = self.db.rw_transaction()?;
        let old_user = self.get_user_by_id(user_id)?;
        rw.update(old_user, user)?;
        rw.commit()?;
        Ok(0)
    }

    pub fn get_user_by_id(&self, id: u32) -> Result<User> {
        let r = self.db.r_transaction()?;
        let user: User = r
            .get()
            .primary(id)?
            .ok_or(anyhow::anyhow!("User not found"))?;
        Ok(user)
    }

    pub fn get_user_by_pkhash(&self, pk_hash: UserPkHash) -> Result<User> {
        let r = self.db.r_transaction()?;
        let user: User = r
            .get()
            .secondary(UserKey::pk_hash, pk_hash)?
            .ok_or(anyhow::anyhow!("User not found"))?;
        Ok(user)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_channels() -> anyhow::Result<()> {
        let db = Builder::new().create_in_memory(models())?;
        let s = Storage::new(db);

        // Test channels
        let cid0 = s.add_channel("talk")?;
        let cid1 = s.add_channel("news")?;
        let channels = s.get_channels()?;
        assert_eq!(channels[0].cid, cid0);
        assert_eq!(channels[0].name, "talk");
        assert_eq!(channels[1].cid, cid1);
        assert_eq!(channels[1].name, "news");

        Ok(())
    }

    #[test]
    fn test_users() -> anyhow::Result<()> {
        let db = Builder::new().create_in_memory(models())?;
        let s = Storage::new(db);

        // Test users
        let mut user0 = User {
            uid: 0,
            short_name: "user0".to_string(),
            pk_hash: UserPkHash([7u8; 32]),
            last_ts: 0,
        };
        user0.uid = s.add_user(user0.clone())?;
        assert_eq!(user0, s.get_user_by_id(user0.uid)?);

        let mut user1 = User {
            uid: 0,
            short_name: "user1".to_string(),
            pk_hash: UserPkHash([8u8; 32]),
            last_ts: 99,
        };
        user1.uid = s.add_user(user1.clone())?;
        assert_eq!(user1, s.get_user_by_id(user1.uid)?);

        assert_eq!(user0, s.get_user_by_pkhash(UserPkHash([7u8; 32]))?);
        assert_eq!(user1, s.get_user_by_pkhash(UserPkHash([8u8; 32]))?);

        user0.last_ts = 778;
        s.update_user(user0.uid, user0.clone())?;
        assert_eq!(user0, s.get_user_by_id(user0.uid)?);

        Ok(())
    }

    #[test]
    fn test_messages() -> anyhow::Result<()> {
        let db = Builder::new().create_in_memory(models())?;
        let s = Storage::new(db);

        let mkmsg = |cid, ts| ChannelMessage {
            cid_ts: (cid, ts),
            uid: 1,
            text: format!("{cid}{ts}"),
        };

        let msg1 = mkmsg(0, 1);
        s.add_message(msg1.clone())?;
        let msg2 = mkmsg(0, 2);
        s.add_message(msg2.clone())?;
        let msg3 = mkmsg(0, 3);
        s.add_message(msg3.clone())?;
        let msg4 = mkmsg(1, 4);
        s.add_message(msg4.clone())?;
        let msg5 = mkmsg(1, 5);
        s.add_message(msg5.clone())?;

        assert_eq!(
            s.get_messages(0, 1, 4)?,
            vec![msg1.clone(), msg2.clone(), msg3.clone()]
        );
        assert_eq!(s.get_messages(0, 2, 4)?, vec![msg2.clone(), msg3.clone()]);
        assert_eq!(s.get_messages(0, 1, 3)?, vec![msg1.clone(), msg2.clone()]);

        assert_eq!(s.get_messages(1, 4, 6)?, vec![msg4.clone(), msg5.clone()]);

        Ok(())
    }
}
