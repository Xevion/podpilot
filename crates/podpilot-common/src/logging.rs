use crate::config::Config;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

/// Configure and initialize logging for the application
pub fn setup_logging(config: &Config) {
    // Configure logging based on config
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let base_level = &config.log_level;
        EnvFilter::new(format!("warn,podpilot_hub={}", base_level))
    });

    let subscriber = FmtSubscriber::builder()
        .with_target(true)
        .with_env_filter(filter)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}
