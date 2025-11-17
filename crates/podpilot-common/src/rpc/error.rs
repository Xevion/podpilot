use serde::{Deserialize, Serialize};

/// Error type for RPC operations
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub enum RpcError {
    /// Agent is not registered with the hub
    #[error("Agent is not registered")]
    NotRegistered,

    /// Agent with this identity already exists and is connected
    #[error("Agent identity conflict: {0}")]
    IdentityConflict(String),

    /// Database error occurred
    #[error("Database error: {0}")]
    Database(String),

    /// Invalid request parameters
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Agent not found
    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    /// Command execution failed
    #[error("Command failed: {0}")]
    CommandFailed(String),

    /// Internal server error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Connection error
    #[error("Connection error: {0}")]
    Connection(String),

    /// Timeout error
    #[error("Operation timed out")]
    Timeout,
}

impl From<anyhow::Error> for RpcError {
    fn from(err: anyhow::Error) -> Self {
        RpcError::Internal(err.to_string())
    }
}
