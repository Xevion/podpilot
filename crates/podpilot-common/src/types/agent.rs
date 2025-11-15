use serde::{Deserialize, Serialize};

/// Cloud provider or platform type for agent instances
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    #[serde(rename = "vastai")]
    VastAI,
    Runpod,
    Local,
}

/// Agent status representing current operational state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Registering,
    Ready,
    Running,
    Idle,
    Error,
    Terminated,
}
