use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{AgentStatus, GpuInfo, ProviderType};

// =============================================================================
// Agent → Hub Messages
// =============================================================================

/// Messages sent from Agent to Hub
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentMessage {
    Register(RegisterRequest),
}

// =============================================================================
// Hub → Agent Messages
// =============================================================================

/// Messages sent from Hub to Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HubMessage {
    RegisterAck(RegisterResponse),
    Error {
        message: String,
        code: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        correlation_id: Option<Uuid>,
    },
}

// =============================================================================
// Registration Message Types
// =============================================================================

/// Agent registration request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub correlation_id: Uuid,
    pub provider: ProviderType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_instance_id: Option<String>,
    pub hostname: String,
    pub gpu_info: GpuInfo,
    pub tailscale_ip: String,
    pub agent_version: String,
}

/// Hub registration acknowledgment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterResponse {
    pub correlation_id: Uuid,
    pub agent_id: Uuid,
    pub registered_at: DateTime<Utc>,
    pub hub_version: String,
}
