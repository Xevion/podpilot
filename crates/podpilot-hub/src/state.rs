use dashmap::DashMap;
use podpilot_common::protocol::HubMessage;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub connections: Arc<DashMap<Uuid, mpsc::Sender<HubMessage>>>,
}

impl AppState {
    pub fn new(db: PgPool) -> Self {
        Self {
            db,
            connections: Arc::new(DashMap::new()),
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
}
