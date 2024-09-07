use std::error::Error;

use p2pea::{PeaError, PeerBuilder};

#[tokio::main]
async fn main() -> Result<(), PeaError> {
    let mut peer = PeerBuilder::default().protocol("test-proto".to_string()).build()?;
    peer.listen()?;
    loop {}

    Ok(())
}
