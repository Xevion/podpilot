use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use uuid::Uuid;

use crate::types::{GpuInfo, ProviderType};

/// Messages sent from Agent to Hub
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentMessage {
    Register(AgentInfo),
    HeartbeatAck(HeartbeatAckMessage),
}

/// Messages sent from Hub to Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HubMessage {
    RegisterAck(AgentRegistration),
    Heartbeat(HeartbeatMessage),
    Error {
        message: String,
        code: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        correlation_id: Option<Uuid>,
    },
}

/// Agent registration information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub correlation_id: Uuid,
    pub provider: ProviderType,
    pub provider_instance_id: String,
    pub hostname: String,
    pub gpu_info: GpuInfo,
    pub tailscale_ip: IpAddr,
    pub agent_version: String,
}

/// Agent registration response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegistration {
    pub correlation_id: Uuid,
    pub agent_id: Uuid,
    pub registered_at: DateTime<Utc>,
    pub hub_version: String,
}

/// Heartbeat ping from Hub to Agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatMessage {
    pub correlation_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub sequence: u64,
}

/// Heartbeat acknowledgment from Agent to Hub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatAckMessage {
    pub correlation_id: Uuid,
    pub timestamp: DateTime<Utc>,
}
