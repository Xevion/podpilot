use crate::rpc::{AgentStatusInfo, Command, CommandResponse, DiskUsage, RpcError};

/// Service trait for Agents - exposes methods that the hub can call
#[tarpc::service]
pub trait AgentService {
    /// Send a command to the agent
    ///
    /// The hub can send various commands like RestartWebui, Terminate, etc.
    /// Returns a response indicating success or failure.
    async fn send_command(cmd: Command) -> Result<CommandResponse, RpcError>;

    /// Query the current agent status
    ///
    /// Returns comprehensive status information including metrics,
    /// uptime, and WebUI status.
    async fn get_status() -> Result<AgentStatusInfo, RpcError>;

    /// Get disk usage information
    ///
    /// Returns detailed disk space statistics for the agent's storage.
    async fn get_disk_usage() -> Result<DiskUsage, RpcError>;

    /// Restart the WebUI process
    ///
    /// Gracefully restarts the A1111/ComfyUI/etc process.
    /// Returns success or error.
    async fn restart_webui() -> Result<(), RpcError>;

    /// Terminate the agent gracefully
    ///
    /// Shuts down all processes and prepares for instance termination.
    /// Returns success or error.
    async fn terminate() -> Result<(), RpcError>;
}
