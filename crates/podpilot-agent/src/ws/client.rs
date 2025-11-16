use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use podpilot_common::protocol::{
    AgentMessage, HeartbeatAckMessage, HubMessage, RegisterRequest, RegisterResponse,
};
use podpilot_common::types::{GpuInfo, ProviderType};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::RwLock;
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
    provider_instance_id: Option<String>,
    hostname: String,
    gpu_info: GpuInfo,
    tailscale_ip: String,
    agent_id: Arc<RwLock<Option<Uuid>>>,
    last_heartbeat: Arc<RwLock<DateTime<Utc>>>,
    shutdown: Arc<AtomicBool>,
}

impl WsClient {
    /// Create a new WebSocket client
    pub fn new(
        hub_url: String,
        provider: ProviderType,
        provider_instance_id: Option<String>,
        hostname: String,
        gpu_info: GpuInfo,
        tailscale_ip: String,
    ) -> Self {
        Self {
            hub_url,
            provider,
            provider_instance_id,
            hostname,
            gpu_info,
            tailscale_ip,
            agent_id: Arc::new(RwLock::new(None)),
            last_heartbeat: Arc::new(RwLock::new(Utc::now())),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Run the WebSocket client with automatic reconnection
    pub async fn run(&self) -> Result<()> {
        let mut backoff = RECONNECT_INITIAL_BACKOFF;

        loop {
            if self.shutdown.load(Ordering::SeqCst) {
                info!("WebSocket client shutting down");
                break;
            }

            match self.connect_and_handle().await {
                Ok(_) => {
                    info!("WebSocket connection closed normally");
                    backoff = RECONNECT_INITIAL_BACKOFF;
                }
                Err(e) => {
                    error!("WebSocket connection error: {}", e);
                    warn!("Reconnecting in {:.2?}", backoff);
                    tokio::time::sleep(backoff).await;

                    // Exponential backoff with max limit
                    backoff = std::cmp::min(
                        Duration::from_secs_f64(backoff.as_secs_f64() * RECONNECT_BACKOFF_MULTIPLIER),
                        RECONNECT_MAX_BACKOFF,
                    );
                }
            }
        }

        Ok(())
    }

    /// Connect to Hub and handle messages
    async fn connect_and_handle(&self) -> Result<()> {
        info!("Connecting to Hub at {}", self.hub_url);

        let (ws_stream, _) = connect_async(&self.hub_url)
            .await
            .context("Failed to connect to Hub WebSocket")?;

        info!("WebSocket connected, sending registration");

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // Send registration message
        let registration = self.create_registration_message();
        let registration_json = serde_json::to_string(&registration)?;
        ws_sender
            .send(Message::Text(registration_json.into()))
            .await?;

        // Wait for registration acknowledgment
        let reg_response = timeout(Duration::from_secs(30), ws_receiver.next())
            .await
            .context("Timeout waiting for registration ack")?
            .ok_or_else(|| anyhow::anyhow!("Connection closed before registration ack"))??;

        if let Message::Text(text) = reg_response {
            let hub_msg: HubMessage = serde_json::from_str(&text)?;
            match hub_msg {
                HubMessage::RegisterAck(ack) => {
                    self.handle_registration_ack(ack).await?;
                }
                HubMessage::Error { message, code, .. } => {
                    anyhow::bail!("Registration failed: {} ({})", message, code);
                }
                _ => {
                    anyhow::bail!("Unexpected message during registration");
                }
            }
        } else {
            anyhow::bail!("Expected text message for registration ack");
        }

        // Update last heartbeat time
        *self.last_heartbeat.write().await = Utc::now();

        // Spawn heartbeat timeout monitor
        let last_heartbeat = self.last_heartbeat.clone();
        let shutdown = self.shutdown.clone();
        let monitor = tokio::spawn(async move {
            let mut check_interval = interval(Duration::from_secs(5));
            loop {
                check_interval.tick().await;

                if shutdown.load(Ordering::SeqCst) {
                    break;
                }

                let last_hb = *last_heartbeat.read().await;
                let elapsed = Utc::now().signed_duration_since(last_hb);

                if elapsed > chrono::Duration::from_std(HEARTBEAT_TIMEOUT).unwrap() {
                    error!("No heartbeat received for {:.2?}, connection lost", HEARTBEAT_TIMEOUT);
                    break;
                }
            }
        });

        // Handle incoming messages
        while let Some(msg_result) = ws_receiver.next().await {
            if self.shutdown.load(Ordering::SeqCst) {
                info!("Shutdown signal received, closing connection");
                break;
            }

            match msg_result {
                Ok(Message::Text(text)) => {
                    if let Err(e) = self.handle_hub_message(&mut ws_sender, &text).await {
                        error!("Error handling Hub message: {}", e);
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("Hub closed connection");
                    break;
                }
                Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {
                    // WebSocket library handles these automatically
                }
                Ok(_) => {}
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
            }
        }

        // Cancel heartbeat monitor
        monitor.abort();

        Ok(())
    }

    /// Create registration message
    fn create_registration_message(&self) -> AgentMessage {
        AgentMessage::Register(RegisterRequest {
            correlation_id: Uuid::new_v4(),
            provider: self.provider,
            provider_instance_id: self.provider_instance_id.clone(),
            hostname: self.hostname.clone(),
            gpu_info: self.gpu_info.clone(),
            tailscale_ip: self.tailscale_ip.clone(),
            agent_version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }

    /// Handle registration acknowledgment
    async fn handle_registration_ack(&self, ack: RegisterResponse) -> Result<()> {
        info!(
            "Registration successful! Agent ID: {}, Hub version: {}",
            ack.agent_id, ack.hub_version
        );
        *self.agent_id.write().await = Some(ack.agent_id);
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
                debug!(
                    "Received heartbeat (seq: {}, correlation: {})",
                    hb.sequence, hb.correlation_id
                );

                // Update last heartbeat time
                *self.last_heartbeat.write().await = Utc::now();

                // Send heartbeat ack
                let ack = AgentMessage::HeartbeatAck(HeartbeatAckMessage {
                    correlation_id: hb.correlation_id,
                    timestamp: Utc::now(),
                });

                let ack_json = serde_json::to_string(&ack)?;
                ws_sender.send(Message::Text(ack_json.into())).await?;

                debug!("Sent heartbeat ack");
            }
            HubMessage::RegisterAck(_) => {
                warn!("Received unexpected RegisterAck message");
            }
            HubMessage::Error { message, code, .. } => {
                error!("Received error from Hub: {} ({})", message, code);
            }
        }

        Ok(())
    }

    /// Shutdown the client gracefully
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}
