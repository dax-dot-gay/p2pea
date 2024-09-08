use std::error::Error;

use async_channel::{bounded, Receiver, Sender};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

use crate::{error::PeaResult, PeaError};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CommandType {
    ListPeers
}

#[derive(Clone, Debug)]
pub struct PeaCommand {
    pub command: CommandType,
    pub channel: Sender<PeaResult<Value>>
}

impl PeaCommand {
    pub fn new(command: CommandType) -> (PeaCommand, Receiver<PeaResult<Value>>) {
        let (send, recv) = bounded::<PeaResult<Value>>(1);
        (PeaCommand {command, channel: send.clone()}, recv)
    }

    pub async fn ok<T: Serialize + DeserializeOwned>(&self, value: T) -> Result<(), Box<dyn Error>> {
        let _ = self.channel.send(PeaError::wrap(serde_json::to_value(value))).await;
        Ok(())
    }

    pub async fn err(&self, value: PeaError) -> Result<(), Box<dyn Error>> {
        let _ = self.channel.send(Err(value)).await;
        Ok(())
    }
}