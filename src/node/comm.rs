use std::num::NonZero;

use async_channel::{unbounded, Receiver, Sender};
use libp2p::{
    core::{transport::ListenerId, ConnectedPoint},
    swarm::ConnectionId,
    Multiaddr, PeerId,
};
use serde_json::Value;

use super::{Error, PeaResult, ProcessError};

#[derive(Clone, Debug)]
pub enum CommandType {}

#[derive(Clone, Debug)]
pub struct Command {
    pub kind: CommandType,
    pub channel: Sender<PeaResult<Value>>,
}

impl Command {
    pub fn new(command: CommandType) -> (Command, Receiver<PeaResult<Value>>) {
        let (tx, rx) = unbounded();
        (
            Command {
                kind: command,
                channel: tx,
            },
            rx,
        )
    }
}

#[derive(Clone, Debug)]
pub enum ListenerEvent {
    NewAddress {
        id: ListenerId,
        address: Multiaddr,
    },
    ExpiredAddress {
        id: ListenerId,
        address: Multiaddr,
    },
    Closed {
        id: ListenerId,
        addresses: Vec<Multiaddr>,
        reason: Option<String>,
    },
    Error {
        id: ListenerId,
        reason: String,
    },
}

#[derive(Clone, Debug)]
pub enum AddressEvent {
    Confirmed(Multiaddr),
    Expired(Multiaddr),
}

#[derive(Clone, Debug)]
pub enum NetworkEvent {
    Dialing {
        peer_id: Option<PeerId>,
        connection_id: ConnectionId
    },
    PeerAddress {
        peer_id: PeerId,
        address: Multiaddr,
    },
    ConnectionOpened {
        peer_id: PeerId,
        connection_id: ConnectionId,
        endpoint: ConnectedPoint,
        count: NonZero<u32>,
    },
    ConnectionClosed {
        peer_id: PeerId,
        connection_id: ConnectionId,
        endpoint: ConnectedPoint,
        count: u32,
        reason: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub enum Event {
    Listener(ListenerEvent),
    Address(AddressEvent),
    Network(NetworkEvent),
}

impl Event {
    pub async fn send(&self, sender: Sender<Event>) -> Result<(), Error> {
        sender
            .send(self.clone())
            .await
            .or(ProcessError::IPCError.wrap())
    }
}
