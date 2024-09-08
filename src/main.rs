use std::{any::Any, error::Error, mem::discriminant};

use p2pea::{events::PeaEventType, PeaError, PeaEvent, PeerBuilder};

#[tokio::main]
async fn main() -> Result<(), PeaError> {
    let peer = PeerBuilder::default().protocol("test-proto".to_string()).build()?;

    Ok(())
}
