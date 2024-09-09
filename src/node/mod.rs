use base64::{engine::general_purpose::URL_SAFE_NO_PAD as b64, Engine};
use derive_builder::Builder;
use error::ClientError;
use libp2p::{identity::Keypair, PeerId};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Debug;

pub mod error;
pub use error::{Error, PeaResult};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeInfo {
    pub key: String,
    pub group: String,
    pub port: u16,
    pub identity: Option<Value>,
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
    pub identity: Option<T>,
}

impl<T: Serialize + DeserializeOwned + Clone + Debug> Node<T> {
    pub fn info(&self) -> NodeInfo {
        NodeInfo {
            key: b64.encode(
                self.key
                    .to_protobuf_encoding()
                    .expect("Keypair could not be serialized."),
            ),
            group: self.group.clone(),
            port: self.port.clone(),
            identity: self.identity.clone().and_then(|value| {
                Some(serde_json::to_value(value).expect("Unable to serialize expected value"))
            }),
        }
    }

    pub fn from_info(info: NodeInfo) -> PeaResult<Node<T>> {
        Ok(Node::<T> {
            key: match b64.decode(info.key.clone()) {
                Ok(v) => Keypair::from_protobuf_encoding(v.as_slice()).or(Err(Error::Client(
                    ClientError::DecodingError("Keypair decoding failed".to_string()),
                ))),
                Err(_) => Err(Error::Client(ClientError::DecodingError(
                    "Base64 decoding failed".to_string(),
                ))),
            }?,
            group: info.group.clone(),
            port: info.port,
            identity: match info.identity {
                Some(identity) => {
                    Some(serde_json::from_value::<T>(identity).or(Err(Error::Client(
                        ClientError::DecodingError("Base64 decoding failed".to_string()),
                    )))?)
                }
                None => None,
            },
        })
    }

    pub fn peer_id(&self) -> PeerId {
        self.key.public().to_peer_id()
    }

    pub fn id(&self) -> String {
        self.peer_id().to_string()
    }
}
