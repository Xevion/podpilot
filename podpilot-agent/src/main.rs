use futures::future::join_all;
use reqwest::Client;
use std::time::Duration;

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

#[tokio::main]
async fn main() {
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

    println!("Starting network health check...");

    loop {
        let checks = devices.iter().map(|device_name| {
            // Spawn a new asynchronous task for each device check
            tokio::spawn(check_device(device_name, client.clone()))
        });

        // Wait for all the spawned tasks to complete
        let results = join_all(checks).await;

        println!(
            "\n--- Network Status @ {} ---",
            chrono::Local::now().format("%H:%M:%S")
        );
        for result in results {
            match result {
                Ok(status) => {
                    if let Some(url) = status.reachable_url {
                        println!("✅ {:<20} | Reachable at: {}", status.name, url);
                    } else if let Some(err) = status.error {
                        println!("❌ {:<20} | Error: {}", status.name, err);
                    }
                }
                Err(e) => {
                    // This would happen if a tokio task itself panicked, which is rare.
                    println!("Critical task error: {}", e);
                }
            }
        }

        println!("Waiting 30 seconds before next check...");
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}
