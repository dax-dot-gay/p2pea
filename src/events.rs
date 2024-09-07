use serde::{Deserialize, Serialize};
use strum::AsRefStr;

#[derive(Serialize, Deserialize, Debug, Clone, AsRefStr)]
#[strum(serialize_all = "kebab-case")]
pub enum PeaEventType {
    NewListeningAddress(String),
    ExternalAddress(String)
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