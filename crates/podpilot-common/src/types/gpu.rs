use serde::{Deserialize, Serialize};

/// GPU information reported by agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub memory_gb: f32,
    pub cuda_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_capability: Option<String>,
}
