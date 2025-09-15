use tokio::signal;
use tracing::info;

/// Future that resolves when the process receives Ctrl+C or SIGTERM
///
/// Use this with axum's `with_graceful_shutdown` to drain connections.
pub async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
        info!("received ctrl+c, starting graceful shutdown");
    };

    #[cfg(unix)]
    let sigterm = async {
        use tokio::signal::unix::{SignalKind, signal};
        let mut sigterm_stream =
            signal(SignalKind::terminate()).expect("Failed to install SIGTERM signal handler");
        sigterm_stream.recv().await;
        info!("received SIGTERM, starting graceful shutdown");
    };

    #[cfg(not(unix))]
    let sigterm = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = sigterm => {}
    }
}
