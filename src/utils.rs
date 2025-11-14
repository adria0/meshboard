use std::borrow::Cow;

use meshtastic::{
    Message,
    protobufs::{Data, FromRadio, MyNodeInfo, PortNum, User, from_radio, mesh_packet},
    types::NodeId,
};

#[derive(Debug)]
pub enum IncomingPacket {
    #[allow(unused)]
    MyInfo(MyNodeInfo),
    #[allow(unused)]
    RoutingApp(Data),
    NodeInfo(NodeId, User),
    TextMessage {
        from: NodeId,
        to: NodeId,
        msg: String,
    },
    #[allow(unused)]
    Other(Cow<'static, str>),
}

impl From<FromRadio> for IncomingPacket {
    fn from(from_radio: FromRadio) -> Self {
        use IncomingPacket::*;
        let Some(payload) = from_radio.payload_variant else {
            return Other(Cow::Borrowed("No payload"));
        };
        match payload {
            from_radio::PayloadVariant::MyInfo(my_node_info) => MyInfo(my_node_info),
            from_radio::PayloadVariant::NodeInfo(node_info) => {
                if let Some(user) = node_info.user {
                    NodeInfo(NodeId::new(node_info.num), user)
                } else {
                    Other(Cow::Borrowed("NodeInfo without user"))
                }
            }
            from_radio::PayloadVariant::Packet(mesh_packet) => {
                let Some(pv) = mesh_packet.payload_variant else {
                    return Other(Cow::Borrowed("Packet without content"));
                };
                let mesh_packet::PayloadVariant::Decoded(data) = pv else {
                    return Other(Cow::Borrowed("Ciphered Mesh Packet"));
                };
                match PortNum::try_from(data.portnum) {
                    Ok(PortNum::RoutingApp) => RoutingApp(data),
                    Ok(PortNum::TextMessageApp) => {
                        let msg = String::from_utf8(data.payload.clone())
                            .unwrap_or("Non-utf8 msg".into());
                        TextMessage {
                            from: NodeId::new(mesh_packet.from),
                            to: NodeId::new(mesh_packet.to),
                            msg,
                        }
                    }
                    Ok(PortNum::NodeinfoApp) => {
                        if let Ok(user) = User::decode(data.payload.as_slice()) {
                            NodeInfo(NodeId::new(mesh_packet.from), user)
                        } else {
                            Other(Cow::Borrowed("NodeInfo without user"))
                        }
                    }
                    _ => {
                        if let Ok(app) = PortNum::try_from(data.portnum) {
                            Other(Cow::Owned(format!("{:?}", app)))
                        } else {
                            Other(Cow::Owned(format!("Unknown portnum {}", data.portnum)))
                        }
                    }
                }
            }
            _ => {
                let mut info = format!("{:?}", payload);
                info.truncate(20);
                Other(Cow::Owned(info))
            }
        }
    }
}
