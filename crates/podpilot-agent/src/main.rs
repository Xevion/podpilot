use axum::{Json, Router, routing::get};
use podpilot_agent::{config::Config, gpu, ws::WsClient};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::process::ExitCode;
use std::time::Instant;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[derive(Serialize, Deserialize)]
struct StatusResponse {
    status: String,
    version: String,
    hub_connected: bool,
}

async fn get_status() -> Json<StatusResponse> {
    Json(StatusResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        hub_connected: false, // TODO: Track actual connection status
    })
}

#[tokio::main]
async fn main() -> ExitCode {
    let start_time = Instant::now();

    // Load configuration
    let config = match Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            return ExitCode::FAILURE;
        }
    };

    // Initialize logging based on config
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .json()
        .flatten_event(true)
        .init();

    info!(
        version = env!("CARGO_PKG_VERSION"),
        hub_url = %config.hub_url,
        provider = ?config.provider,
        "starting podpilot-agent"
    );

    // Detect GPU information
    let gpu_info = gpu::detect_gpu();
    info!(
        gpu_name = %gpu_info.name,
        memory_gb = gpu_info.memory_gb,
        cuda_version = %gpu_info.cuda_version,
        "GPU detected"
    );

    // Create WebSocket client
    let ws_client = WsClient::new(
        config.hub_url.clone(),
        config.provider,
        config.provider_instance_id.clone(),
        config.get_hostname(),
        gpu_info.clone(),
        config.tailscale_ip.clone(),
    );

    // Spawn WebSocket client task
    let ws_handle = {
        let ws_client = ws_client.clone();
        tokio::spawn(async move {
            if let Err(e) = ws_client.run().await {
                error!("WebSocket client error: {}", e);
            }
        })
    };

    // Create and run status API server
    let app = Router::new().route("/status", get(get_status));
    let addr = SocketAddr::from(([0, 0, 0, 0], config.status_port));

    info!(address = %addr, "starting status API server");

    let result = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => {
            // Run server with graceful shutdown
            if let Err(error) = axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal(start_time))
                .await
            {
                error!(error = ?error, "server error");
                ExitCode::FAILURE
            } else {
                info!("stopped gracefully");
                ExitCode::SUCCESS
            }
        }
        Err(error) => {
            error!(error = ?error, "failed to bind TCP listener");
            ExitCode::FAILURE
        }
    };

    // Shutdown WebSocket client
    let shutdown_start = Instant::now();
    let ws_shutdown_start = Instant::now();
    ws_client.shutdown();
    let _ = ws_handle.await;
    let ws_shutdown_duration = ws_shutdown_start.elapsed().as_millis() as u64;

    info!(
        total_shutdown_ms = shutdown_start.elapsed().as_millis() as u64,
        ws_client_ms = ws_shutdown_duration,
        graceful = true,
        "shutdown complete"
    );

    result
}

/// Wait for SIGTERM or SIGINT signal for graceful shutdown
async fn shutdown_signal(start_time: Instant) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!(
                signal = "SIGINT",
                uptime_secs = start_time.elapsed().as_secs(),
                "shutdown initiated"
            );
        }
        _ = terminate => {
            info!(
                signal = "SIGTERM",
                uptime_secs = start_time.elapsed().as_secs(),
                "shutdown initiated"
            );
        }
    }
}
