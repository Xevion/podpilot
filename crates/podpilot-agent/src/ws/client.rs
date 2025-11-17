use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use podpilot_common::protocol::{
    AgentInfo, AgentMessage, AgentRegistration, HeartbeatAckMessage, HubMessage,
};
use podpilot_common::types::{GpuInfo, ProviderType};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, watch};
use tokio::time::{interval, timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(30);
const RECONNECT_INITIAL_BACKOFF: Duration = Duration::from_secs(1);
const RECONNECT_MAX_BACKOFF: Duration = Duration::from_secs(60);
const RECONNECT_BACKOFF_MULTIPLIER: f64 = 2.0;

/// WebSocket client for Agent-to-Hub communication
#[derive(Clone)]
pub struct WsClient {
    hub_url: String,
    provider: ProviderType,
    provider_instance_id: String,
    hostname: String,
    gpu_info: GpuInfo,
    tailscale_ip: IpAddr,
    agent_id: Arc<RwLock<Option<Uuid>>>,
    last_heartbeat: Arc<RwLock<DateTime<Utc>>>,
    shutdown_tx: Arc<watch::Sender<bool>>,
    shutdown_rx: watch::Receiver<bool>,
}

impl WsClient {
    /// Create a new WebSocket client
    pub fn new(
        hub_url: String,
        provider: ProviderType,
        provider_instance_id: String,
        hostname: String,
        gpu_info: GpuInfo,
        tailscale_ip: IpAddr,
    ) -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        Self {
            hub_url,
            provider,
            provider_instance_id,
            hostname,
            gpu_info,
            tailscale_ip,
            agent_id: Arc::new(RwLock::new(None)),
            last_heartbeat: Arc::new(RwLock::new(Utc::now())),
            shutdown_tx: Arc::new(shutdown_tx),
            shutdown_rx,
        }
    }

    /// Run the WebSocket client with automatic reconnection
    pub async fn run(&self) -> Result<()> {
        let mut backoff = RECONNECT_INITIAL_BACKOFF;
        let mut shutdown_rx = self.shutdown_rx.clone();
        let mut reconnect_count: u32 = 0;

        loop {
            // Check if shutdown was already signaled to avoid deadlock
            if *shutdown_rx.borrow() {
                debug!("shutdown initiated");
                break;
            }

            tokio::select! {
                _ = shutdown_rx.changed() => {
                    debug!("shutdown initiated");
                    break;
                }
                result = self.connect_and_handle(reconnect_count) => {
                    match result {
                        Ok(_) => {
                            info!("connection closed normally");
                            backoff = RECONNECT_INITIAL_BACKOFF;
                            reconnect_count = 0;
                        }
                        Err(e) => {
                            reconnect_count += 1;
                            error!(
                                error = %e,
                                attempt = reconnect_count,
                                backoff_secs = backoff.as_secs_f64(),
                                "connection failed, will retry"
                            );
                            tokio::time::sleep(backoff).await;

                            // Exponential backoff with max limit
                            backoff = std::cmp::min(
                                Duration::from_secs_f64(backoff.as_secs_f64() * RECONNECT_BACKOFF_MULTIPLIER),
                                RECONNECT_MAX_BACKOFF,
                            );
                        }
                    }
                }
            }
        }

        info!("shutdown complete");
        Ok(())
    }

    /// Connect to Hub and handle messages
    async fn connect_and_handle(&self, attempt: u32) -> Result<()> {
        let session_start = Instant::now();
        let connect_start = Instant::now();

        info!(
            hub_url = %self.hub_url,
            attempt = if attempt == 0 { 1 } else { attempt },
            "connecting to hub"
        );

        let (ws_stream, _) = connect_async(&self.hub_url).await?;

        info!(
            connect_duration_ms = connect_start.elapsed().as_millis() as u64,
            "connected, sending registration"
        );

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // Send registration message
        let registration = self.create_registration_message();
        let registration_json = serde_json::to_string(&registration)?;
        ws_sender.send(Message::Text(registration_json)).await?;

        // Wait for registration acknowledgment
        let reg_response = timeout(Duration::from_secs(30), ws_receiver.next())
            .await
            .context("Timeout waiting for registration ack (30s)")?
            .ok_or_else(|| anyhow::anyhow!("Connection closed during registration"))??;

        if let Message::Text(text) = reg_response {
            let hub_msg: HubMessage =
                serde_json::from_str(&text).context("Failed to parse registration response")?;
            match hub_msg {
                HubMessage::RegisterAck(ack) => {
                    self.handle_registration_ack(ack).await?;
                }
                HubMessage::Error { message, code, .. } => {
                    anyhow::bail!("Registration rejected by hub [code: {}]: {}", code, message);
                }
                _ => {
                    anyhow::bail!("Unexpected message type during registration: {:?}", hub_msg);
                }
            }
        } else {
            anyhow::bail!(
                "Expected text message for registration ack, received: {:?}",
                reg_response
            );
        }

        // Update last heartbeat time
        *self.last_heartbeat.write().await = Utc::now();

        // Spawn heartbeat timeout monitor
        let last_heartbeat = self.last_heartbeat.clone();
        let mut shutdown_rx = self.shutdown_rx.clone();
        let monitor = tokio::spawn(async move {
            let mut check_interval = interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        debug!("heartbeat monitor shutdown");
                        break;
                    }
                    _ = check_interval.tick() => {
                        let last_hb = *last_heartbeat.read().await;
                        let elapsed = Utc::now().signed_duration_since(last_hb);

                        if elapsed > chrono::Duration::from_std(HEARTBEAT_TIMEOUT).unwrap() {
                            error!(timeout_secs = HEARTBEAT_TIMEOUT.as_secs(), "no heartbeat received, connection lost");
                            break;
                        }
                    }
                }
            }
        });

        // Handle incoming messages
        let mut shutdown_rx = self.shutdown_rx.clone();

        let close_reason = loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    debug!("closing connection due to shutdown");
                    // Send close frame to Hub
                    let _ = ws_sender.send(Message::Close(None)).await;
                    break "shutdown";
                }
                msg_result = ws_receiver.next() => {
                    match msg_result {
                        Some(Ok(Message::Text(text))) => {
                            if let Err(e) = self.handle_hub_message(&mut ws_sender, &text).await {
                                error!(error = %e, "error handling hub message");
                            }
                        }
                        Some(Ok(Message::Close(_))) => {
                            break "hub_closed";
                        }
                        Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {
                            // WebSocket library handles these automatically
                        }
                        Some(Ok(_)) => {}
                        Some(Err(e)) => {
                            error!(error = %e, "websocket error");
                            break "error";
                        }
                        None => {
                            break "stream_ended";
                        }
                    }
                }
            }
        };

        // Cancel heartbeat monitor
        monitor.abort();

        info!(
            session_duration_secs = session_start.elapsed().as_secs(),
            reason = close_reason,
            "connection closed"
        );

        Ok(())
    }

    /// Create registration message
    fn create_registration_message(&self) -> AgentMessage {
        AgentMessage::Register(AgentInfo {
            correlation_id: Uuid::new_v4(),
            provider: self.provider,
            provider_instance_id: self.provider_instance_id.clone(),
            hostname: self.hostname.clone(),
            gpu_info: self.gpu_info.clone(),
            tailscale_ip: self.tailscale_ip,
            agent_version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }

    /// Handle registration acknowledgment
    async fn handle_registration_ack(&self, ack: AgentRegistration) -> Result<()> {
        let agent_id = ack.agent_id;
        *self.agent_id.write().await = Some(agent_id);

        info!(
            agent_id = %agent_id,
            hub_version = %ack.hub_version,
            gpu_name = %self.gpu_info.name,
            provider = ?self.provider,
            "connected to hub"
        );
        Ok(())
    }

    /// Handle incoming message from Hub
    async fn handle_hub_message(
        &self,
        ws_sender: &mut futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            Message,
        >,
        text: &str,
    ) -> Result<()> {
        let hub_msg: HubMessage = serde_json::from_str(text)?;

        match hub_msg {
            HubMessage::Heartbeat(hb) => {
                debug!(sequence = hb.sequence, correlation_id = %hb.correlation_id, "received heartbeat");

                // Update last heartbeat time
                *self.last_heartbeat.write().await = Utc::now();

                // Send heartbeat ack
                let ack = AgentMessage::HeartbeatAck(HeartbeatAckMessage {
                    correlation_id: hb.correlation_id,
                    timestamp: Utc::now(),
                });

                let ack_json = serde_json::to_string(&ack)?;
                ws_sender.send(Message::Text(ack_json)).await?;

                debug!("sent heartbeat ack");
            }
            HubMessage::RegisterAck(_) => {
                warn!("received unexpected register ack");
            }
            HubMessage::Error { message, code, .. } => {
                error!(error_code = code, error_message = %message, "received error from hub");
            }
        }

        Ok(())
    }

    /// Shutdown the client gracefully
    pub fn shutdown(&self) {
        debug!("shutdown requested");
        let _ = self.shutdown_tx.send(true);
    }
}
