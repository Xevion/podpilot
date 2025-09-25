use axum::{Json, Router, routing::get};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
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
async fn main() {
    // Initialize tracing subscriber with env filter; default to info, and verbose for reqwest/hyper
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,reqwest=trace,hyper=trace"));

    #[cfg(debug_assertions)]
    fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_ansi(cfg!(debug_assertions))
        .compact()
        .init();

    let server_task = tokio::spawn(async move {
        let app = Router::new().route("/status", get(get_status));
        let addr = SocketAddr::from(([0, 0, 0, 0], 8081));
        println!("Agent API server listening on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });

    let _ = tokio::join!(server_task);
}
