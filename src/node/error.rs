use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ClientError {
    // Failed to decode user input data
    DecodingError(String),

    // Attempted to operate on non-running node
    InactiveError
}

impl Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl ClientError {
    pub fn wrap<T>(&self) -> PeaResult<T> {
        Err(Error::Client(self.clone()))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ProcessError {
    // Failed to handle thread-synchronized data
    SyncError,

    // Requested resource is inactive
    ResourceError,

    // Unexpected internal value
    LogicError(String),

    // Closed connection
    Closed,

    // Error in communication between main thread & server
    IPCError
}

impl Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl ProcessError {
    pub fn wrap<T>(&self) -> PeaResult<T> {
        Err(Error::Process(self.clone()))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum NetworkingError {
    // Failure during swarm creation
    SwarmCreation(String),

    // Failed to initialize network process
    InitializationError(String),

    // Failed to connect to bootstrap node
    BootstrapError
}

impl Display for NetworkingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl NetworkingError {
    pub fn wrap<T>(&self) -> PeaResult<T> {
        Err(Error::Network(self.clone()))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Error {
    Client(ClientError),
    Process(ProcessError),
    Network(NetworkingError)
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub type PeaResult<T> = Result<T, Error>;