//! Configuration module for the podpilot-hub application.
//!
//! This module handles loading and parsing configuration from environment variables
//! using the figment crate. It supports flexible duration parsing that accepts both
//! numeric values (interpreted as seconds) and duration strings with units.

use fundu::{DurationParser, TimeUnit};
use secrecy::SecretString;
use serde::{Deserialize, Deserializer};
use std::time::Duration;

/// Tailscale OAuth configuration for Hub authentication
///
/// Contains optional OAuth credentials. Both fields must be provided together or both omitted.
#[derive(Debug, Clone, Deserialize)]
pub struct TailscaleConfig {
    /// OAuth client ID (e.g., "k1AbCd2EfGh3")
    #[serde(rename = "hub_tailscale_client_id")]
    pub client_id: Option<SecretString>,
    /// OAuth client secret (e.g., "tskey-client-k1AbCd2EfGh3-123abc")
    #[serde(rename = "hub_tailscale_client_secret")]
    pub client_secret: Option<SecretString>,
}

impl TailscaleConfig {
    /// Validate that both credentials are present or both are absent
    ///
    /// Returns an error if only one credential is provided.
    pub fn validate(&self) -> Result<(), String> {
        match (&self.client_id, &self.client_secret) {
            (Some(_), None) => Err(
                "HUB_TAILSCALE_CLIENT_SECRET is required when HUB_TAILSCALE_CLIENT_ID is set"
                    .to_string(),
            ),
            (None, Some(_)) => Err(
                "HUB_TAILSCALE_CLIENT_ID is required when HUB_TAILSCALE_CLIENT_SECRET is set"
                    .to_string(),
            ),
            _ => Ok(()),
        }
    }

    /// Get the OAuth credentials if both are present
    pub fn oauth(&self) -> Option<TailscaleOAuth> {
        match (&self.client_id, &self.client_secret) {
            (Some(id), Some(secret)) => Some(TailscaleOAuth {
                client_id: id.clone(),
                client_secret: secret.clone(),
            }),
            _ => None,
        }
    }
}

/// Tailscale OAuth credentials (both client_id and client_secret present)
#[derive(Debug, Clone)]
pub struct TailscaleOAuth {
    pub client_id: SecretString,
    pub client_secret: SecretString,
}

/// Main application configuration containing all sub-configurations
#[derive(Deserialize)]
pub struct Config {
    /// Log level for the application
    ///
    /// This value is used to set the log level for this application's target specifically.
    /// e.g. "debug" would be similar to "warn,podpilot_hub=debug,..."
    ///
    /// Valid values are: "trace", "debug", "info", "warn", "error"
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_port")]
    pub port: u16,
    /// Database connection URL
    pub database_url: String,
    /// Graceful shutdown timeout duration
    ///
    /// Accepts both numeric values (seconds) and duration strings
    #[serde(
        default = "default_shutdown_timeout",
        deserialize_with = "deserialize_duration"
    )]
    pub shutdown_timeout: Duration,
    /// Tailscale OAuth configuration for Hub authentication (optional)
    ///
    /// When running locally with an existing Tailscale daemon, this is not needed.
    /// When running in Docker/Railway, provide OAuth credentials to connect to tailnet.
    ///
    /// Both client_id and client_secret must be provided together via:
    /// - HUB_TAILSCALE_CLIENT_ID
    /// - HUB_TAILSCALE_CLIENT_SECRET
    #[serde(flatten)]
    pub tailscale: TailscaleConfig,
}

/// Default log level of "info"
fn default_log_level() -> String {
    "info".to_string()
}

/// Default port of 80
fn default_port() -> u16 {
    80
}

/// Default shutdown timeout of 8 seconds
fn default_shutdown_timeout() -> Duration {
    Duration::from_secs(8)
}

/// Duration parser configured to handle various time units with seconds as default
///
/// Supports:
/// - Seconds (s) - default unit
/// - Milliseconds (ms)
/// - Minutes (m)
/// - Hours (h)
///
/// Does not support fractions, exponents, or infinity values
/// Allows for whitespace between the number and the time unit
/// Allows for multiple time units to be specified (summed together, e.g "10s 2m" = 120 + 10 = 130 seconds)
const DURATION_PARSER: DurationParser<'static> = DurationParser::builder()
    .time_units(&[TimeUnit::Second, TimeUnit::MilliSecond, TimeUnit::Minute])
    .parse_multiple(None)
    .allow_time_unit_delimiter()
    .disable_infinity()
    .disable_fraction()
    .disable_exponent()
    .default_unit(TimeUnit::Second)
    .build();

/// Custom deserializer for duration fields that accepts both numeric and string values
///
/// This deserializer handles the flexible duration parsing by accepting:
/// - Unsigned integers (interpreted as seconds)
/// - Signed integers (interpreted as seconds, must be non-negative)
/// - Strings (parsed using the fundu duration parser)
///
/// # Examples
///
/// - `1` -> 1 second
/// - `"30s"` -> 30 seconds
/// - `"2 m"` -> 2 minutes
/// - `"1500ms"` -> 15 seconds
fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Visitor;

    struct DurationVisitor;

    impl<'de> Visitor<'de> for DurationVisitor {
        type Value = Duration;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a duration string or number")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            DURATION_PARSER.parse(value)
                .map_err(|e| {
                    serde::de::Error::custom(format!(
                        "Invalid duration format '{}': {}. Examples: '5' (5 seconds), '3500ms', '30s', '2m', '1.5h'",
                        value, e
                    ))
                })?
                .try_into()
                .map_err(|e| serde::de::Error::custom(format!("Duration conversion error: {}", e)))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Duration::from_secs(value))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if value < 0 {
                return Err(serde::de::Error::custom("Duration cannot be negative"));
            }
            Ok(Duration::from_secs(value as u64))
        }
    }

    deserializer.deserialize_any(DurationVisitor)
}
