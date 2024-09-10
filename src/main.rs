use std::time::Duration;

use p2pea::{BootstrapNode, Node, NodeBuilder, CommandType};
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    let mut node = NodeBuilder::<String>::default()
        .default_bootstrap()
        .build()
        .unwrap();
    node.start().expect("Failed to start");
    let listener = node.listen().unwrap();

    loop {
        while let Some(evt) = listener.try_next() {
            println!("{evt:?}");
        }

        sleep(Duration::from_secs(1)).await;
        node.invoke(CommandType::GetClosePeers).await.expect("Failed to invoke.");
    }
}
