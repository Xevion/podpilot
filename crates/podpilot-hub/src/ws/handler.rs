use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};
use podpilot_common::protocol::{AgentInfo, AgentMessage, AgentRegistration, HubMessage};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::state::AppState;

/// WebSocket upgrade handler for agent connections
pub async fn agent_websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(|socket| handle_agent_socket(socket, state))
}

/// Handle a single agent WebSocket connection
async fn handle_agent_socket(socket: WebSocket, state: AppState) {
    info!("New WebSocket connection from agent");

    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Wait for registration message with timeout
    let agent_id = match wait_for_registration(&mut ws_receiver, &mut ws_sender, &state).await {
        Ok(id) => {
            info!("Agent {} registered successfully", id);
            id
        }
        Err(e) => {
            error!("Registration failed: {}", e);
            let _ = ws_sender.close().await;
            return;
        }
    };

    info!("Agent {} connection established", agent_id);

    // Create channel for sending outbound messages to this agent
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<HubMessage>(32);

    // Register connection in AppState
    state.register_connection(agent_id, outbound_tx);

    // Spawn task to handle outbound messages (Hub -> Agent)
    let mut ws_sender_task = ws_sender;
    let outbound_task = tokio::spawn(async move {
        while let Some(message) = outbound_rx.recv().await {
            let json = match serde_json::to_string(&message) {
                Ok(j) => j,
                Err(e) => {
                    error!("Failed to serialize outbound message: {}", e);
                    continue;
                }
            };

            if let Err(e) = ws_sender_task.send(Message::Text(json.into())).await {
                error!("Failed to send message to WebSocket: {}", e);
                break;
            }
        }
        ws_sender_task
    });

    // Handle inbound messages (Agent -> Hub)
    while let Some(msg_result) = ws_receiver.next().await {
        match msg_result {
            Ok(Message::Close(_)) => {
                info!("Agent {} closed connection", agent_id);
                break;
            }
            Ok(Message::Ping(_)) => {
                // WebSocket library auto-responds to pings
            }
            Ok(Message::Text(text)) => {
                if let Err(e) = handle_agent_message(&state, agent_id, &text).await {
                    warn!("Error handling message from agent {}: {}", agent_id, e);
                }
            }
            Ok(_) => {}
            Err(e) => {
                error!("WebSocket error for agent {}: {}", agent_id, e);
                break;
            }
        }
    }

    // Cleanup on disconnect
    state.remove_connection(&agent_id);
    info!("Agent {} disconnected and removed from registry", agent_id);

    // Abort outbound task and retrieve sender for cleanup
    outbound_task.abort();
}

/// Wait for and process the registration message
async fn wait_for_registration(
    receiver: &mut futures_util::stream::SplitStream<WebSocket>,
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    state: &AppState,
) -> anyhow::Result<Uuid> {
    use anyhow::{Context, anyhow};
    use tokio::time::{Duration, timeout};

    // Wait for first message with 30s timeout
    let msg_result = timeout(Duration::from_secs(30), receiver.next())
        .await
        .context("Timeout waiting for registration")?;

    let msg = msg_result.ok_or_else(|| anyhow!("Connection closed before registration"))??;

    // Parse the registration message
    let text = match msg {
        Message::Text(t) => t,
        _ => return Err(anyhow!("Expected text message for registration")),
    };

    let agent_msg: AgentMessage =
        serde_json::from_str(&text).context("Failed to parse registration message")?;

    match agent_msg {
        AgentMessage::Register(req) => {
            // Create agent record in database
            let agent_id = create_agent_record(state, &req).await?;

            // Send registration acknowledgment
            let response = HubMessage::RegisterAck(AgentRegistration {
                correlation_id: req.correlation_id,
                agent_id,
                registered_at: chrono::Utc::now(),
                hub_version: env!("CARGO_PKG_VERSION").to_string(),
            });

            let response_json = serde_json::to_string(&response)
                .context("Failed to serialize registration response")?;

            sender
                .send(Message::Text(response_json.into()))
                .await
                .context("Failed to send registration ack")?;

            Ok(agent_id)
        }
        AgentMessage::HeartbeatAck(_) => {
            Err(anyhow!("Unexpected HeartbeatAck during registration"))
        }
    }
}

/// Handle incoming agent messages
async fn handle_agent_message(state: &AppState, agent_id: Uuid, text: &str) -> anyhow::Result<()> {
    let agent_msg: AgentMessage = serde_json::from_str(text)?;

    match agent_msg {
        AgentMessage::HeartbeatAck(ack) => {
            debug!(
                "Received heartbeat ack from agent {} (correlation: {})",
                agent_id, ack.correlation_id
            );

            // Update last_seen_at in database
            sqlx::query!(
                r#"
                UPDATE agents
                SET last_seen_at = NOW()
                WHERE id = $1
                "#,
                agent_id
            )
            .execute(&state.db)
            .await?;
        }
        AgentMessage::Register(_) => {
            warn!(
                "Received unexpected Register message from already-registered agent {}",
                agent_id
            );
        }
    }

    Ok(())
}

/// Create or update agent record in the database
///
/// Checks for an existing agent with the same (tailscale_ip, provider_instance_id).
/// If found, reuses the existing record and updates its status. Otherwise, creates a new agent.
async fn create_agent_record(state: &AppState, req: &AgentInfo) -> anyhow::Result<Uuid> {
    use crate::data::models::ProviderType as HubProviderType;
    use anyhow::Context;

    // Convert common types to Hub types for database
    let provider: HubProviderType = match req.provider {
        podpilot_common::types::ProviderType::VastAI => HubProviderType::VastAI,
        podpilot_common::types::ProviderType::Runpod => HubProviderType::Runpod,
        podpilot_common::types::ProviderType::Local => HubProviderType::Local,
    };

    let gpu_info_json =
        serde_json::to_value(&req.gpu_info).context("Failed to serialize GPU info")?;

    // Check for existing agent by (tailscale_ip, provider_instance_id)
    let existing_agent = sqlx::query_scalar!(
        r#"
        SELECT id FROM agents
        WHERE tailscale_ip = $1
          AND provider_instance_id = $2
          AND terminated_at IS NULL
        "#,
        req.tailscale_ip as _,
        &req.provider_instance_id
    )
    .fetch_optional(&state.db)
    .await
    .context("Failed to query for existing agent")?;

    if let Some(agent_id) = existing_agent {
        // Reuse existing agent - update status, hostname, and timestamp
        info!("Reusing existing agent record: {}", agent_id);

        sqlx::query!(
            r#"
            UPDATE agents
            SET status = 'registering'::agent_status,
                hostname = $2,
                gpu_info = $3,
                last_seen_at = NOW()
            WHERE id = $1
            "#,
            agent_id,
            &req.hostname,
            gpu_info_json
        )
        .execute(&state.db)
        .await
        .context("Failed to update existing agent record")?;

        Ok(agent_id)
    } else {
        // Create new agent
        info!("Creating new agent record");

        let agent_id = sqlx::query_scalar!(
            r#"
            INSERT INTO agents (
                provider, provider_instance_id, hostname, status, tailscale_ip, gpu_info,
                registered_at, last_seen_at
            )
            VALUES ($1, $2, $3, 'registering'::agent_status, $4, $5, NOW(), NOW())
            RETURNING id
            "#,
            provider as _,
            &req.provider_instance_id,
            &req.hostname,
            req.tailscale_ip as _,
            gpu_info_json
        )
        .fetch_one(&state.db)
        .await
        .context("Failed to create agent record")?;

        Ok(agent_id)
    }
}
