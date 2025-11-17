use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use std::net::IpAddr;
use uuid::Uuid;

/// Cloud provider or platform type for agent instances
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "provider_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    VastAI,
    Runpod,
    Local,
}

/// Agent status representing current operational state
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "agent_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Registering,
    Ready,
    Running,
    Idle,
    Error,
    Terminated,
}

/// Type of model file (checkpoint, LoRA, embedding, VAE)
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "model_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ModelType {
    Checkpoint,
    Lora,
    Embedding,
    Vae,
}

/// Remote GPU agent instance
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct Agent {
    pub id: Uuid,
    pub provider: ProviderType,
    pub provider_instance_id: Option<String>,
    pub hostname: String,
    pub status: AgentStatus,
    pub tailscale_ip: Option<IpAddr>,
    pub gpu_info: Option<Json<serde_json::Value>>,
    pub registered_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub terminated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Generated asset (image, video, etc.) stored in R2
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct Asset {
    pub id: Uuid,
    pub agent_id: Option<Uuid>,
    pub r2_key: String,
    pub filename: String,
    pub file_size: i64,
    pub content_type: String,
    pub metadata: Option<Json<serde_json::Value>>,
    pub created_at: DateTime<Utc>,
    pub synced_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Model file stored in R2 (checkpoint, LoRA, embedding, VAE)
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct Model {
    pub id: Uuid,
    pub name: String,
    #[sqlx(rename = "type")]
    pub model_type: ModelType,
    pub r2_key: String,
    pub file_size: i64,
    pub hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Many-to-many relationship tracking which models each agent has downloaded
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct AgentModel {
    pub agent_id: Uuid,
    pub model_id: Uuid,
    pub downloaded_at: DateTime<Utc>,
}
