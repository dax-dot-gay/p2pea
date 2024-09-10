use std::time::Duration;

use p2pea::{BootstrapNode, Node, NodeBuilder};
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    let mut node = NodeBuilder::<String>::default()
        .default_bootstrap()
        .build()
        .unwrap();
    node.start().expect("Failed to start");

    for evt in node.listen().expect("Failed to get listener") {
        println!("{evt:?}");
    }
}
