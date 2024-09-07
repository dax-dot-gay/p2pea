use std::{any::Any, error::Error, mem::discriminant};

use p2pea::{events::PeaEventType, PeaError, PeaEvent, PeerBuilder};

#[tokio::main]
async fn main() -> Result<(), PeaError> {
    let mut peer = PeerBuilder::default().protocol("test-proto".to_string()).build()?;
    peer.listen()?;
    loop {}

    Ok(())
}
