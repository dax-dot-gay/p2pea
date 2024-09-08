use std::str::FromStr;

use base64::Engine;
use libp2p::PeerId;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use strum::AsRefStr;

use crate::{error::PeaResult, PeaError};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StreamData {
    peer: String,
    data: String,
    size: usize
}

impl StreamData {
    pub fn id_str(&self) -> String {
        self.peer.clone()
    }

    pub fn id(&self) -> PeaResult<PeerId> {
        PeerId::from_str(self.peer.as_str()).or_else(|_| Err(PeaError::InvalidId(self.peer.clone())))
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(self.data.clone()).expect("Critical error, invalid base64 encoding")
    }

    pub fn as_string(&self) -> PeaResult<String> {
        PeaError::wrap(String::from_utf8(self.as_bytes()))
    }

    pub fn as_value(&self) -> PeaResult<Value> {
        PeaError::wrap(self.as_string().and_then(|s| PeaError::wrap(serde_json::from_str::<Value>(s.as_str()))))
    }

    pub fn as_object<T: Serialize + DeserializeOwned>(&self) -> PeaResult<T> {
        PeaError::wrap(self.as_value().and_then(|v| PeaError::wrap(serde_json::from_value::<T>(v))))
    }

    pub fn as_base64(&self) -> String {
        self.data.clone()
    }

    pub fn new(id: PeerId, data: Vec<u8>, size: usize) -> Self {
        StreamData {
            peer: id.to_string(),
            data: base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data),
            size
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, AsRefStr)]
#[strum(serialize_all = "kebab-case")]
pub enum PeaEventType {
    NewListeningAddress(String),
    ExternalAddress(String),
    ConnectionOpen{peer: String, id: String, address: String},
    ConnectionClosed{peer: String, id: String, address: String, reason: Option<String>},
    DataRecv(StreamData),
    DataRecvFailure{peer: String, reason: String}
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeaEvent {
    pub protocol: String,
    pub version: String,
    pub peer: String,
    pub event: PeaEventType
}

impl PeaEvent {
    pub fn matches<T: AsRef<str>>(&self, evt: T) -> bool {
        self.event.as_ref().to_string() == evt.as_ref().to_string()
    }
}