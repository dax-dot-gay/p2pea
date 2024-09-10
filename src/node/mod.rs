use async_channel::{Receiver, Sender};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as b64, Engine};
use derive_builder::Builder;
use libp2p::{
    autonat, identity::Keypair, mdns, noise, ping, relay, rendezvous, swarm, tcp, upnp, yamux,
    PeerId, Swarm, SwarmBuilder,
};
use libp2p_stream as stream;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::{
    fmt::Debug,
    sync::{Arc, Mutex, MutexGuard},
    thread::JoinHandle,
    time::Duration,
    u64,
};

pub mod error;
pub use error::{ClientError, Error, NetworkingError, PeaResult, ProcessError};

pub mod comm;
pub use comm::{Command, CommandType, Event};

#[derive(swarm::NetworkBehaviour)]
pub struct NodeBehavior {
    rendezvous: rendezvous::client::Behaviour,
    ping: ping::Behaviour,
    upnp: upnp::tokio::Behaviour,
    mdns: mdns::tokio::Behaviour,
    autonat: autonat::Behaviour,
    stream: stream::Behaviour,
    relay: relay::client::Behaviour,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeInfo {
    pub key: String,
    pub group: String,
    pub port: u16,
    pub identity: Option<Value>,
}

#[derive(Clone, Builder)]
pub struct Node<T: Serialize + DeserializeOwned + Clone + Debug = Value> {
    #[builder(default = "Keypair::generate_ed25519()")]
    pub key: Keypair,

    #[builder(default = "\"orphans\".to_string()")]
    pub group: String,

    #[builder(default = "0")]
    pub port: u16,

    #[builder(default = "None")]
    pub identity: Option<T>,

    #[builder(setter(skip))]
    swarm: Arc<Mutex<Option<swarm::Swarm<NodeBehavior>>>>,

    #[builder(setter(skip))]
    commands: Arc<Mutex<Option<Sender<Command>>>>,

    #[builder(setter(skip))]
    events: Arc<Mutex<Option<Receiver<Event>>>>,

    #[builder(setter(skip))]
    handle: Arc<Mutex<Option<JoinHandle<PeaResult<()>>>>>,
}

impl<T: Serialize + DeserializeOwned + Clone + Debug> Node<T> {
    pub fn info(&self) -> NodeInfo {
        NodeInfo {
            key: b64.encode(
                self.key
                    .to_protobuf_encoding()
                    .expect("Keypair could not be serialized."),
            ),
            group: self.group.clone(),
            port: self.port.clone(),
            identity: self.identity.clone().and_then(|value| {
                Some(serde_json::to_value(value).expect("Unable to serialize expected value"))
            }),
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
            group: info.group.clone(),
            port: info.port,
            identity: match info.identity {
                Some(identity) => {
                    Some(serde_json::from_value::<T>(identity).or(Err(Error::Client(
                        ClientError::DecodingError("Base64 decoding failed".to_string()),
                    )))?)
                }
                None => None,
            },
            swarm: Arc::new(Mutex::new(None)),
            commands: Arc::new(Mutex::new(None)),
            events: Arc::new(Mutex::new(None)),
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

    fn commands(&self) -> PeaResult<MutexGuard<Option<Sender<Command>>>> {
        self.commands.lock().or(ProcessError::SyncError.wrap())
    }

    fn events(&self) -> PeaResult<MutexGuard<Option<Receiver<Event>>>> {
        self.events.lock().or(ProcessError::SyncError.wrap())
    }

    fn thread_handle(&self) -> PeaResult<MutexGuard<Option<JoinHandle<PeaResult<()>>>>> {
        self.handle.lock().or(ProcessError::SyncError.wrap())
    }

    pub fn start(&self) -> PeaResult<()> {
        let mut swarm = SwarmBuilder::with_existing_identity(self.key.clone())
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
                Ok(NodeBehavior {
                    rendezvous: rendezvous::client::Behaviour::new(key.clone()),
                    ping: ping::Behaviour::default(),
                    upnp: upnp::tokio::Behaviour::default(),
                    mdns: mdns::tokio::Behaviour::new(mdns::Config::default(), self.peer_id())
                        .unwrap(),
                    autonat: autonat::Behaviour::new(self.peer_id(), autonat::Config::default()),
                    stream: stream::Behaviour::default(),
                    relay,
                })
            })
            .or(NetworkingError::SwarmCreation("Behaviour definition failure".to_string()).wrap())?
            .with_swarm_config(|cfg| {
                cfg.with_idle_connection_timeout(Duration::from_secs(u64::MAX))
            })
            .build();

        Ok(())
    }
}
