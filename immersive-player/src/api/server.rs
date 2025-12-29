//! WebSocket server for real-time control
//!
//! Provides a WebSocket server that clients can connect to for remote control.

#![allow(dead_code)]

use super::handlers::CommandHandler;
use super::protocol::{ClientMessage, ServerMessage, StateUpdate};
use crate::composition::Composition;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, RwLock};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

/// WebSocket server for remote composition control
pub struct WebSocketServer {
    /// Shared composition state
    composition: Arc<RwLock<Composition>>,
    /// Broadcast channel for state updates
    broadcast_tx: broadcast::Sender<ServerMessage>,
    /// Server address
    addr: SocketAddr,
    /// Connected clients
    clients: Arc<RwLock<HashMap<Uuid, ClientInfo>>>,
}

/// Information about a connected client
#[derive(Debug)]
struct ClientInfo {
    /// Client ID
    id: Uuid,
    /// Remote address
    addr: SocketAddr,
}

impl WebSocketServer {
    /// Create a new WebSocket server
    pub fn new(composition: Arc<RwLock<Composition>>, port: u16) -> Self {
        let (broadcast_tx, _) = broadcast::channel(100);
        let addr = SocketAddr::from(([0, 0, 0, 0], port));

        Self {
            composition,
            broadcast_tx,
            addr,
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the server address
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Get the broadcast sender for sending updates
    pub fn broadcast_sender(&self) -> broadcast::Sender<ServerMessage> {
        self.broadcast_tx.clone()
    }

    /// Broadcast a state update to all clients
    pub async fn broadcast_state(&self) {
        let comp = self.composition.read().await;
        let update = ServerMessage::StateUpdate(StateUpdate::from_composition(&comp));
        let _ = self.broadcast_tx.send(update);
    }

    /// Start the WebSocket server
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind(self.addr).await?;
        log::info!("WebSocket server listening on ws://{}", self.addr);

        while let Ok((stream, addr)) = listener.accept().await {
            let composition = self.composition.clone();
            let broadcast_tx = self.broadcast_tx.clone();
            let clients = self.clients.clone();

            tokio::spawn(async move {
                if let Err(e) =
                    Self::handle_connection(stream, addr, composition, broadcast_tx, clients).await
                {
                    log::error!("Error handling WebSocket connection from {}: {}", addr, e);
                }
            });
        }

        Ok(())
    }

    /// Handle a single WebSocket connection
    async fn handle_connection(
        stream: TcpStream,
        addr: SocketAddr,
        composition: Arc<RwLock<Composition>>,
        broadcast_tx: broadcast::Sender<ServerMessage>,
        clients: Arc<RwLock<HashMap<Uuid, ClientInfo>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ws_stream = tokio_tungstenite::accept_async(stream).await?;
        let client_id = Uuid::new_v4();

        log::info!("New WebSocket connection from {} (id: {})", addr, client_id);

        // Register client
        {
            let mut clients_guard = clients.write().await;
            clients_guard.insert(client_id, ClientInfo { id: client_id, addr });
        }

        let (mut write, mut read) = ws_stream.split();
        let handler = CommandHandler::new(composition.clone());
        let mut broadcast_rx = broadcast_tx.subscribe();

        // Send initial state
        let comp = composition.read().await;
        let initial_state = ServerMessage::StateUpdate(StateUpdate::from_composition(&comp));
        drop(comp);

        let initial_json = serde_json::to_string(&initial_state)?;
        write.send(Message::Text(initial_json)).await?;

        loop {
            tokio::select! {
                // Handle incoming messages
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            match serde_json::from_str::<ClientMessage>(&text) {
                                Ok(client_msg) => {
                                    let response = handler.handle(client_msg).await;
                                    let response_json = serde_json::to_string(&response)?;
                                    write.send(Message::Text(response_json)).await?;

                                    // Broadcast state change to other clients
                                    if !matches!(response, ServerMessage::Pong | ServerMessage::Error { .. }) {
                                        let comp = composition.read().await;
                                        let update = ServerMessage::StateUpdate(StateUpdate::from_composition(&comp));
                                        let _ = broadcast_tx.send(update);
                                    }
                                }
                                Err(e) => {
                                    let error = ServerMessage::Error {
                                        message: format!("Invalid message: {}", e),
                                    };
                                    let error_json = serde_json::to_string(&error)?;
                                    write.send(Message::Text(error_json)).await?;
                                }
                            }
                        }
                        Some(Ok(Message::Close(_))) => {
                            log::info!("Client {} disconnected", client_id);
                            break;
                        }
                        Some(Ok(Message::Ping(data))) => {
                            write.send(Message::Pong(data)).await?;
                        }
                        Some(Err(e)) => {
                            log::error!("WebSocket error for client {}: {}", client_id, e);
                            break;
                        }
                        None => break,
                        _ => {}
                    }
                }

                // Handle broadcast messages
                broadcast_msg = broadcast_rx.recv() => {
                    if let Ok(msg) = broadcast_msg {
                        let msg_json = serde_json::to_string(&msg)?;
                        if write.send(Message::Text(msg_json)).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }

        // Unregister client
        {
            let mut clients_guard = clients.write().await;
            clients_guard.remove(&client_id);
        }

        log::info!("Client {} removed", client_id);
        Ok(())
    }

    /// Get the number of connected clients
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }
}


