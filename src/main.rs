use std::time::Duration;

use p2pea::{Node, NodeBuilder};
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    let mut node = NodeBuilder::<String>::default().bootstrap_string("/ip4/192.168.0.41/tcp/34997").unwrap().build().unwrap();
    node.start().expect("Failed to start");

    for evt in node.listen().expect("Failed to get listener") {
        println!("{evt:?}");
    }
    
}