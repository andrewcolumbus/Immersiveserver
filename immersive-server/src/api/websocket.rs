//! WebSocket handler for real-time state updates
//!
//! Provides a WebSocket endpoint at `/ws` that streams state updates to connected clients.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};

use super::shared::{SharedStateHandle, WsEvent, WsSnapshot};

/// WebSocket upgrade handler
///
/// Upgrades an HTTP connection to WebSocket and starts streaming events.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedStateHandle>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle an individual WebSocket connection
async fn handle_socket(socket: WebSocket, state: SharedStateHandle) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to broadcast events
    let mut rx = state.subscribe();

    // Send initial snapshot
    let snapshot = state.get_snapshot();
    let ws_snapshot = WsSnapshot::from(&snapshot);
    let initial_event = WsEvent::Snapshot(ws_snapshot);
    if let Ok(json) = serde_json::to_string(&initial_event) {
        if sender.send(Message::Text(json)).await.is_err() {
            return; // Client disconnected
        }
    }

    tracing::info!("WebSocket client connected");

    // Spawn task to handle incoming messages (for future bidirectional support)
    let _state_clone = state.clone(); // Reserved for future bidirectional commands
    let recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Handle incoming commands (optional, for bidirectional support)
                    if let Ok(cmd) = serde_json::from_str::<serde_json::Value>(&text) {
                        tracing::debug!("WebSocket received: {:?}", cmd);
                        // Future: Parse and execute commands
                    }
                }
                Ok(Message::Ping(data)) => {
                    // Pong is handled automatically by axum
                    tracing::trace!("WebSocket ping received");
                    let _ = data; // Silence unused warning
                }
                Ok(Message::Close(_)) => {
                    tracing::info!("WebSocket client requested close");
                    break;
                }
                Err(e) => {
                    tracing::warn!("WebSocket receive error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });

    // Forward broadcast events to WebSocket
    let send_task = tokio::spawn(async move {
        // Throttle FPS updates to avoid flooding (max 10 updates/sec)
        let mut last_fps_send = std::time::Instant::now();
        let fps_throttle = std::time::Duration::from_millis(100);

        loop {
            match rx.recv().await {
                Ok(event) => {
                    // Throttle FPS events
                    if matches!(event, WsEvent::Fps { .. }) {
                        if last_fps_send.elapsed() < fps_throttle {
                            continue;
                        }
                        last_fps_send = std::time::Instant::now();
                    }

                    if let Ok(json) = serde_json::to_string(&event) {
                        if sender.send(Message::Text(json)).await.is_err() {
                            break; // Client disconnected
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("WebSocket client lagged, skipped {} events", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    });

    // Wait for either task to complete (client disconnect)
    tokio::select! {
        _ = recv_task => {},
        _ = send_task => {},
    }

    tracing::info!("WebSocket client disconnected");
}
