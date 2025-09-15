use crate::app::App;
use crate::cli::Args;
use clap::Parser;
use std::process::ExitCode;
use tracing::info;

mod api;
mod app;
mod cli;
mod data;
mod signals;
mod state;
mod web;

#[tokio::main]
async fn main() -> ExitCode {
    dotenvy::dotenv().ok();

    // Parse CLI arguments
    let _args = Args::parse();

    // Create and initialize the application
    let app = App::new().await.expect("Failed to initialize application");

    // Setup logging
    podpilot_common::logging::setup_logging(app.config());

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

    // Run the application (Axum server + graceful shutdown)
    app.run().await
}
