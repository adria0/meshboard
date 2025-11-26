use anyhow::{Result, bail};
use std::{collections::HashMap, future::pending, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{
        RwLock,
        mpsc::{UnboundedReceiver, UnboundedSender},
    },
};
use tokio_util::sync::CancellationToken;

use meshtastic::{
    Message,
    api::{ConnectedStreamApi, StreamApi, StreamHandle, state::Configured},
    packet::PacketDestination,
    protobufs::{
        Data, FromRadio, MeshPacket, MyNodeInfo, PortNum, Routing, User, from_radio,
        mesh_packet::{self, Priority},
        routing,
    },
    types::{MeshChannel, NodeId},
    utils::{
        generate_rand_id,
        stream::{BleId, build_ble_stream},
    },
};

use super::router::*;
pub use super::types::*;

use TextMessageStatus::*;

macro_rules! r {
    ($slf:ident . $field:ident) => {
        $slf.state.read().await.$field
    };
}
macro_rules! w {
    ($slf:ident . $field:ident) => {
        $slf.state.write().await.$field
    };
}

#[derive(Debug, Clone)]
pub enum Status {
    PacketCount(usize),
    NewMessage(u32),
    UpdatedMessage(u32),
}

#[derive(Default)]
pub struct HandlerState {
    my_node_info: Option<MyNodeInfo>,
    nodes: HashMap<u32, User>,
    messages: HashMap<u32, TextMessage>,
}

pub type State = Arc<RwLock<HandlerState>>;

pub struct Handler {
    pub state: State,
    pub cancel: CancellationToken,
    pub msg_tx: UnboundedSender<TextMessage>,
    pub status_rx: UnboundedReceiver<Status>,
}

pub struct Service {
    state: State,
    cancel: CancellationToken,
    packet_rx: UnboundedReceiver<FromRadio>,
    stream_api: ConnectedStreamApi<Configured>,
    msg_rx: UnboundedReceiver<TextMessage>,
    status_tx: UnboundedSender<Status>,
}

impl Handler {
    pub async fn long_name_of(&self, user_id: u32) -> Option<String> {
        self.state
            .read()
            .await
            .nodes
            .get(&user_id)
            .map(|user| user.long_name.clone())
    }

    pub async fn format_msg(&self, msg: &TextMessage) -> String {
        let me = r!(self.my_node_info).as_ref().unwrap().my_node_num;
        let name = async |id| {
            self.long_name_of(id)
                .await
                .unwrap_or(format!("NodeId({})", id))
        };

        let status = match msg.status {
            Sent => "📤".into(),
            Recieved => "📥".into(),
            ImplicitAck => "✔️".into(),
            ExplicitAck => "✔️✔️".into(),
            RoutingError(error) => format!("❌ {:?}", error),
        };

        if msg.to == 0xffffffff {
            format!("💬 {} : {} {} ", name(msg.from).await, msg.text, status)
        } else if msg.to == me {
            format!("👤 {} : {} {}", name(msg.from).await, msg.text, status)
        } else {
            format!(
                "📩 {} → {} : {} {}",
                name(msg.from).await,
                name(msg.to).await,
                msg.text,
                status
            )
        }
    }

    pub async fn msg(&self, id: u32) -> Option<TextMessage> {
        self.state.read().await.messages.get(&id).cloned()
    }

    pub async fn me(&self) -> u32 {
        r!(self.my_node_info).as_ref().unwrap().my_node_num
    }

    pub async fn send_text<T: Into<String>, D: Into<Destination>>(
        &self,
        text: T,
        to: D,
    ) -> Result<()> {
        let from = r!(self.my_node_info).as_ref().unwrap().my_node_num;
        let to = match to.into() {
            Destination::Node(node_num) => node_num,
            Destination::Broadcast => 0xffffffff,
            Destination::ShortName(short_name) => {
                let mut id = None;
                for (node_id, node) in &r!(self.nodes) {
                    if node.short_name == short_name {
                        id = Some(*node_id);
                        break;
                    }
                }
                let Some(id) = id else {
                    bail!("Node '{short_name}' not found")
                };
                id
            }
        };
        self.msg_tx.send(TextMessage::sent(from, to, text.into()))?;
        Ok(())
    }
}

impl Service {
    pub async fn from_ble(ble_device: &str) -> Result<(Handler, Service)> {
        let ble_stream =
            build_ble_stream(&BleId::from_name(&ble_device), Duration::from_secs(5)).await?;
        let (handler, service) = Self::build(ble_stream).await?;
        Ok((handler, service))
    }

    async fn build<S>(stream_handle: StreamHandle<S>) -> Result<(Handler, Service)>
    where
        S: AsyncReadExt + AsyncWriteExt + Send + 'static,
    {
        let stream_api = StreamApi::new();
        let config_id = generate_rand_id();

        let (packet_rx, stream_api) = stream_api.connect(stream_handle).await;
        let stream_api = stream_api.configure(config_id).await?;

        let (status_tx, status_rx) = tokio::sync::mpsc::unbounded_channel::<Status>();
        let (msg_tx, msg_rx) = tokio::sync::mpsc::unbounded_channel::<TextMessage>();

        let state = Arc::new(RwLock::new(HandlerState::default()));

        let cancel = CancellationToken::new();

        let handler = Handler {
            state: state.clone(),
            cancel: cancel.clone(),
            msg_tx,
            status_rx,
        };

        let service = Service {
            state,
            cancel,
            packet_rx,
            stream_api,
            msg_rx,
            status_tx,
        };

        Ok((handler, service))
    }
    pub async fn start(mut self) -> Result<()> {
        let mut buffer_flushed = false;
        self.status_tx.send(Status::PacketCount(0))?;
        let mut packet_count = 0;
        loop {
            tokio::select! {
                from_radio = self.packet_rx.recv() => {
                    packet_count += 1;
                    let Some(from_radio) = from_radio else {
                        bail!("BLE stream closed");
                    };
                    self.process_from_radio(from_radio).await?;
                }
                msg = async {
                    if buffer_flushed {
                        self.msg_rx.recv().await
                    } else {
                        pending().await
                    }
                } => {
                    let Some(msg) = msg else {
                        bail!("Text message stream closed");
                    };
                    self.process_send_text(msg).await?;
                }
                _ = tokio::time::sleep(Duration::from_millis(1000)) => {
                    if !buffer_flushed {
                        buffer_flushed = true;
                    }
                    self.status_tx.send(Status::PacketCount(packet_count))?;
                }
                _ = self.cancel.cancelled() => {
                    break;
                }
            }
        }
        Ok(())
    }

    async fn process_send_text(&mut self, msg: TextMessage) -> Result<()> {
        let from = r!(self.my_node_info).as_ref().unwrap().my_node_num;
        let mut packet_router = Router::new(NodeId::new(from));
        self.stream_api
            .send_text(
                &mut packet_router,
                msg.text.clone(),
                PacketDestination::Node(NodeId::new(msg.to)),
                true,
                MeshChannel::new(0).unwrap(),
            )
            .await?;
        let id = packet_router.last_sent().unwrap().id;
        w!(self.messages).insert(id, msg);
        self.status_tx.send(Status::NewMessage(id))?;

        Ok(())
    }

    async fn process_from_radio(&self, from_radio: FromRadio) -> Result<()> {
        let Some(payload) = from_radio.payload_variant else {
            bail!("No payload");
        };
        match payload {
            // Load for information about my node
            from_radio::PayloadVariant::MyInfo(node_info) => {
                w!(self.my_node_info) = Some(node_info);
            }
            // Local for the data in NodeDB
            from_radio::PayloadVariant::NodeInfo(node_info) if node_info.user.is_some() => {
                w!(self.nodes).insert(node_info.num, node_info.user.unwrap());
            }
            // Mesh packet loaded
            from_radio::PayloadVariant::Packet(mesh_packet) => {
                if let Some(mesh_packet::PayloadVariant::Decoded(ref data)) =
                    mesh_packet.payload_variant
                {
                    match PortNum::try_from(data.portnum) {
                        Ok(PortNum::NodeinfoApp) => {
                            self.handle_nodeinfo(&mesh_packet, data).await?
                        }
                        Ok(PortNum::TextMessageApp) => {
                            self.handle_textmessage(&mesh_packet, data).await?
                        }
                        Ok(PortNum::RoutingApp) => self.handle_routing(&mesh_packet, &data).await?,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_nodeinfo(&self, mesh_packet: &MeshPacket, data: &Data) -> Result<()> {
        let user = User::decode(data.payload.as_slice())?;
        w!(self.nodes).insert(mesh_packet.from, user);
        Ok(())
    }

    async fn handle_textmessage(&self, mesh_packet: &MeshPacket, data: &Data) -> Result<()> {
        let msg = String::from_utf8(data.payload.clone())?;
        w!(self.messages).insert(
            mesh_packet.id,
            TextMessage::recieved(mesh_packet.from, mesh_packet.to, msg),
        );
        self.status_tx.send(Status::NewMessage(mesh_packet.id))?;

        Ok(())
    }

    async fn handle_routing(&self, mesh_packet: &MeshPacket, data: &Data) -> Result<()> {
        let Routing { variant } = Routing::decode(data.payload.as_slice())?;
        let Some(routing::Variant::ErrorReason(routing_error)) = variant else {
            return Ok(());
        };
        let mut status = None;

        if routing_error != routing::Error::None as i32 {
            status = Some(RoutingError(routing::Error::try_from(routing_error)?));
        } else if mesh_packet.from == mesh_packet.to && mesh_packet.priority == Priority::Ack as i32
        {
            status = Some(ImplicitAck);
        } else if mesh_packet.from != mesh_packet.to {
            status = Some(ExplicitAck);
        }

        if let Some(msg) = w!(self.messages).get_mut(&data.request_id)
            && let Some(status) = status
        {
            msg.status = status;
            self.status_tx
                .send(Status::UpdatedMessage(data.request_id))?;
        }

        Ok(())
    }
}
