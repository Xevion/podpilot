use crate::state::AppState;
use crate::web::create_router;
use figment::value::UncasedStr;
use figment::{Figment, providers::Env};
use podpilot_common::config::Config;
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use std::process::ExitCode;
use std::time::Duration;
use tracing::info;

/// Main application struct containing all necessary components
pub struct App {
    config: Config,
    state: AppState,
    #[allow(dead_code)]
    db: sqlx::PgPool,
}

impl App {
    /// Create a new App instance with all necessary components initialized
    pub async fn new() -> Result<Self, anyhow::Error> {
        // Load configuration
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

        // Check if the database URL is via private networking
        let is_private = config.database_url.contains("railway.internal");
        let slow_threshold = if is_private {
            Duration::from_millis(200)
        } else {
            Duration::from_millis(500)
        };

        // Create database connection pool
        let db_pool = PgPoolOptions::new()
            .min_connections(0)
            .max_connections(4)
            .acquire_slow_threshold(slow_threshold)
            .acquire_timeout(Duration::from_secs(4))
            .idle_timeout(Duration::from_secs(60 * 2))
            .max_lifetime(Duration::from_secs(60 * 30))
            .connect_lazy(&config.database_url)
            .expect("Failed to create database pool");

        info!(
            is_private = is_private,
            slow_threshold = format!("{:.2?}", slow_threshold),
            "database pool established"
        );

        let app_state = AppState::new(db_pool.clone());

        Ok(App {
            config,
            db: db_pool,
            state: app_state,
        })
    }

    /// Run the application: start Axum and handle graceful shutdown signals
    pub async fn run(self) -> ExitCode {
        use crate::signals::shutdown_signal;

        let router = create_router(self.state.clone());
        let addr = SocketAddr::from(([0, 0, 0, 0], self.config.port));

        tracing::info!(address = %addr, "starting axum web server");

        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                if let Err(error) = axum::serve(listener, router)
                    .with_graceful_shutdown(shutdown_signal())
                    .await
                {
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

    /// Get a reference to the configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get a reference to the app state
    #[allow(dead_code)]
    pub fn app_state(&self) -> &AppState {
        &self.state
    }
}
