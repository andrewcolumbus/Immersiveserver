//! Axum server setup and startup

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::sync::mpsc;
use tower_http::cors::{Any, CorsLayer};

use super::routes::create_router;
use super::shared::{ApiCommand, SharedState, SharedStateHandle};

/// Run the API server on the specified port with shared state
///
/// This function is intended to be run on a tokio runtime.
/// It will block until the server is shut down or the shutdown signal is received.
pub async fn run_server(
    port: u16,
    shared_state: SharedStateHandle,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> Result<(), std::io::Error> {
    // Enable CORS for cross-origin requests (dashboard on port 2900)
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = create_router(shared_state).layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    log::info!("API server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            // Wait for shutdown signal
            let _ = shutdown_rx.changed().await;
            log::info!("API server shutting down gracefully");
        })
        .await
}

/// Create a new shared state and command channel
///
/// Returns the shared state handle (for the API server) and the command receiver (for the main app)
pub fn create_shared_state() -> (SharedStateHandle, mpsc::UnboundedReceiver<ApiCommand>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let state = Arc::new(SharedState::new(tx));
    (state, rx)
}
