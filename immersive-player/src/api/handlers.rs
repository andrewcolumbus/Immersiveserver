//! Command handlers for WebSocket messages
//!
//! Processes incoming client messages and updates composition state.

#![allow(dead_code)]

use super::protocol::{ClientMessage, LayerStateUpdate, ServerMessage, StateUpdate};
use crate::composition::{BlendMode, Composition};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Handles incoming WebSocket commands
pub struct CommandHandler {
    /// Shared composition state
    composition: Arc<RwLock<Composition>>,
}

impl CommandHandler {
    /// Create a new command handler
    pub fn new(composition: Arc<RwLock<Composition>>) -> Self {
        Self { composition }
    }

    /// Handle an incoming client message
    pub async fn handle(&self, message: ClientMessage) -> ServerMessage {
        match message {
            ClientMessage::TriggerClip { layer, column } => {
                self.trigger_clip(layer, column).await
            }
            ClientMessage::StopLayer { layer } => self.stop_layer(layer).await,
            ClientMessage::SetLayerOpacity { layer, opacity } => {
                self.set_layer_opacity(layer, opacity).await
            }
            ClientMessage::SetLayerBlendMode { layer, blend_mode } => {
                self.set_layer_blend_mode(layer, &blend_mode).await
            }
            ClientMessage::SetMasterOpacity { opacity } => {
                self.set_master_opacity(opacity).await
            }
            ClientMessage::RequestState => self.get_full_state().await,
            ClientMessage::Ping => ServerMessage::Pong,
        }
    }

    /// Trigger a clip
    async fn trigger_clip(&self, layer: usize, column: usize) -> ServerMessage {
        let mut comp = self.composition.write().await;
        comp.trigger_clip(layer, column);

        ServerMessage::ClipTriggered { layer, column }
    }

    /// Stop a layer
    async fn stop_layer(&self, layer: usize) -> ServerMessage {
        let mut comp = self.composition.write().await;
        comp.stop_layer(layer);

        ServerMessage::LayerStopped { layer }
    }

    /// Set layer opacity
    async fn set_layer_opacity(&self, layer_index: usize, opacity: f32) -> ServerMessage {
        let mut comp = self.composition.write().await;
        if let Some(layer) = comp.get_layer_by_index_mut(layer_index) {
            layer.opacity = opacity.clamp(0.0, 1.0);
            ServerMessage::LayerUpdate(LayerStateUpdate::from_layer(layer))
        } else {
            ServerMessage::Error {
                message: format!("Layer {} not found", layer_index),
            }
        }
    }

    /// Set layer blend mode
    async fn set_layer_blend_mode(&self, layer_index: usize, blend_mode: &str) -> ServerMessage {
        let mode = match blend_mode.to_lowercase().as_str() {
            "normal" => BlendMode::Normal,
            "add" => BlendMode::Add,
            "multiply" => BlendMode::Multiply,
            "screen" => BlendMode::Screen,
            "overlay" => BlendMode::Overlay,
            _ => {
                return ServerMessage::Error {
                    message: format!("Unknown blend mode: {}", blend_mode),
                }
            }
        };

        let mut comp = self.composition.write().await;
        if let Some(layer) = comp.get_layer_by_index_mut(layer_index) {
            layer.blend_mode = mode;
            ServerMessage::LayerUpdate(LayerStateUpdate::from_layer(layer))
        } else {
            ServerMessage::Error {
                message: format!("Layer {} not found", layer_index),
            }
        }
    }

    /// Set master opacity
    async fn set_master_opacity(&self, opacity: f32) -> ServerMessage {
        let mut comp = self.composition.write().await;
        comp.master_opacity = opacity.clamp(0.0, 1.0);
        self.get_full_state_internal(&comp)
    }

    /// Get full composition state
    async fn get_full_state(&self) -> ServerMessage {
        let comp = self.composition.read().await;
        self.get_full_state_internal(&comp)
    }

    /// Internal helper to create state update
    fn get_full_state_internal(&self, comp: &Composition) -> ServerMessage {
        ServerMessage::StateUpdate(StateUpdate::from_composition(comp))
    }
}


