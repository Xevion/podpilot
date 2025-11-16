use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::state::AppState;

/// Cleanup task that marks stale agents as 'error' and removes them from the connection registry
pub async fn cleanup_task(state: AppState, shutdown: Arc<AtomicBool>) {
    info!("Starting agent cleanup task");

    let mut tick_interval = interval(Duration::from_secs(15));

    loop {
        tokio::select! {
            _ = tick_interval.tick() => {
                cleanup_stale_agents(&state).await;
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Cleanup task received shutdown signal");
                shutdown.store(true, Ordering::SeqCst);
                break;
            }
        }

        // Check shutdown flag
        if shutdown.load(Ordering::SeqCst) {
            info!("Cleanup task shutting down");
            break;
        }
    }

    info!("Cleanup task stopped");
}

/// Find and mark stale agents as 'error', then remove from connection registry
async fn cleanup_stale_agents(state: &AppState) {
    // Query for agents that haven't sent a heartbeat in 30+ seconds
    // Only check agents that are in active states (not already error/terminated)
    let result = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id
        FROM agents
        WHERE status IN ('ready', 'running', 'idle')
          AND last_seen_at < NOW() - INTERVAL '30 seconds'
        "#,
    )
    .fetch_all(&state.db)
    .await;

    let stale_agents = match result {
        Ok(agents) => agents,
        Err(e) => {
            error!("Failed to query stale agents: {}", e);
            return;
        }
    };

    if stale_agents.is_empty() {
        return;
    }

    warn!(
        "Found {} stale agents (no heartbeat for 30+ seconds)",
        stale_agents.len()
    );

    for agent_id in stale_agents {
        // Mark agent as error in database
        if let Err(e) = sqlx::query(
            r#"
            UPDATE agents
            SET status = 'error'::agent_status,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(agent_id)
        .execute(&state.db)
        .await
        {
            error!("Failed to mark agent {} as error: {}", agent_id, e);
            continue;
        }

        // Remove from connection registry
        state.remove_connection(&agent_id);

        warn!("Marked agent {} as error due to missed heartbeats", agent_id);
    }
}
