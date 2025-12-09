#[allow(dead_code)]
use std::time::Instant;

use meshtastic::protobufs::routing;

#[derive(Debug, Clone)]
pub enum TextMessageStatus {
    Sent,
    Recieved,
    ImplicitAck,
    ExplicitAck,
    RoutingError(routing::Error),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TextMessage {
    pub ts: Instant,
    pub from: u32,
    pub to: u32,
    pub text: String,
    pub status: TextMessageStatus,
    pub pk_hash: [u8; 32],
}

impl TextMessage {
    pub fn sent(from: u32, to: u32, text: String) -> Self {
        Self {
            ts: Instant::now(),
            from,
            to,
            text,
            pk_hash: [0; 32],
            status: TextMessageStatus::Sent,
        }
    }
    pub fn recieved(from: u32, to: u32, text: String, pk_hash: [u8; 32]) -> Self {
        Self {
            ts: Instant::now(),
            from,
            to,
            text,
            pk_hash,
            status: TextMessageStatus::Recieved,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Destination {
    ShortName(String),
    Node(u32),
    Broadcast,
}

impl From<String> for Destination {
    fn from(short_name: String) -> Self {
        Destination::ShortName(short_name)
    }
}
impl From<&str> for Destination {
    fn from(short_name: &str) -> Self {
        Destination::ShortName(short_name.to_string())
    }
}
impl From<u32> for Destination {
    fn from(id: u32) -> Self {
        Destination::Node(id)
    }
}
