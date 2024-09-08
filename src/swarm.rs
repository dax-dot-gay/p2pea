use std::{
    error::Error, str::FromStr, thread::{spawn, JoinHandle}, time::Duration
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
    swarm::{self, DialError, SwarmEvent},
    tcp, upnp, yamux, Multiaddr, PeerId, StreamProtocol, Swarm, SwarmBuilder,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

use crate::{
    commands::{CommandType, PeaCommand},
    error::PeaResult,
    events::{PeaEvent, PeaEventType},
    PeaError,
};

#[derive(swarm::NetworkBehaviour)]
pub struct PeaBehavior {
    upnp: upnp::tokio::Behaviour,
    mdns: mdns::tokio::Behaviour,
    identify: identify::Behaviour,
    autonat: autonat::Behaviour,
    ping: ping::Behaviour,
    request_response: request_response::json::Behaviour<Value, Value>,
}

struct Server {
    pub protocol: String,
    pub version: String,
    pub service_port: u16,
    pub identity: Keypair,
    pub swarm: Swarm<PeaBehavior>,
    pub events: Sender<PeaEvent>,
    pub commands: Receiver<PeaCommand>,
}

impl Server {
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
            .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(u64::MAX)))
            .build();

        Ok(Server {
            protocol,
            version,
            identity: key,
            swarm,
            service_port: port,
            events,
            commands,
        })
    }

    fn emit(&self, event: PeaEventType) -> () {
        let data = PeaEvent {
            protocol: self.protocol.clone(),
            version: self.version.clone(),
            peer: self.identity.public().to_peer_id().to_string(),
            event,
        };
        let _ = self.events.send_blocking(data);
    }

    async fn handle_event(&mut self, event: SwarmEvent<PeaBehaviorEvent>) -> () {
        match event {
            SwarmEvent::NewListenAddr {
                listener_id: _,
                address,
            } => self.emit(PeaEventType::NewListeningAddress(address.to_string())),
            SwarmEvent::ExternalAddrConfirmed { address } => {
                self.emit(PeaEventType::ExternalAddress(address.to_string()))
            }
            SwarmEvent::ConnectionEstablished {
                peer_id,
                connection_id,
                endpoint,
                num_established: _,
                concurrent_dial_errors: _,
                established_in: _,
            } => self.emit(PeaEventType::ConnectionOpen {
                peer: peer_id.to_string(),
                id: connection_id.to_string(),
                address: endpoint.get_remote_address().to_string(),
            }),
            SwarmEvent::ConnectionClosed {
                peer_id,
                connection_id,
                endpoint,
                num_established: _,
                cause
            } => self.emit(PeaEventType::ConnectionClosed {
                peer: peer_id.to_string(),
                id: connection_id.to_string(),
                address: endpoint.get_remote_address().to_string(),
                reason: cause.and_then(|e| Some(format!("{e:?}")))
            }),
            evt => {
                println!("{:?}", evt);
                ()
            }
        }
    }

    async fn handle_command(&mut self, command: PeaCommand) -> () {
        match command.clone().command {
            CommandType::ListPeers => {
                command
                    .ok(self
                        .swarm
                        .connected_peers()
                        .map(|v| v.to_string())
                        .collect::<Vec<String>>())
                    .await
            }
            CommandType::DirectConnect(addr) => {
                if let Ok(adr) = addr.parse::<Multiaddr>() {
                    match self.swarm.dial(adr) {
                        Ok(_) => {
                            command
                                .ok(self
                                    .swarm
                                    .connected_peers()
                                    .map(|v| v.to_string())
                                    .collect::<Vec<String>>())
                                .await;
                        }
                        Err(e) => {
                            command
                                .err(PeaError::wrap::<(), DialError>(Err(e)).unwrap_err())
                                .await;
                        }
                    }
                    ()
                } else {
                    ()
                }
            }
            CommandType::SendData { peer, data } => {
                match PeerId::from_str(peer.as_str()) {
                    Ok(id) => command.ok(self.swarm.behaviour_mut().request_response.send_request(&id, data).to_string()).await,
                    Err(e) => command.err(PeaError::InvalidId(format!("{e:?}"))).await
                }
            }
        }
    }

    pub async fn serve(&mut self) -> Result<(), Box<dyn Error>> {
        let listener = self
            .swarm
            .listen_on(format!("/ip4/0.0.0.0/tcp/{}", self.service_port).parse()?)?;
        loop {
            let mut commands = Box::pin(self.commands.clone());
            tokio::select! {
                event = self.swarm.select_next_some() => self.handle_event(event).await,
                command = commands.next() => match command {
                    Some(c) => self.handle_command(c).await,
                    None => break
                }
            }
        }

        self.swarm.remove_listener(listener);
        Ok(())
    }
}
#[derive(Serialize, Deserialize, Clone, Builder, Debug)]
#[builder(build_fn(error = "crate::PeaError"))]
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
    pub fn keypair(&self) -> PeaResult<Keypair> {
        Ok(PeaError::wrap(Keypair::from_protobuf_encoding(
            PeaError::wrap(engine::general_purpose::URL_SAFE_NO_PAD.decode(self.key.clone()))?
                .as_slice(),
        ))?)
    }

    pub fn id(&self) -> PeaResult<PeerId> {
        Ok(PeaError::wrap(self.keypair())?.public().to_peer_id())
    }

    pub fn connect(&self) -> ActivePeer {
        ActivePeer::new(self.clone())
    }
}

pub struct ActivePeer {
    pub peer: Peer,
    pub events: Receiver<PeaEvent>,
    pub commands: Sender<PeaCommand>,
    pub server: JoinHandle<Result<(), PeaError>>,
}

impl ActivePeer {
    pub fn new(peer: Peer) -> Self {
        let (evt_send, evt_recv) = unbounded::<PeaEvent>();
        let (com_send, com_recv) = unbounded::<PeaCommand>();

        let s_peer = peer.clone();
        let tx = evt_send.clone();
        let rx = com_recv.clone();

        let handle = spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let mut server = PeaError::wrap(Server::new(
                        s_peer.protocol.clone(),
                        s_peer.version.clone(),
                        s_peer.keypair()?,
                        s_peer.service_port,
                        tx,
                        rx,
                    ))?;
                    PeaError::wrap(server.serve().await)
                })
        });

        ActivePeer {
            peer: peer.clone(),
            events: evt_recv.clone(),
            commands: com_send.clone(),
            server: handle,
        }
    }

    pub fn events(&self) -> PeerEventLoop {
        PeerEventLoop {
            peer: self.peer.clone(),
            events: self.events.clone(),
        }
    }

    pub async fn call<T: Serialize + DeserializeOwned>(
        &self,
        command: CommandType,
    ) -> PeaResult<T> {
        let (cmd, recv) = PeaCommand::new(command);
        PeaError::wrap(self.commands.send(cmd).await)?;

        match PeaError::wrap(recv.recv().await)? {
            Ok(v) => PeaError::wrap(serde_json::from_value::<T>(v)),
            Err(e) => Err(e),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PeerEventLoop {
    pub peer: Peer,
    pub events: Receiver<PeaEvent>,
}

impl Iterator for PeerEventLoop {
    type Item = PeaEvent;
    fn next(&mut self) -> Option<Self::Item> {
        match self.events.recv_blocking() {
            Ok(event) => Some(event),
            Err(_) => None,
        }
    }
}

impl PeerEventLoop {
    pub fn try_next(&mut self) -> PeaResult<PeaEvent> {
        self.events.try_recv().or(Err(PeaError::NoEvents))
    }
}
