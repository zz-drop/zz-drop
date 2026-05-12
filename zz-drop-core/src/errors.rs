use thiserror::Error;

use crate::agent_proto::AgentError;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid profile: {0}")]
    InvalidProfile(String),

    #[error("serde error: {0}")]
    Serde(String),

    #[error("agent: {0}")]
    Agent(#[from] AgentError),
}

impl From<serde_json::Error> for CoreError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serde(value.to_string())
    }
}
