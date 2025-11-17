use clap::Parser;
use figment::value::UncasedStr;
use figment::{Figment, providers::Env};
use podpilot_common::config::Config;
use podpilot_hub::app::App;
use podpilot_hub::cli::Args;
use std::process::ExitCode;
use tracing::info;

#[tokio::main]
async fn main() -> ExitCode {
    dotenvy::dotenv().ok();

    // Parse CLI arguments
    let _args = Args::parse();

    let config: Config = Figment::new()
        .merge(Env::raw().map(|k| {
            if k == UncasedStr::new("RAILWAY_DEPLOYMENT_DRAINING_SECONDS") {
                "SHUTDOWN_TIMEOUT".into()
            } else {
                k.into()
            }
        }))
        .extract()
        .expect("Failed to load config");

    podpilot_common::logging::setup_logging(&config);

    // Log application startup context
    info!(
        version = env!("CARGO_PKG_VERSION"),
        environment = if cfg!(debug_assertions) {
            "development"
        } else {
            "production"
        },
        "starting podpilot-hub"
    );

    // Create and initialize the application
    let app = App::new(config)
        .await
        .expect("Failed to initialize application");

    // Run the application (Axum server + graceful shutdown)
    app.run().await
}
