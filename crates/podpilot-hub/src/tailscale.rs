//! Tailscale integration for the podpilot-hub.
//!
//! This module manages the Tailscale daemon lifecycle and provides
//! functionality to query the node's Tailscale IP address.

use anyhow::{Context, Result, anyhow};
use podpilot_common::config::Config;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use std::net::IpAddr;
use std::process::{Child, Command};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;

use crate::state::AppState;

/// Response from the Tailscale local API /status endpoint
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct TailscaleStatus {
    backend_state: String,
    #[serde(rename = "Self")]
    self_: Option<TailscaleSelf>,
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
        tracing::info!(
            "Detected existing Tailscale daemon at {}",
            socket_path.display()
        );
        true
    } else {
        tracing::debug!(
            "No existing Tailscale daemon detected (no socket at {})",
            socket_path.display()
        );
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
        // Spawn our own daemon with userspace networking
        let child = spawn_tailscaled_userspace().context("Failed to spawn tailscaled daemon")?;

        // Store the process handle for automatic cleanup on Drop
        {
            let mut process = TAILSCALED_PROCESS.write().await;
            *process = Some(TailscaledHandle::new(child));
        }

        // Wait for daemon to initialize (socket creation + backend startup)
        tracing::debug!("Waiting 2 seconds for Tailscale daemon to initialize");
        sleep(Duration::from_secs(2)).await;

        // Wait for daemon to be ready to accept commands
        wait_for_daemon_ready()
            .await
            .context("Tailscale daemon failed to become ready")?;

        tracing::info!("Tailscale daemon is ready (responsive to commands)");

        // Connect to tailnet if OAuth credentials provided
        if let Some(oauth) = config.tailscale.oauth() {
            connect_to_tailnet(
                &oauth.client_id,
                &oauth.client_secret,
            )
            .await
            .context("Failed to connect to Tailscale network with OAuth credentials")?;

            tracing::info!("Initiated connection to Tailscale network");

            // Wait for full authentication and connection
            wait_for_connection()
                .await
                .context("Tailscale failed to fully authenticate and connect")?;

            tracing::info!("Successfully connected to Tailscale network with OAuth credentials");
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

/// Wait for the Tailscale daemon to be ready to accept commands
///
/// "Ready" means the daemon responds to `tailscale status --json` with a successful exit code.
/// The --json flag ensures exit code 0 even when not authenticated (NeedsLogin state).
/// This does NOT mean the daemon is authenticated or connected to a tailnet.
async fn wait_for_daemon_ready() -> Result<()> {
    let max_attempts = 50;
    let poll_interval = Duration::from_millis(200);
    let start_time = std::time::Instant::now();
    let mut last_error = String::new();

    tracing::debug!("Waiting for Tailscale daemon to become ready (responsive to commands)");

    for attempt in 1..=max_attempts {
        let result = tokio::process::Command::new("tailscale")
            .args(["status", "--json"])
            .output()
            .await;

        match result {
            Ok(output) if output.status.success() => {
                let elapsed = start_time.elapsed();
                tracing::debug!(
                    attempts = attempt,
                    elapsed_ms = elapsed.as_millis(),
                    "Tailscale daemon is ready"
                );
                return Ok(());
            }
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                last_error = format!(
                    "Command failed with exit code {:?}\nstdout: {}\nstderr: {}",
                    output.status.code(),
                    stdout.trim(),
                    stderr.trim()
                );
                tracing::debug!(
                    attempt,
                    max_attempts,
                    error = %last_error,
                    "Daemon not ready yet"
                );
            }
            Err(e) => {
                last_error = format!("Failed to execute tailscale command: {}", e);
                tracing::debug!(
                    attempt,
                    max_attempts,
                    error = %last_error,
                    "Daemon not ready yet"
                );
            }
        }

        if attempt < max_attempts {
            sleep(poll_interval).await;
        }
    }

    let elapsed = start_time.elapsed();
    let timeout_ms = max_attempts * poll_interval.as_millis() as u32;

    let mut error_msg = format!(
        "Tailscale daemon did not become ready after {} attempts ({} ms elapsed, {} ms timeout)",
        max_attempts,
        elapsed.as_millis(),
        timeout_ms
    );

    if !last_error.is_empty() {
        error_msg.push_str(&format!("\n\nLast error: {}", last_error));
    }

    Err(anyhow!(error_msg))
}

/// Wait for Tailscale to be fully connected and authenticated
///
/// Polls until BackendState is "Running" and the node has Tailscale IPs assigned.
/// This should be called after `tailscale up` to ensure full authentication.
async fn wait_for_connection() -> Result<()> {
    let max_attempts = 60;
    let poll_interval = Duration::from_millis(500);
    let start_time = std::time::Instant::now();
    let mut last_backend_state = String::new();

    tracing::debug!("Waiting for Tailscale to connect and authenticate");

    for attempt in 1..=max_attempts {
        match fetch_tailscale_status().await {
            Ok(status) => {
                last_backend_state = status.backend_state.clone();

                if status.backend_state == "Running" {
                    if let Some(ref self_info) = status.self_ {
                        if !self_info.tailscale_i_ps.is_empty() {
                            let elapsed = start_time.elapsed();
                            tracing::debug!(
                                attempts = attempt,
                                elapsed_ms = elapsed.as_millis(),
                                ips = ?self_info.tailscale_i_ps,
                                "Tailscale is fully connected"
                            );
                            return Ok(());
                        }
                    }
                }

                tracing::debug!(
                    attempt,
                    max_attempts,
                    backend_state = %status.backend_state,
                    has_self = status.self_.is_some(),
                    "Waiting for connection"
                );
            }
            Err(e) => {
                tracing::debug!(
                    attempt,
                    max_attempts,
                    error = %e,
                    "Failed to fetch status while waiting for connection"
                );
            }
        }

        if attempt < max_attempts {
            sleep(poll_interval).await;
        }
    }

    let elapsed = start_time.elapsed();
    let timeout_ms = max_attempts * poll_interval.as_millis() as u32;

    Err(anyhow!(
        "Tailscale did not connect after {} attempts ({} ms elapsed, {} ms timeout). Last state: {}",
        max_attempts,
        elapsed.as_millis(),
        timeout_ms,
        last_backend_state
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
async fn connect_to_tailnet(client_id: &SecretString, client_secret: &SecretString) -> Result<()> {
    const HOSTNAME: &str = "podpilot-hub";
    const TAGS: &str = "tag:podpilot-hub";

    // Validate all inputs (even hardcoded ones, for defense in depth)
    validate_hostname(HOSTNAME).context("Invalid hostname")?;
    validate_tags(TAGS).context("Invalid tags")?;

    tracing::debug!(
        hostname = HOSTNAME,
        tags = TAGS,
        "Connecting to Tailscale network"
    );

    // Use separate arguments instead of format! to avoid shell injection
    let output = Command::new("tailscale")
        .arg("up")
        .arg("--client-id")
        .arg(client_id.expose_secret())
        .arg("--client-secret")
        .arg(client_secret.expose_secret())
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
    let self_info = status
        .self_
        .as_ref()
        .ok_or_else(|| anyhow!("Tailscale not authenticated (Self is null)"))?;

    self_info
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
    tracing::info!(
        interval_secs = interval.as_secs(),
        "Starting Tailscale IP updater task"
    );

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
