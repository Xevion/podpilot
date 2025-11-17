use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::AgentStatus;

/// System and GPU metrics from the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metrics {
    /// GPU memory usage in bytes
    pub gpu_memory_used: u64,
    /// Total GPU memory in bytes
    pub gpu_memory_total: u64,
    /// GPU utilization percentage (0-100)
    pub gpu_utilization: u8,
    /// GPU temperature in Celsius
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_temperature: Option<u8>,
    /// Disk space used in bytes
    pub disk_used: u64,
    /// Total disk space in bytes
    pub disk_total: u64,
    /// System memory used in bytes
    pub memory_used: u64,
    /// Total system memory in bytes
    pub memory_total: u64,
    /// Timestamp when metrics were collected
    pub collected_at: DateTime<Utc>,
}

/// Metadata for a generated asset (image, video, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetMetadata {
    /// Filename of the asset
    pub filename: String,
    /// File size in bytes
    pub file_size: u64,
    /// MIME type (e.g., "image/png")
    pub content_type: String,
    /// R2 storage key
    pub r2_key: String,
    /// SHA256 hash of the file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256_hash: Option<String>,
    /// Generation prompt (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// Negative prompt (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    /// Model used for generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
    /// Generation parameters as JSON
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_params: Option<serde_json::Value>,
    /// When the asset was created
    pub created_at: DateTime<Utc>,
}

/// Structured log line from the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    /// Log level (trace, debug, info, warn, error)
    pub level: LogLevel,
    /// Log message
    pub message: String,
    /// Source of the log (component name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Additional context fields as JSON
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<serde_json::Value>,
    /// Timestamp of the log
    pub timestamp: DateTime<Utc>,
}

/// Log level enum
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Commands that the hub can send to agents
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    /// Get current agent status
    GetStatus,
    /// Get disk usage information
    GetDiskUsage,
    /// Restart the WebUI process
    RestartWebui,
    /// Terminate the agent gracefully
    Terminate,
    /// Download a specific model
    DownloadModel { model_id: Uuid, r2_key: String },
    /// Delete a model from agent storage
    DeleteModel { model_id: Uuid },
}

/// Response from command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CommandResponse {
    /// Command executed successfully
    Success {
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
    },
    /// Command failed
    Failed {
        error: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        details: Option<serde_json::Value>,
    },
}

/// Disk usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskUsage {
    /// Total disk space in bytes
    pub total: u64,
    /// Used disk space in bytes
    pub used: u64,
    /// Available disk space in bytes
    pub available: u64,
    /// Usage percentage (0-100)
    pub usage_percent: u8,
    /// Mount point or path
    pub path: String,
}

/// Status information for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatusInfo {
    /// Current agent status
    pub status: AgentStatus,
    /// Current metrics
    pub metrics: Metrics,
    /// Agent uptime in seconds
    pub uptime_seconds: u64,
    /// Whether the WebUI is running
    pub webui_running: bool,
    /// WebUI URL if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webui_url: Option<String>,
}
