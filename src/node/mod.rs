use async_channel::{unbounded, Receiver, Sender};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as b64, Engine};
use derive_builder::Builder;
use futures::StreamExt;
use libp2p::{
    autonat, identify,
    identity::Keypair,
    kad, mdns, noise, ping, relay,
    swarm::{self, SwarmEvent},
    tcp, upnp, yamux, Multiaddr, PeerId, Stream, StreamProtocol, Swarm, SwarmBuilder,
};
use libp2p_stream as stream;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::{
    fmt::Debug,
    str::FromStr,
    sync::{Arc, Mutex, MutexGuard},
    thread::{spawn, JoinHandle},
    time::Duration,
    u64,
};

pub mod error;
pub use error::{ClientError, Error, NetworkingError, PeaResult, ProcessError};

pub mod comm;
pub use comm::{Command, CommandType, Event};

#[derive(swarm::NetworkBehaviour)]
pub struct NodeBehavior {
    ping: ping::Behaviour,
    upnp: upnp::tokio::Behaviour,
    mdns: mdns::tokio::Behaviour,
    autonat: autonat::Behaviour,
    stream: stream::Behaviour,
    relay: relay::client::Behaviour,
    identify: identify::Behaviour,
    kad: kad::Behaviour<kad::store::MemoryStore>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeInfo {
    pub key: String,
    pub protocol: String,
    pub port: u16,
    pub identity: Option<Value>,
    pub bootstrap_nodes: Vec<BootstrapNode>,
}

#[derive(Clone, Debug)]
pub struct NodeEventListener(Receiver<Event>);

impl NodeEventListener {
    pub fn new(rec: Receiver<Event>) -> Self {
        NodeEventListener(rec)
    }

    pub fn try_next(&self) -> Option<Event> {
        match self.0.try_recv() {
            Ok(v) => Some(v),
            Err(_) => None,
        }
    }
}

impl Iterator for NodeEventListener {
    type Item = Event;
    fn next(&mut self) -> Option<Self::Item> {
        match self.0.recv_blocking() {
            Ok(v) => Some(v),
            Err(_) => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BootstrapNode {
    Boot(String, Multiaddr),
    Relay(String, Multiaddr),
    Other(String, Multiaddr),
}

impl BootstrapNode {
    pub fn relay<I: AsRef<str>, T: AsRef<str>>(peer_id: I, address: T) -> PeaResult<Self> {
        match PeerId::from_str(peer_id.as_ref()) {
            Ok(id) => match address.as_ref().to_string().parse::<Multiaddr>() {
                Ok(addr) => Ok(BootstrapNode::Relay(id.to_string(), addr)),
                Err(e) => ClientError::DecodingError(e.to_string()).wrap(),
            },
            Err(e) => ClientError::DecodingError(e.to_string()).wrap(),
        }
    }

    pub fn boot<I: AsRef<str>, T: AsRef<str>>(peer_id: I, address: T) -> PeaResult<Self> {
        match PeerId::from_str(peer_id.as_ref()) {
            Ok(id) => match address.as_ref().to_string().parse::<Multiaddr>() {
                Ok(addr) => Ok(BootstrapNode::Boot(id.to_string(), addr)),
                Err(e) => ClientError::DecodingError(e.to_string()).wrap(),
            },
            Err(e) => ClientError::DecodingError(e.to_string()).wrap(),
        }
    }

    pub fn other<I: AsRef<str>, T: AsRef<str>>(peer_id: I, address: T) -> PeaResult<Self> {
        match PeerId::from_str(peer_id.as_ref()) {
            Ok(id) => match address.as_ref().to_string().parse::<Multiaddr>() {
                Ok(addr) => Ok(BootstrapNode::Other(id.to_string(), addr)),
                Err(e) => ClientError::DecodingError(e.to_string()).wrap(),
            },
            Err(e) => ClientError::DecodingError(e.to_string()).wrap(),
        }
    }

    pub fn address(&self) -> Multiaddr {
        match self {
            BootstrapNode::Boot(_, addr) => addr.clone(),
            BootstrapNode::Relay(_, addr) => addr.clone(),
            BootstrapNode::Other(_, addr) => addr.clone(),
        }
    }

    pub fn id(&self) -> PeerId {
        match self {
            BootstrapNode::Boot(id, _) => {
                PeerId::from_str(id.as_str()).expect("Invalid PeerId value in protected location.")
            }
            BootstrapNode::Relay(id, _) => {
                PeerId::from_str(id.as_str()).expect("Invalid PeerId value in protected location.")
            }
            BootstrapNode::Other(id, _) => {
                PeerId::from_str(id.as_str()).expect("Invalid PeerId value in protected location.")
            }
        }
    }

    pub fn is_relay(&self) -> bool {
        if let BootstrapNode::Relay(_, _) = self {
            true
        } else {
            false
        }
    }

    pub fn is_boot(&self) -> bool {
        if let BootstrapNode::Boot(_, _) = self {
            true
        } else {
            false
        }
    }

    pub fn is_other(&self) -> bool {
        if let BootstrapNode::Other(_, _) = self {
            true
        } else {
            false
        }
    }
}

#[derive(Clone, Builder)]
pub struct Node<T: Serialize + DeserializeOwned + Clone + Debug + Send + Sync = Value> {
    #[builder(default = "Keypair::generate_ed25519()")]
    pub key: Keypair,

    #[builder(default = "\"/p2pea/generic/0.1.0\".to_string()")]
    pub protocol: String,

    #[builder(default = "0")]
    pub port: u16,

    #[builder(default = "None")]
    pub identity: Option<T>,

    #[builder(default = "Vec::new()", setter(custom))]
    pub bootstrap_nodes: Vec<BootstrapNode>,

    #[builder(setter(skip))]
    swarm: Arc<Mutex<Option<swarm::Swarm<NodeBehavior>>>>,

    #[builder(setter(skip))]
    commands: Option<Sender<Command>>,

    #[builder(setter(skip))]
    events: Option<Receiver<Event>>,

    #[builder(setter(skip))]
    handle: Arc<Mutex<Option<JoinHandle<PeaResult<()>>>>>,
}

impl<T: Serialize + DeserializeOwned + Clone + Debug + Send + Sync + 'static> NodeBuilder<T> {
    pub fn bootstrap(&mut self, address: BootstrapNode) -> &mut Self {
        if self.bootstrap_nodes.is_some() {
            self.bootstrap_nodes.as_mut().unwrap().push(address);
        } else {
            self.bootstrap_nodes = Some(vec![address]);
        }
        self
    }

    pub fn default_bootstrap(&mut self) -> &mut Self {
        self.bootstrap(
            BootstrapNode::boot(
                "QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
                "/dnsaddr/bootstrap.libp2p.io",
            )
            .expect("Invalid default boot node"),
        )
        .bootstrap(
            BootstrapNode::boot(
                "QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa",
                "/dnsaddr/bootstrap.libp2p.io",
            )
            .expect("Invalid default boot node"),
        )
        .bootstrap(
            BootstrapNode::boot(
                "QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb",
                "/dnsaddr/bootstrap.libp2p.io",
            )
            .expect("Invalid default boot node"),
        )
        .bootstrap(
            BootstrapNode::boot(
                "QmcZf59bWwK5XFi76CZX8cbJ4BhTzzA3gU1ZjYZcYW3dwt",
                "/dnsaddr/bootstrap.libp2p.io",
            )
            .expect("Invalid default boot node"),
        )
    }
}

impl<T: Serialize + DeserializeOwned + Clone + Debug + Send + Sync + 'static> Node<T> {
    pub fn info(&self) -> NodeInfo {
        NodeInfo {
            key: b64.encode(
                self.key
                    .to_protobuf_encoding()
                    .expect("Keypair could not be serialized."),
            ),
            protocol: self.protocol.clone(),
            port: self.port.clone(),
            identity: self.identity.clone().and_then(|value| {
                Some(serde_json::to_value(value).expect("Unable to serialize expected value"))
            }),
            bootstrap_nodes: self.bootstrap_nodes.clone(),
        }
    }

    pub fn from_info(info: NodeInfo) -> PeaResult<Node<T>> {
        Ok(Node::<T> {
            key: match b64.decode(info.key.clone()) {
                Ok(v) => Keypair::from_protobuf_encoding(v.as_slice()).or(Err(Error::Client(
                    ClientError::DecodingError("Keypair decoding failed".to_string()),
                ))),
                Err(_) => Err(Error::Client(ClientError::DecodingError(
                    "Base64 decoding failed".to_string(),
                ))),
            }?,
            protocol: info.protocol.clone(),
            port: info.port,
            identity: match info.identity {
                Some(identity) => {
                    Some(serde_json::from_value::<T>(identity).or(Err(Error::Client(
                        ClientError::DecodingError("Base64 decoding failed".to_string()),
                    )))?)
                }
                None => None,
            },
            bootstrap_nodes: info.bootstrap_nodes.clone(),
            swarm: Arc::new(Mutex::new(None)),
            commands: None,
            events: None,
            handle: Arc::new(Mutex::new(None)),
        })
    }

    pub fn peer_id(&self) -> PeerId {
        self.key.public().to_peer_id()
    }

    pub fn id(&self) -> String {
        self.peer_id().to_string()
    }

    fn swarm(&self) -> PeaResult<MutexGuard<Option<Swarm<NodeBehavior>>>> {
        self.swarm.lock().or(ProcessError::SyncError.wrap())
    }

    fn thread_handle(&self) -> PeaResult<MutexGuard<Option<JoinHandle<PeaResult<()>>>>> {
        self.handle.lock().or(ProcessError::SyncError.wrap())
    }

    async fn handle_event(
        &self,
        event: SwarmEvent<NodeBehaviorEvent>,
        sender: Sender<Event>,
        swarm: &mut swarm::Swarm<NodeBehavior>,
    ) -> PeaResult<()> {
        match event {
            SwarmEvent::NewListenAddr {
                listener_id,
                address,
            } => {
                Event::Listener(comm::ListenerEvent::NewAddress {
                    id: listener_id,
                    address,
                })
                .send(sender)
                .await
            }
            SwarmEvent::ExpiredListenAddr {
                listener_id,
                address,
            } => {
                Event::Listener(comm::ListenerEvent::ExpiredAddress {
                    id: listener_id,
                    address,
                })
                .send(sender)
                .await
            }
            SwarmEvent::ListenerClosed {
                listener_id,
                addresses,
                reason,
            } => {
                Event::Listener(comm::ListenerEvent::Closed {
                    id: listener_id,
                    addresses,
                    reason: match reason {
                        Ok(_) => None,
                        Err(e) => Some(e.to_string()),
                    },
                })
                .send(sender)
                .await
            }
            SwarmEvent::ListenerError { listener_id, error } => {
                Event::Listener(comm::ListenerEvent::Error {
                    id: listener_id,
                    reason: error.to_string(),
                })
                .send(sender)
                .await
            }
            SwarmEvent::ExternalAddrConfirmed { address } => {
                Event::Address(comm::AddressEvent::Confirmed(address))
                    .send(sender)
                    .await
            }
            SwarmEvent::ExternalAddrExpired { address } => {
                Event::Address(comm::AddressEvent::Expired(address))
                    .send(sender)
                    .await
            }
            SwarmEvent::Dialing {
                peer_id,
                connection_id,
            } => {
                Event::Network(comm::NetworkEvent::Dialing {
                    peer_id,
                    connection_id,
                })
                .send(sender)
                .await
            }
            SwarmEvent::ConnectionEstablished {
                peer_id,
                connection_id,
                endpoint,
                num_established,
                concurrent_dial_errors: _,
                established_in: _,
            } => {
                Event::Network(comm::NetworkEvent::ConnectionOpened {
                    peer_id,
                    connection_id,
                    endpoint,
                    count: num_established,
                })
                .send(sender)
                .await
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                connection_id,
                endpoint,
                num_established,
                cause,
            } => {
                Event::Network(comm::NetworkEvent::ConnectionClosed {
                    peer_id,
                    connection_id,
                    endpoint,
                    count: num_established,
                    reason: cause.and_then(|c| Some(c.to_string())),
                })
                .send(sender)
                .await
            }
            SwarmEvent::NewExternalAddrOfPeer { peer_id, address } => {
                Event::Network(comm::NetworkEvent::PeerAddress { peer_id, address })
                    .send(sender)
                    .await
            }
            _ => Ok(()),
        }
    }

    async fn handle_command(
        &self,
        command: Command,
        swarm: &mut swarm::Swarm<NodeBehavior>,
    ) -> PeaResult<()> {
        Ok(())
    }

    async fn handle_stream(
        &self,
        peer: PeerId,
        stream: Stream,
        sender: Sender<Event>,
        swarm: &mut swarm::Swarm<NodeBehavior>,
    ) -> PeaResult<()> {
        Ok(())
    }

    async fn run(&self, commands: Receiver<Command>, events: Sender<Event>) -> PeaResult<()> {
        if let Ok(mut swarm_container) = self.swarm() {
            if swarm_container.is_some() {
                let swarm = swarm_container.as_mut().unwrap();

                for addr in self.bootstrap_nodes.clone() {
                    if addr.is_boot() {
                        swarm.behaviour_mut().kad.add_address(&addr.id(), addr.address());
                    } else {
                        swarm.add_peer_address(addr.id(), addr.address());
                    }
                }

                let mut comm = Box::pin(commands);
                let mut streams = swarm
                    .behaviour_mut()
                    .stream
                    .new_control()
                    .accept(
                        StreamProtocol::try_from_owned(self.protocol.clone())
                            .or(ClientError::ProtocolError.wrap())?,
                    )
                    .or(NetworkingError::InitializationError(
                        "Failed to start stream handler".to_string(),
                    )
                    .wrap())?;
                let listener = swarm
                    .listen_on(
                        format!("/ip4/0.0.0.0/tcp/{}", self.port)
                            .parse::<Multiaddr>()
                            .unwrap(),
                    )
                    .or(NetworkingError::InitializationError(
                        "Failed to start listener".to_string(),
                    )
                    .wrap())?;
                loop {
                    let result = tokio::select! {
                        event = swarm.select_next_some() => {self.handle_event(event, events.clone(), swarm).await},
                        command = comm.next() => {match command {
                            Some(c) => self.handle_command(c, swarm).await,
                            None => ProcessError::Closed.wrap()
                        }},
                        stream = streams.next() => match stream {
                            Some((peer, strm)) => self.handle_stream(peer, strm, events.clone(), swarm).await,
                            None => ProcessError::Closed.wrap()
                        }
                    };
                    if let Err(error) = result {
                        let output = match error {
                            Error::Process(ProcessError::Closed) => Ok(()),
                            e => Err(e),
                        };
                        swarm.remove_listener(listener);
                        return output;
                    }
                }
            } else {
                ProcessError::LogicError("Swarm is unexpected None".to_string()).wrap()
            }
        } else {
            ProcessError::SyncError.wrap()
        }
    }

    pub fn start(&mut self) -> PeaResult<()> {
        let swarm = SwarmBuilder::with_existing_identity(self.key.clone())
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )
            .or(NetworkingError::SwarmCreation("Initial build failure".to_string()).wrap())?
            .with_relay_client(noise::Config::new, yamux::Config::default)
            .or(NetworkingError::SwarmCreation("Relay transport setup failure".to_string()).wrap())?
            .with_behaviour(|key, relay| {
                let mut kcfg = kad::Config::new(StreamProtocol::try_from_owned(self.protocol.clone()).expect("Unhandled invalid protocol"));
                kcfg.set_query_timeout(Duration::from_secs(5 * 60));
                Ok(NodeBehavior {
                    ping: ping::Behaviour::default(),
                    upnp: upnp::tokio::Behaviour::default(),
                    mdns: mdns::tokio::Behaviour::new(mdns::Config::default(), self.peer_id())
                        .unwrap(),
                    autonat: autonat::Behaviour::new(self.peer_id(), autonat::Config::default()),
                    stream: stream::Behaviour::default(),
                    relay,
                    identify: identify::Behaviour::new(identify::Config::new(
                        self.protocol.clone(),
                        key.public(),
                    )),
                    kad: kad::Behaviour::with_config(
                        self.peer_id(),
                        kad::store::MemoryStore::new(self.peer_id()),
                        kcfg,
                    ),
                })
            })
            .or(NetworkingError::SwarmCreation("Behaviour definition failure".to_string()).wrap())?
            .with_swarm_config(|cfg| {
                cfg.with_idle_connection_timeout(Duration::from_secs(u64::MAX))
            })
            .build();

        let (com_send, com_recv) = unbounded::<Command>();
        let (evt_send, evt_recv) = unbounded::<Event>();

        let _ = self.swarm()?.insert(swarm);
        self.commands = Some(com_send);
        self.events = Some(evt_recv);

        let cloned = self.clone();
        let handle = spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(cloned.run(com_recv.clone(), evt_send.clone()))
        });
        let _ = self.thread_handle()?.insert(handle);

        Ok(())
    }

    pub fn listen(&self) -> PeaResult<NodeEventListener> {
        if let Some(events) = &self.events {
            Ok(NodeEventListener(events.clone()))
        } else {
            ClientError::InactiveError.wrap()
        }
    }
}
