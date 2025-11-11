use axum::{Json, Router, routing::get};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::process::ExitCode;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Serialize, Deserialize)]
struct StatusResponse {
    status: String,
    version: String,
}

async fn get_status() -> Json<StatusResponse> {
    Json(StatusResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[tokio::main]
async fn main() -> ExitCode {
    // Initialize tracing subscriber with env filter; default to warn, and trace for podpilot_agent
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("warn,podpilot_agent=trace"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_ansi(cfg!(debug_assertions))
        .compact()
        .init();

    let app = Router::new().route("/status", get(get_status));
    let addr = SocketAddr::from(([0, 0, 0, 0], 8081));

    tracing::info!(address = %addr, "starting agent API server");

    match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => {
            if let Err(error) = axum::serve(listener, app).await {
                tracing::error!(error = ?error, "axum server error");
                ExitCode::FAILURE
            } else {
                tracing::info!("axum server stopped");
                ExitCode::SUCCESS
            }
        }
        Err(error) => {
            tracing::error!(error = ?error, "failed to bind TCP listener");
            ExitCode::FAILURE
        }
    }
}
