use std::{any::Any, error::Error, mem::discriminant};

use libp2p::PeerId;
use p2pea::{events::PeaEventType, PeaError, PeaEvent, PeerBuilder, CommandType};

#[tokio::main]
async fn main() -> Result<(), PeaError> {
    let peer = PeerBuilder::default().protocol("test-proto".to_string()).build()?;
    let server = peer.connect();

    if let Some(addr) = std::env::args().nth(1) {
        let result = server.call::<Vec<String>>(CommandType::DirectConnect(addr)).await;
        println!("Peers: {result:?}");
    }

    for event in server.events() {
        println!("EVENT: {event:?}");
        for id in server.call::<Vec<String>>(CommandType::ListPeers).await.unwrap() {
            let _ = server.call::<String>(CommandType::SendData { peer: id, data: serde_json::to_value("ALL HAIL DARK LORD POTATOMAN").unwrap() }).await;
        }
    }

    Ok(())
}
