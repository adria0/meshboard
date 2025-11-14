use meshtastic::{protobufs::User, types::NodeId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Storage {
    pub nodes: HashMap<u32, User>,
}

impl Storage {
    pub fn insert_node(&mut self, node_id: NodeId, user: User) {
        self.nodes.insert(node_id.id(), user);
    }

    pub fn long_name_of(&self, node_id: NodeId) -> String {
        if let Some(user) = self.nodes.get(&node_id.id()) {
            user.long_name.clone()
        } else {
            format!("{}", node_id)
        }
    }
}
