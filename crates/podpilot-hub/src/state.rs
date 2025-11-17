use dashmap::DashMap;
use podpilot_common::protocol::HubMessage;
use sqlx::PgPool;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub connections: Arc<DashMap<Uuid, mpsc::Sender<HubMessage>>>,
    pub tailscale_ip: Arc<RwLock<Option<IpAddr>>>,
}

impl AppState {
    pub fn new(db: PgPool) -> Self {
        Self {
            db,
            connections: Arc::new(DashMap::new()),
            tailscale_ip: Arc::new(RwLock::new(None)),
        }
    }

    /// Register a new agent connection
    pub fn register_connection(&self, agent_id: Uuid, sender: mpsc::Sender<HubMessage>) {
        self.connections.insert(agent_id, sender);
    }

    /// Remove an agent connection
    pub fn remove_connection(&self, agent_id: &Uuid) {
        self.connections.remove(agent_id);
    }

    /// Send a message to a specific agent
    pub async fn send_to_agent(&self, agent_id: &Uuid, message: HubMessage) -> anyhow::Result<()> {
        if let Some(sender) = self.connections.get(agent_id) {
            sender
                .send(message)
                .await
                .map_err(|_| anyhow::anyhow!("Failed to send message to agent {}", agent_id))?;
            Ok(())
        } else {
            anyhow::bail!("Agent {} not connected", agent_id)
        }
    }

    /// Get all connected agent IDs
    pub fn connected_agents(&self) -> Vec<Uuid> {
        self.connections.iter().map(|entry| *entry.key()).collect()
    }

    /// Get the number of connected agents
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Get the current Tailscale IP address
    pub async fn tailscale_ip(&self) -> Option<IpAddr> {
        *self.tailscale_ip.read().await
    }
}
