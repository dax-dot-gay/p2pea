use async_channel::{unbounded, Receiver, Sender};
use serde_json::Value;

use super::PeaResult;

#[derive(Clone, Debug)]
pub enum CommandType {}

#[derive(Clone, Debug)]
pub struct Command {
    pub kind: CommandType,
    pub channel: Sender<PeaResult<Value>>
}

impl Command {
    pub fn new(command: CommandType) -> (Command, Receiver<PeaResult<Value>>) {
        let (tx, rx) = unbounded();
        (Command {kind: command, channel: tx}, rx)
    }
}

#[derive(Clone, Debug)]
pub enum Event {}