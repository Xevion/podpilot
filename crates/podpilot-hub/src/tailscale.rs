//! Tailscale integration for the podpilot-hub.
//!
//! This module manages the Tailscale daemon lifecycle and provides
//! functionality to query the node's Tailscale IP address.

use anyhow::{anyhow, Context, Result};
use podpilot_common::config::Config;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use std::net::IpAddr;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;

use crate::state::AppState;

/// Response from the Tailscale local API /status endpoint
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TailscaleStatus {
    self_: TailscaleSelf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TailscaleSelf {
    tailscale_i_ps: Vec<IpAddr>,
}

/// Wrapper for Tailscale daemon process with automatic cleanup
///
/// Implements Drop to ensure the daemon is terminated gracefully when dropped.
struct TailscaledHandle {
    child: Child,
}

impl TailscaledHandle {
    fn new(child: Child) -> Self {
        Self { child }
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for TailscaledHandle {
    fn drop(&mut self) {
        tracing::info!("Shutting down Tailscale daemon (PID: {})", self.child.id());

        // Try to kill the process
        if let Err(e) = self.child.kill() {
            tracing::warn!("Failed to kill tailscaled process: {}", e);
            return;
        }

        // Wait for process to exit with timeout
        let pid = self.child.id();
        match self.child.wait() {
            Ok(status) => {
                tracing::info!("Tailscaled process exited: {}", status);
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to wait for tailscaled process (PID: {}) to exit: {}",
                    pid,
                    e
                );
            }
        }
    }
}

/// Tailscale daemon process handle
static TAILSCALED_PROCESS: once_cell::sync::Lazy<Arc<RwLock<Option<TailscaledHandle>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(None)));

/// Check if a Tailscale daemon is already running by checking for the socket file
fn detect_existing_daemon() -> bool {
    let socket_path = std::path::Path::new("/var/run/tailscale/tailscaled.sock");

    if socket_path.exists() {
        tracing::info!("Detected existing Tailscale daemon at {}", socket_path.display());
        true
    } else {
        tracing::debug!("No existing Tailscale daemon detected (no socket at {})", socket_path.display());
        false
    }
}

/// Initialize Tailscale - detect existing daemon or spawn our own
pub async fn initialize(config: &Config) -> Result<()> {
    tracing::info!("Initializing Tailscale integration");

    // Check if daemon already exists (e.g., running on host system)
    let daemon_exists = detect_existing_daemon();

    if daemon_exists {
        tracing::info!("Using existing host Tailscale daemon (local development mode)");
        // Skip spawning and skip connection (assume host is already connected)
        // The IP updater task will fetch the IP from the existing daemon
    } else {
        tracing::info!("Spawning Tailscale daemon with userspace networking (container mode)");

        // Spawn our own daemon with userspace networking
        let child = spawn_tailscaled_userspace()
            .context("Failed to spawn tailscaled daemon")?;

        // Store the process handle for automatic cleanup on Drop
        {
            let mut process = TAILSCALED_PROCESS.write().await;
            *process = Some(TailscaledHandle::new(child));
        }

        // Wait for daemon to be ready
        wait_for_daemon_ready()
            .await
            .context("Tailscale daemon failed to become ready")?;

        tracing::info!("Tailscale daemon is ready");

        // Connect to tailnet if OAuth credentials provided
        if let Some(oauth) = config.tailscale.oauth() {
            let authkey = oauth.authkey();

            connect_to_tailnet(&authkey)
                .await
                .context("Failed to connect to Tailscale network with OAuth credentials")?;

            tracing::info!("Connected to Tailscale network with OAuth credentials");
        } else {
            tracing::warn!(
                "No OAuth credentials provided (HUB_TAILSCALE_CLIENT_ID/HUB_TAILSCALE_CLIENT_SECRET), \
                 daemon started but not connected to tailnet"
            );
        }
    }

    Ok(())
}

/// Spawn tailscaled daemon with userspace networking (for containers)
fn spawn_tailscaled_userspace() -> Result<Child> {
    tracing::debug!("Spawning tailscaled daemon with userspace networking");

    let child = Command::new("tailscaled")
        .args([
            "--tun=userspace-networking",
            "--socks5-server=localhost:1055",
            "--state=/var/lib/tailscale/state",
            "--socket=/var/run/tailscale/tailscaled.sock",
        ])
        .spawn()
        .context("Failed to execute tailscaled command")?;

    Ok(child)
}

/// Wait for the Tailscale daemon to be ready by checking CLI availability
async fn wait_for_daemon_ready() -> Result<()> {
    let max_attempts = 30;
    let poll_interval = Duration::from_millis(500);

    tracing::debug!("Waiting for Tailscale daemon to become ready");

    for attempt in 1..=max_attempts {
        match fetch_tailscale_status().await {
            Ok(_) => {
                tracing::debug!(attempts = attempt, "Tailscale daemon is ready");
                return Ok(());
            }
            Err(e) => {
                tracing::trace!(attempt, error = %e, "Daemon not ready yet");
            }
        }

        if attempt < max_attempts {
            sleep(poll_interval).await;
        }
    }

    Err(anyhow!(
        "Tailscale daemon did not become ready after {} attempts",
        max_attempts
    ))
}

/// Validate authkey format to prevent command injection
///
/// Authkeys should only contain alphanumeric characters, hyphens, and colons.
fn validate_authkey(authkey: &str) -> Result<()> {
    if !authkey
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == ':')
    {
        return Err(anyhow!(
            "Invalid authkey format: contains disallowed characters"
        ));
    }
    Ok(())
}

/// Validate hostname format to prevent command injection
///
/// Hostnames should only contain alphanumeric characters and hyphens.
fn validate_hostname(hostname: &str) -> Result<()> {
    if !hostname.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return Err(anyhow!(
            "Invalid hostname format: contains disallowed characters"
        ));
    }
    Ok(())
}

/// Validate tags format to prevent command injection
///
/// Tags should only contain alphanumeric characters, hyphens, and colons (for tag: prefix).
fn validate_tags(tags: &str) -> Result<()> {
    if !tags
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == ':')
    {
        return Err(anyhow!(
            "Invalid tags format: contains disallowed characters"
        ));
    }
    Ok(())
}

/// Connect to the Tailscale network using OAuth credentials
///
/// Hostname and tags are hardcoded for Hub deployment.
/// The authkey is wrapped in SecretString to prevent accidental logging.
/// All inputs are validated to prevent command injection attacks.
async fn connect_to_tailnet(authkey: &SecretString) -> Result<()> {
    const HOSTNAME: &str = "podpilot-hub";
    const TAGS: &str = "tag:podpilot-hub";

    // Validate all inputs (even hardcoded ones, for defense in depth)
    validate_hostname(HOSTNAME).context("Invalid hostname")?;
    validate_tags(TAGS).context("Invalid tags")?;

    let authkey_value = authkey.expose_secret();
    validate_authkey(authkey_value).context("Invalid authkey")?;

    tracing::debug!(hostname = HOSTNAME, tags = TAGS, "Connecting to Tailscale network");

    // Use separate arguments instead of format! to avoid shell injection
    let output = Command::new("tailscale")
        .arg("up")
        .arg("--authkey")
        .arg(authkey_value)
        .arg("--hostname")
        .arg(HOSTNAME)
        .arg("--advertise-tags")
        .arg(TAGS)
        .arg("--accept-dns=false")
        .output()
        .context("Failed to execute tailscale up command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("tailscale up failed: {}", stderr));
    }

    Ok(())
}

/// Fetch the current Tailscale status using the CLI
async fn fetch_tailscale_status() -> Result<TailscaleStatus> {
    let output = tokio::process::Command::new("tailscale")
        .args(["status", "--json"])
        .output()
        .await
        .context("Failed to execute 'tailscale status --json' command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "tailscale status command failed: {}",
            stderr.trim()
        ));
    }

    let status = serde_json::from_slice::<TailscaleStatus>(&output.stdout)
        .context("Failed to parse Tailscale status JSON output")?;

    Ok(status)
}

/// Extract the Tailscale IP address from the status response
fn extract_tailscale_ip(status: &TailscaleStatus) -> Result<IpAddr> {
    status
        .self_
        .tailscale_i_ps
        .first()
        .copied()
        .ok_or_else(|| anyhow!("No Tailscale IPs found in status response"))
}

/// Background task that periodically fetches and updates the Tailscale IP
pub async fn tailscale_ip_updater_task(
    state: AppState,
    interval: Duration,
    shutdown: Arc<AtomicBool>,
) {
    tracing::info!(interval_secs = interval.as_secs(), "Starting Tailscale IP updater task");

    loop {
        // Try to fetch and update the Tailscale IP
        match fetch_tailscale_status().await {
            Ok(status) => match extract_tailscale_ip(&status) {
                Ok(ip) => {
                    let mut tailscale_ip = state.tailscale_ip.write().await;
                    if *tailscale_ip != Some(ip) {
                        tracing::info!(%ip, "Tailscale IP updated");
                        *tailscale_ip = Some(ip);
                    } else {
                        tracing::trace!(%ip, "Tailscale IP unchanged");
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to extract Tailscale IP from status");
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, "Failed to fetch Tailscale status");
            }
        }

        // Check shutdown flag
        if shutdown.load(Ordering::SeqCst) {
            tracing::info!("Tailscale IP updater task shutting down");
            break;
        }

        // Wait for the interval
        sleep(interval).await;
    }

    tracing::info!("Tailscale IP updater task stopped");
}
