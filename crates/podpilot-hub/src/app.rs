use crate::state::AppState;
use crate::web::create_router;
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
    pub async fn new(config: Config) -> Result<Self, anyhow::Error> {
        // Validate Tailscale configuration (both credentials present or both absent)
        config
            .tailscale
            .validate()
            .expect("Invalid Tailscale configuration");

        // Check if the database URL is via private networking
        let is_private = config.database_url.contains("railway.internal");
        let slow_threshold = if cfg!(debug_assertions) {
            Duration::from_secs(1)
        } else if is_private {
            Duration::from_millis(200)
        } else {
            Duration::from_millis(500)
        };

        let db_pool = PgPoolOptions::new()
            .min_connections(0)
            .max_connections(4)
            .acquire_slow_threshold(slow_threshold)
            .acquire_timeout(Duration::from_secs(4))
            .idle_timeout(Duration::from_secs(60 * 2))
            .max_lifetime(Duration::from_secs(60 * 30))
            .connect(&config.database_url)
            .await
            .expect("Failed to create database pool");

        info!(
            is_private = is_private,
            slow_threshold = format!("{:.2?}", slow_threshold),
            "database pool established"
        );

        // Run database migrations automatically
        info!("running database migrations");
        sqlx::migrate!("../../migrations")
            .run(&db_pool)
            .await
            .expect("Failed to run database migrations");
        info!("database migrations completed successfully");

        Self::validate_database_schema(&db_pool)
            .await
            .expect("Database schema validation failed");

        let app_state = AppState::new(db_pool.clone());

        // Initialize Tailscale (auto-detects existing daemon or spawns own)
        crate::tailscale::initialize(&config)
            .await
            .expect("Failed to initialize Tailscale");

        Ok(App {
            config,
            db: db_pool,
            state: app_state,
        })
    }

    /// Run the application: start Axum and handle graceful shutdown signals
    pub async fn run(self) -> ExitCode {
        use crate::signals::shutdown_signal;
        use crate::ws::{cleanup_task, heartbeat_sender_task};
        use std::sync::Arc;
        use std::sync::atomic::AtomicBool;

        let router = create_router(self.state.clone());
        let addr = SocketAddr::from(([0, 0, 0, 0], self.config.port));

        // Spawn background tasks
        let shutdown_flag = Arc::new(AtomicBool::new(false));

        let heartbeat_state = self.state.clone();
        let heartbeat_shutdown = shutdown_flag.clone();
        tokio::spawn(async move {
            heartbeat_sender_task(heartbeat_state, heartbeat_shutdown).await;
        });

        let cleanup_state = self.state.clone();
        let cleanup_shutdown = shutdown_flag.clone();
        tokio::spawn(async move {
            cleanup_task(cleanup_state, cleanup_shutdown).await;
        });

        // Spawn Tailscale IP updater task (always enabled)
        let tailscale_state = self.state.clone();
        let tailscale_shutdown = shutdown_flag.clone();
        tokio::spawn(async move {
            crate::tailscale::tailscale_ip_updater_task(
                tailscale_state,
                Duration::from_secs(60), // Hardcoded to 60 seconds
                tailscale_shutdown,
            )
            .await;
        });

        info!("Background tasks spawned (heartbeat sender, cleanup, tailscale updater)");

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

    /// Validate that critical database tables exist
    async fn validate_database_schema(pool: &sqlx::PgPool) -> Result<(), anyhow::Error> {
        use anyhow::Context;

        let critical_tables = ["agents", "assets", "models"];

        for table in critical_tables {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS (
                    SELECT FROM information_schema.tables
                    WHERE table_schema = 'public'
                    AND table_name = $1
                )",
            )
            .bind(table)
            .fetch_one(pool)
            .await
            .with_context(|| format!("Failed to check if table '{}' exists", table))?;

            if !exists {
                anyhow::bail!(
                    "Critical table '{}' does not exist in database schema",
                    table
                );
            }
        }

        info!("Database schema validation passed");
        Ok(())
    }
}
