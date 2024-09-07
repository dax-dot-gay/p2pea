use std::{
    error::Error,
    sync::{Arc, Mutex},
};

use async_channel::{unbounded, Receiver, Sender};
use base64::{engine, Engine};
use derive_builder::Builder;
use futures::StreamExt;
use libp2p::{
    autonat, identify,
    identity::Keypair,
    mdns, noise, ping,
    request_response::{self, ProtocolSupport},
    swarm::{self, SwarmEvent},
    tcp, upnp, yamux, PeerId, StreamProtocol, Swarm, SwarmBuilder,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{spawn, task::JoinHandle};

use crate::{commands::PeaCommand, events::PeaEvent};

#[derive(swarm::NetworkBehaviour)]
pub struct PeaBehavior {
    upnp: upnp::tokio::Behaviour,
    mdns: mdns::tokio::Behaviour,
    identify: identify::Behaviour,
    autonat: autonat::Behaviour,
    ping: ping::Behaviour,
    request_response: request_response::json::Behaviour<Value, Value>,
}

pub struct ActivePeer {
    pub protocol: String,
    pub version: String,
    pub service_port: u16,
    pub identity: Keypair,
    pub swarm: Swarm<PeaBehavior>,
    pub events: Sender<PeaEvent>,
    pub commands: Receiver<PeaCommand>,
}

impl ActivePeer {
    pub fn new(
        protocol: String,
        version: String,
        key: Keypair,
        port: u16,
        events: Sender<PeaEvent>,
        commands: Receiver<PeaCommand>,
    ) -> Result<Self, Box<dyn Error>> {
        let swarm = SwarmBuilder::with_existing_identity(key.clone())
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|key| {
                Ok(PeaBehavior {
                    upnp: upnp::tokio::Behaviour::default(),
                    mdns: mdns::tokio::Behaviour::new(
                        mdns::Config::default(),
                        key.public().to_peer_id(),
                    )?,
                    identify: identify::Behaviour::new(identify::Config::new(
                        format!("{protocol}/{version}"),
                        key.public(),
                    )),
                    autonat: autonat::Behaviour::new(
                        key.public().to_peer_id(),
                        autonat::Config::default(),
                    ),
                    ping: ping::Behaviour::default(),
                    request_response: request_response::json::Behaviour::new(
                        [(StreamProtocol::new("/pea-json"), ProtocolSupport::Full)],
                        request_response::Config::default(),
                    ),
                })
            })?
            .build();

        Ok(ActivePeer {
            protocol,
            version,
            identity: key,
            swarm,
            service_port: port,
            events,
            commands,
        })
    }

    async fn handle_event(
        &mut self,
        event: SwarmEvent<PeaBehaviorEvent>,
    ) -> Result<(), Box<dyn Error>> {
        println!("EVENT: {event:?}");
        Ok(())
    }

    async fn handle_command(&mut self, command: PeaCommand) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    pub async fn serve(&mut self) -> Result<(), Box<dyn Error>> {
        let listener = self
            .swarm
            .listen_on(format!("/ip4/0.0.0.0/tcp/{}", self.service_port).parse()?)?;
        loop {
            let mut commands = Box::pin(self.commands.clone());
            tokio::select! {
                event = self.swarm.select_next_some() => self.handle_event(event).await?,
                command = commands.next() => match command {
                    Some(c) => self.handle_command(c).await?,
                    None => break
                }
            }
        }

        self.swarm.remove_listener(listener);
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Builder)]
pub struct Peer {
    #[builder(default = "\"p2pea.generic\".to_string()")]
    pub protocol: String,
    #[builder(default = "\"1.0.0\".to_string()")]
    pub version: String,
    #[builder(default = "0")]
    pub service_port: u16,
    #[builder(
        default = "engine::general_purpose::URL_SAFE_NO_PAD.encode(Keypair::generate_ed25519().to_protobuf_encoding().unwrap())"
    )]
    pub key: String,

    #[serde(skip)]
    #[builder(setter(skip))]
    pub commands: Option<Sender<PeaCommand>>,

    #[serde(skip)]
    #[builder(setter(skip))]
    pub events: Option<Receiver<PeaEvent>>,

    #[serde(skip)]
    #[builder(setter(skip))]
    pub handle: Option<Arc<Mutex<JoinHandle<Result<(), String>>>>>,
}

impl PeerBuilder {
    pub fn with_keypair(&mut self, key: Keypair) -> Result<Self, Box<dyn Error>> {
        self.key =
            Some(engine::general_purpose::URL_SAFE_NO_PAD.encode(key.to_protobuf_encoding()?));
        Ok(self.clone())
    }

    pub fn with_b64_key(&mut self, key: String) -> Self {
        self.key = Some(key);
        self.clone()
    }
}

impl Peer {
    pub fn keypair(&self) -> Result<Keypair, Box<dyn Error>> {
        Ok(Keypair::from_protobuf_encoding(
            engine::general_purpose::URL_SAFE_NO_PAD
                .decode(self.key.clone())?
                .as_slice(),
        )?)
    }

    pub fn id(&self) -> Result<PeerId, Box<dyn Error>> {
        Ok(self.keypair()?.public().to_peer_id())
    }

    pub fn listen(&mut self) -> Result<(), Box<dyn Error>> {
        let (com_send, com_recv) = unbounded::<PeaCommand>();
        let (evt_send, evt_recv) = unbounded::<PeaEvent>();
        self.commands = Some(com_send);
        self.events = Some(evt_recv);
        let mut peer = ActivePeer::new(
            self.protocol.clone(),
            self.version.clone(),
            self.keypair()?,
            self.service_port.clone(),
            evt_send,
            com_recv,
        )?;

        let handle = spawn(async move { peer.serve().await.or_else(|e| Err(format!("{e:?}"))) });
        self.handle = Some(Arc::new(Mutex::new(handle)));
        Ok(())
    }

    pub fn unlisten(&mut self) -> Result<(), String> {
        if self.handle.is_some() {
            self.commands.clone().unwrap().close();
            self.events.clone().unwrap().close();
            self.commands = None;
            self.events = None;

            self.handle = None;

            Ok(())
        } else {
            Err("Not currently listening".to_string())
        }
    }
}
