use std::str::FromStr;

use libp2p::PeerId;
use p2pea::{CommandType, PeaError, PeaEventType, Peer, PeerBuilder};

#[tokio::main]
async fn main() -> Result<(), PeaError> {
    let peer = PeerBuilder::default().protocol("test-proto".to_string()).build()?;
    let server = peer.connect();

    if let Some(addr) = std::env::args().nth(1) {
        let result = server.call::<Vec<String>>(CommandType::DirectConnect(addr)).await;
        println!("Peers: {result:?}");
    }

    for event in server.events() {
        match event.event {
            PeaEventType::ExternalAddress(addr) => println!("External: {addr}"),
            PeaEventType::DataRecv(data) => println!("Recieved: {:?}", data.as_object::<Peer>()),
            _ => ()
        }
        for p in server.call::<Vec<String>>(CommandType::ListPeers).await.unwrap() {
            let _ = server.send_data(PeerId::from_str(p.as_str()).unwrap(), peer.clone()).await;
        }
    }

    Ok(())
}
