use std::error::Error;

use p2pea::PeerBuilder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut peer = PeerBuilder::default().protocol("test-proto".to_string()).build()?;
    peer.listen()?;
    loop {}

    Ok(())
}
