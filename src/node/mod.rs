use derive_builder::Builder;
use libp2p::identity::Keypair;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Debug;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as b64, Engine};

pub mod error;
pub use error::{PeaResult, Error};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeInfo {
    pub key: String,
    pub group: String,
    pub port: u16,
    pub identity: Option<Value>
}

#[derive(Clone, Debug, Builder)]
pub struct Node<T: Serialize + DeserializeOwned + Clone + Debug = Value> {
    #[builder(default = "Keypair::generate_ed25519()")]
    pub key: Keypair,

    #[builder(default = "\"orphans\".to_string()")]
    pub group: String,

    #[builder(default = "0")]
    pub port: u16,

    #[builder(default = "None")]
    pub identity: Option<T>
}

impl<T: Serialize + DeserializeOwned + Clone + Debug> Node<T> {
    pub fn info(&self) -> NodeInfo {
        NodeInfo {
            key: b64.encode(self.key.to_protobuf_encoding().expect("Keypair could not be serialized.")),
            group: self.group.clone(),
            port: self.port.clone(),
            identity: self.identity.clone().and_then(|value| Some(serde_json::to_value(value).expect("Unable to serialize expected value")))
        }
    }

    pub fn from_info(info: NodeInfo) -> PeaResult<NodeInfo> {
        
    }
}