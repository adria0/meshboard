use std::convert::Infallible;

use meshtastic::{
    packet::PacketRouter,
    protobufs::{FromRadio, MeshPacket},
    types::NodeId,
};

pub struct Router {
    last_sent: Option<MeshPacket>,
    node_id: NodeId,
}

impl Router {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            last_sent: None,
            node_id,
        }
    }
    pub fn last_sent(&mut self) -> Option<MeshPacket> {
        self.last_sent.take()
    }
}

impl PacketRouter<(), Infallible> for Router {
    fn handle_packet_from_radio(&mut self, _packet: FromRadio) -> Result<(), Infallible> {
        Ok(())
    }
    fn handle_mesh_packet(&mut self, packet: MeshPacket) -> Result<(), Infallible> {
        self.last_sent = Some(packet);
        Ok(())
    }
    fn source_node_id(&self) -> NodeId {
        self.node_id
    }
}
