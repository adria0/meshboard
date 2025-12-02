use anyhow::Result;

pub mod in_memory;
mod test;

pub type MessageId = u32;
pub type ChannelId = u32;
pub type UserId = u32;
pub type UserPkHash = [u8; 32];

#[derive(Debug, Clone)]
pub struct User {
    // User Id
    pub uid: UserId,
    // User Id
    pub radio_userid: u32,
    // Public Key Hash
    pub pk_hash: UserPkHash,
    // Last Seen Timestamp
    pub last_ts: u64,
}

#[derive(Debug, Clone)]
pub struct Channel {
    pub cid: ChannelId,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ChannelMessage {
    pub ts: u64,
    pub uid: UserId,
    pub text: String,
}

#[async_trait::async_trait]
pub trait Storage {
    async fn add_channel(&self, name: &str) -> Result<ChannelId>;
    async fn get_channels(&self) -> Result<Vec<Channel>>;
    async fn get_channel_by_name(&self, name: &str) -> Result<Channel>;
    async fn rm_channel(&self, cid: ChannelId) -> Result<ChannelId>;

    async fn add_message(&self, channel: ChannelId, message: &ChannelMessage) -> Result<u32>;
    async fn get_messages(
        &self,
        channel: ChannelId,
        from_ts: u32,
        to_ts: u32,
    ) -> Result<Vec<ChannelMessage>>;

    async fn add_user(&self, user: &User) -> Result<UserId>;
    async fn update_user(&self, user: &User) -> Result<UserId>;
    async fn get_user_by_id(&self, uid: UserId) -> Result<User>;
    async fn get_user_by_pkhash(&self, pkhash: &[u8; 32]) -> Result<User>;
}

pub async fn test_channels<S: Storage + Send + Sync>(s: &S) -> Result<()> {
    let before = s.get_channels().await?.len();

    let c1 = s.add_channel("test-a").await?;
    let c2 = s.add_channel("test-b").await?;
    let c3 = s.add_channel("test-c").await?;

    assert!(c1 != c2 && c2 != c3 && c1 != c3);

    let all = s.get_channels().await?;
    assert!(all.len() >= before + 3);

    let a = s.get_channel_by_name("test-a").await?;
    let b = s.get_channel_by_name("test-b").await?;
    let c = s.get_channel_by_name("test-c").await?;

    assert_eq!(a.cid, c1);
    assert_eq!(b.cid, c2);
    assert_eq!(c.cid, c3);
    assert_eq!(a.name, "test-a");
    assert_eq!(b.name, "test-b");
    assert_eq!(c.name, "test-c");

    assert!(all.iter().any(|ch| ch.cid == c1));
    assert!(all.iter().any(|ch| ch.cid == c2));
    assert!(all.iter().any(|ch| ch.cid == c3));

    let _ = s.rm_channel(c2).await?;
    let all2 = s.get_channels().await?;
    assert!(!all2.iter().any(|ch| ch.cid == c2));
    assert!(all2.iter().any(|ch| ch.cid == c1));
    assert!(all2.iter().any(|ch| ch.cid == c3));

    Ok(())
}

pub async fn test_users<S: Storage + Send + Sync>(s: &S) -> Result<()> {
    let mut pk1 = [0u8; 32];
    pk1[0] = 1;
    let mut pk2 = [0u8; 32];
    pk2[0] = 2;

    let u1 = User {
        uid: 0,
        radio_userid: 10,
        pk_hash: pk1,
        last_ts: 100,
    };
    let u2 = User {
        uid: 0,
        radio_userid: 20,
        pk_hash: pk2,
        last_ts: 200,
    };

    let id1 = s.add_user(&u1).await?;
    let id2 = s.add_user(&u2).await?;
    assert!(id1 != id2);

    let r1 = s.get_user_by_id(id1).await?;
    let r2 = s.get_user_by_id(id2).await?;

    assert_eq!(r1.uid, id1);
    assert_eq!(r2.uid, id2);
    assert_eq!(r1.radio_userid, 10);
    assert_eq!(r2.radio_userid, 20);
    assert_eq!(r1.pk_hash, pk1);
    assert_eq!(r2.pk_hash, pk2);

    let r1_pk = s.get_user_by_pkhash(&pk1).await?;
    let r2_pk = s.get_user_by_pkhash(&pk2).await?;
    assert_eq!(r1_pk.uid, id1);
    assert_eq!(r2_pk.uid, id2);

    let mut u1_updated = r1.clone();
    u1_updated.last_ts = 999;
    u1_updated.radio_userid = 11;

    let upd_id = s.update_user(&u1_updated).await?;
    assert_eq!(upd_id, id1);

    let r1_after = s.get_user_by_id(id1).await?;
    assert_eq!(r1_after.last_ts, 999);
    assert_eq!(r1_after.radio_userid, 11);
    assert_eq!(r1_after.pk_hash, pk1);

    let r1_after_pk = s.get_user_by_pkhash(&pk1).await?;
    assert_eq!(r1_after_pk.uid, id1);
    assert_eq!(r1_after_pk.last_ts, 999);

    Ok(())
}

pub async fn test_messages<S: Storage + Send + Sync>(s: &S) -> Result<()> {
    let cid1 = s.add_channel("msgs-1").await?;
    let cid2 = s.add_channel("msgs-2").await?;

    let u = User {
        uid: 0,
        radio_userid: 1,
        pk_hash: [3u8; 32],
        last_ts: 0,
    };
    let uid = s.add_user(&u).await?;

    let m1 = ChannelMessage {
        ts: 10,
        uid,
        text: "hello".into(),
    };
    let m2 = ChannelMessage {
        ts: 20,
        uid,
        text: "world".into(),
    };
    let m3 = ChannelMessage {
        ts: 30,
        uid,
        text: "bye".into(),
    };
    let m4 = ChannelMessage {
        ts: 40,
        uid,
        text: "other-channel".into(),
    };

    let id1 = s.add_message(cid1, &m1).await?;
    let id2 = s.add_message(cid1, &m2).await?;
    let id3 = s.add_message(cid1, &m3).await?;
    let id4 = s.add_message(cid2, &m4).await?;

    assert!(id1 != id2 && id2 != id3 && id3 != id4);

    let all_c1 = s.get_messages(cid1, 0, u32::MAX).await?;
    assert_eq!(all_c1.len(), 3);
    assert_eq!(all_c1[0].text, "hello");
    assert_eq!(all_c1[1].text, "world");
    assert_eq!(all_c1[2].text, "bye");

    let all_c2 = s.get_messages(cid2, 0, u32::MAX).await?;
    assert_eq!(all_c2.len(), 1);
    assert_eq!(all_c2[0].text, "other-channel");

    let window = s.get_messages(cid1, 0, 15).await?;
    assert_eq!(window.len(), 1);
    assert_eq!(window[0].text, "hello");

    let window2 = s.get_messages(cid1, 15, 25).await?;
    assert_eq!(window2.len(), 1);
    assert_eq!(window2[0].text, "world");

    let window3 = s.get_messages(cid1, 25, 35).await?;
    assert_eq!(window3.len(), 1);
    assert_eq!(window3[0].text, "bye");

    let none = s.get_messages(cid1, 35, 100).await?;
    assert!(none.is_empty());

    let none_empty_channel = s.get_messages(999999, 0, u32::MAX).await?;
    assert!(none_empty_channel.is_empty());

    let _ = s.rm_channel(cid1).await?;
    let after_rm = s.get_messages(cid1, 0, u32::MAX).await?;
    assert!(after_rm.is_empty());

    Ok(())
}

pub async fn test_storage<S: Storage + Send + Sync>(s: &S) -> Result<()> {
    test_channels(s).await?;
    test_users(s).await?;
    test_messages(s).await?;
    Ok(())
}
