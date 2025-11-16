use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};
use podpilot_common::protocol::{AgentMessage, HubMessage, RegisterRequest, RegisterResponse};
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
    use anyhow::{anyhow, Context};
    use tokio::time::{timeout, Duration};

    // Wait for first message with 30s timeout
    let msg_result = timeout(Duration::from_secs(30), receiver.next())
        .await
        .context("Timeout waiting for registration")?;

    let msg = msg_result
        .ok_or_else(|| anyhow!("Connection closed before registration"))??;

    // Parse the registration message
    let text = match msg {
        Message::Text(t) => t,
        _ => return Err(anyhow!("Expected text message for registration")),
    };

    let agent_msg: AgentMessage = serde_json::from_str(&text)
        .context("Failed to parse registration message")?;

    match agent_msg {
        AgentMessage::Register(req) => {
            // Create agent record in database
            let agent_id = create_agent_record(state, &req).await?;

            // Send registration acknowledgment
            let response = HubMessage::RegisterAck(RegisterResponse {
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
async fn handle_agent_message(
    state: &AppState,
    agent_id: Uuid,
    text: &str,
) -> anyhow::Result<()> {
    let agent_msg: AgentMessage = serde_json::from_str(text)?;

    match agent_msg {
        AgentMessage::HeartbeatAck(ack) => {
            debug!(
                "Received heartbeat ack from agent {} (correlation: {})",
                agent_id, ack.correlation_id
            );

            // Update last_seen_at in database
            sqlx::query(
                r#"
                UPDATE agents
                SET last_seen_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(agent_id)
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

/// Create agent record in the database
async fn create_agent_record(
    state: &AppState,
    req: &RegisterRequest,
) -> anyhow::Result<Uuid> {
    use anyhow::Context;

    // Convert common types to Hub types for database
    let provider_str = match req.provider {
        podpilot_common::types::ProviderType::VastAI => "vastai",
        podpilot_common::types::ProviderType::Runpod => "runpod",
        podpilot_common::types::ProviderType::Local => "local",
    };

    let gpu_info_json = serde_json::to_value(&req.gpu_info)
        .context("Failed to serialize GPU info")?;

    // Use sqlx::query instead of query! macro to avoid type mapping issues
    let agent = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO agents (
            provider, provider_instance_id, hostname, status, gpu_info,
            registered_at, last_seen_at
        )
        VALUES ($1::provider_type, $2, $3, 'registering'::agent_status, $4, NOW(), NOW())
        RETURNING id
        "#,
    )
    .bind(provider_str)
    .bind(&req.provider_instance_id)
    .bind(&req.hostname)
    .bind(gpu_info_json)
    .fetch_one(&state.db)
    .await
    .context("Failed to create agent record")?;

    Ok(agent)
}
