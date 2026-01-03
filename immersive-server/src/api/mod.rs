//! REST API server for Immersive Server
//!
//! Provides HTTP endpoints and WebSocket for remote control and monitoring.

pub mod dashboard;
pub mod routes;
pub mod server;
pub mod shared;
pub mod state;
pub mod types;
pub mod websocket;

pub use dashboard::run_dashboard_server;
pub use server::{create_shared_state, run_server};
pub use shared::{
    ApiCommand, AppSnapshot, ClipSnapshot, EffectSnapshot, FileSnapshot, LayerSnapshot,
    SharedState, SharedStateHandle, SourceSnapshot, StreamingSnapshot, ViewportSnapshot,
    WsEvent, WsSnapshot,
};
pub use types::*;
