use libp2p::{swarm::SwarmEvent, PeerId};

use crate::swarm::PeaBehaviorEvent;

pub enum PeaEvent {
    Swarm{protocol: String, port: String, id: PeerId, event: SwarmEvent<PeaBehaviorEvent>}
}