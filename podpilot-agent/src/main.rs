use axum::{Json, Router, routing::get};
use futures::future::join_all;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};

// A small struct to hold the results for each device check
struct DeviceStatus {
    name: String,
    reachable_url: Option<String>,
    error: Option<String>,
}

/// Attempts to find a single reachable URL for a given device name.
async fn check_device(device_name: &str, client: Client) -> DeviceStatus {
    let urls_to_try = [
        format!("http://{}", device_name),
        format!("https://{}", device_name),
        format!("http://{}.pipefish-stargazer.ts.net", device_name),
        format!("https://{}.pipefish-stargazer.ts.net", device_name),
    ];

    for url in urls_to_try {
        match client.get(&url).send().await {
            Ok(response) => {
                // On the first successful connection, we're done with this device.
                let status_text = format!(
                    "{} {}",
                    response.status().as_u16(),
                    response.status().canonical_reason().unwrap_or("??")
                );
                return DeviceStatus {
                    name: device_name.to_string(),
                    reachable_url: Some(format!("({status_text}) {url}")),
                    error: None,
                };
            }
            Err(_) => {
                // Ignore the error and try the next URL for this device
                continue;
            }
        }
    }

    // If we get here, no URLs were reachable for this device.
    DeviceStatus {
        name: device_name.to_string(),
        reachable_url: None,
        error: Some("All URLs were unreachable".to_string()),
    }
}

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
    fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .compact()
        .init();

    let server_task = tokio::spawn(async move {
        let app = Router::new().route("/status", get(get_status));
        let addr = SocketAddr::from(([0, 0, 0, 0], 8081));
        println!("Agent API server listening on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });

    let health_check_task = tokio::spawn(async move {
        let client = Client::builder()
            .timeout(Duration::from_secs(5)) // Reduced timeout for faster checks
            .user_agent(format!("podpilot-agent/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("Failed to create HTTP client");

        let devices = [
            "ether",
            "lumine",
            "railway",
            "tower",
            "seer",
            "google-pixel-7-pro",
            "ether-wsl",
        ];

        info!("Starting network health check...");

        loop {
            let checks = devices.iter().map(|device_name| {
                // Spawn a new asynchronous task for each device check
                tokio::spawn(check_device(device_name, client.clone()))
            });

            // Wait for all the spawned tasks to complete
            let results = join_all(checks).await;

            info!(
                "--- Network Status @ {} ---",
                chrono::Local::now().format("%H:%M:%S")
            );
            for result in results {
                match result {
                    Ok(status) => {
                        if let Some(url) = status.reachable_url {
                            info!("✅ {:<20} | Reachable at: {}", status.name, url);
                        } else if let Some(err) = status.error {
                            info!("❌ {:<20} | Error: {}", status.name, err);
                        }
                    }
                    Err(e) => {
                        // This would happen if a tokio task itself panicked, which is rare.
                        info!("Critical task error: {}", e);
                    }
                }
            }

            info!("Waiting 30 seconds before next check...");
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });

    let _ = tokio::join!(server_task, health_check_task);
}
