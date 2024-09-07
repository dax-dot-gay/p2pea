use std::fmt::{Display, Debug};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PeaError {
    Generic(String),
    Listening,
    NotListening
}

impl PeaError {
    pub fn wrap<V, E: Debug>(r: Result<V, E>) -> Result<V, PeaError> {
        r.or_else(|e| Err(PeaError::Generic(format!("{e:?}"))))
    }
}

impl Display for PeaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type PeaResult<T> = Result<T, PeaError>;