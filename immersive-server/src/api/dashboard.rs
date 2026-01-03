//! Dashboard web server
//!
//! Serves the web dashboard on a configurable port (default 2900)

use axum::{
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};

/// Embedded dashboard HTML
const DASHBOARD_HTML: &str = include_str!("dashboard.html");

/// Dashboard handler - serves the embedded HTML
async fn dashboard_handler() -> impl IntoResponse {
    Html(DASHBOARD_HTML)
}

/// Create the dashboard router
pub fn create_dashboard_router() -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/", get(dashboard_handler))
        .layer(cors)
}

/// Run the dashboard server on the specified port
pub async fn run_dashboard_server(port: u16) -> Result<(), std::io::Error> {
    let app = create_dashboard_router();
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!("Dashboard server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}
