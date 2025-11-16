use chrono::Utc;
use podpilot_common::protocol::{HeartbeatMessage, HubMessage};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::{Duration, interval};
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::state::AppState;

/// Heartbeat sender task that periodically sends heartbeat pings to all connected agents
pub async fn heartbeat_sender_task(state: AppState, shutdown: Arc<AtomicBool>) {
    info!("Starting heartbeat sender task");

    let mut tick_interval = interval(Duration::from_secs(10));
    let mut sequence_map: HashMap<Uuid, u64> = HashMap::new();

    loop {
        tokio::select! {
            _ = tick_interval.tick() => {
                send_heartbeats(&state, &mut sequence_map).await;
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Heartbeat sender received shutdown signal");
                shutdown.store(true, Ordering::SeqCst);
                break;
            }
        }

        // Check shutdown flag
        if shutdown.load(Ordering::SeqCst) {
            info!("Heartbeat sender shutting down");
            break;
        }
    }

    info!("Heartbeat sender task stopped");
}

/// Send heartbeat pings to all connected agents
async fn send_heartbeats(state: &AppState, sequence_map: &mut HashMap<Uuid, u64>) {
    let connected_agents = state.connected_agents();

    if connected_agents.is_empty() {
        debug!("No connected agents to send heartbeats to");
        return;
    }

    debug!("Sending heartbeats to {} agents", connected_agents.len());

    for agent_id in connected_agents {
        // Get or initialize sequence number for this agent
        let sequence = sequence_map.entry(agent_id).or_insert(0);
        *sequence += 1;

        let heartbeat = HubMessage::Heartbeat(HeartbeatMessage {
            correlation_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            sequence: *sequence,
        });

        if let Err(e) = state.send_to_agent(&agent_id, heartbeat).await {
            error!("Failed to send heartbeat to agent {}: {}", agent_id, e);
            // Remove sequence entry for disconnected agents
            sequence_map.remove(&agent_id);
        }
    }
}
