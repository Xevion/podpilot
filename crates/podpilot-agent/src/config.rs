use figment::{Figment, providers::Env};
use podpilot_common::types::ProviderType;
use serde::{Deserialize, Serialize};

/// Agent configuration loaded from environment variables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// WebSocket URL for Hub connection
    /// Default: ws://localhost:8080/ws/agent
    #[serde(default = "default_hub_url")]
    pub hub_url: String,

    /// Port for agent status API
    /// Default: 8081
    #[serde(default = "default_status_port")]
    pub status_port: u16,

    /// Provider type (local, vastai, runpod)
    /// Default: local
    #[serde(default = "default_provider")]
    pub provider: ProviderType,

    /// Optional provider instance ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_instance_id: Option<String>,

    /// Hostname override (auto-detected if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,

    /// Tailscale IP address
    /// Default: 0.0.0.0 (should be set in production)
    #[serde(default = "default_tailscale_ip")]
    pub tailscale_ip: String,

    /// Log level
    /// Default: info
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_hub_url() -> String {
    "ws://localhost:8080/ws/agent".to_string()
}

fn default_status_port() -> u16 {
    8081
}

fn default_provider() -> ProviderType {
    ProviderType::Local
}

fn default_tailscale_ip() -> String {
    "0.0.0.0".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Config {
    /// Load configuration from environment variables
    pub fn load() -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Env::raw().map(|k| {
                // Map environment variable names to struct field names
                match k.as_str() {
                    "HUB_WEBSOCKET_URL" => "hub_url".into(),
                    "STATUS_PORT" => "status_port".into(),
                    "PROVIDER_TYPE" => "provider".into(),
                    "PROVIDER_INSTANCE_ID" => "provider_instance_id".into(),
                    "HOSTNAME" => "hostname".into(),
                    "TAILSCALE_IP" => "tailscale_ip".into(),
                    "LOG_LEVEL" => "log_level".into(),
                    _ => k.into(),
                }
            }))
            .extract()
    }

    /// Get the hostname, using configured value or auto-detecting
    pub fn get_hostname(&self) -> String {
        self.hostname.clone().unwrap_or_else(|| {
            hostname::get()
                .unwrap_or_else(|_| std::ffi::OsString::from("unknown"))
                .to_string_lossy()
                .to_string()
        })
    }
}
