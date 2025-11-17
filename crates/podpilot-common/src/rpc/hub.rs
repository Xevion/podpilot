use uuid::Uuid;

use crate::protocol::AgentInfo;
use crate::protocol::AgentRegistration;
use crate::rpc::{AssetMetadata, LogLine, Metrics, RpcError};
use crate::types::AgentStatus;

/// Service trait for the Hub - exposes methods that agents can call
#[tarpc::service]
pub trait HubService {
    /// Register a new agent or reconnect an existing agent
    ///
    /// If an agent with the same (tailscale_ip, provider_instance_id) exists,
    /// it will reuse the existing database record. Otherwise, creates a new one.
    async fn register_agent(info: AgentInfo) -> Result<AgentRegistration, RpcError>;

    /// Send a periodic heartbeat with status and metrics
    ///
    /// Updates the agent's last_seen_at timestamp and current status.
    /// Metrics are stored for monitoring purposes.
    async fn heartbeat(
        agent_id: Uuid,
        status: AgentStatus,
        metrics: Metrics,
    ) -> Result<(), RpcError>;

    /// Register a newly uploaded asset
    ///
    /// Called after the agent successfully uploads a file to R2.
    /// Creates a database record linking the asset to the agent.
    /// Returns the UUID of the created asset record.
    async fn register_asset(agent_id: Uuid, asset: AssetMetadata) -> Result<Uuid, RpcError>;

    /// Stream log lines to the hub
    ///
    /// Agents can batch multiple log lines and send them periodically.
    /// The hub stores or forwards these logs for monitoring.
    async fn send_logs(agent_id: Uuid, logs: Vec<LogLine>) -> Result<(), RpcError>;
}
