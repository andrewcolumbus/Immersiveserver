//! WebSocket protocol definitions
//!
//! JSON message types for client-server communication.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Messages sent from client to server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Trigger a clip at layer/column
    TriggerClip { layer: usize, column: usize },
    /// Stop a layer
    StopLayer { layer: usize },
    /// Set layer opacity
    SetLayerOpacity { layer: usize, opacity: f32 },
    /// Set layer blend mode
    SetLayerBlendMode { layer: usize, blend_mode: String },
    /// Set master opacity
    SetMasterOpacity { opacity: f32 },
    /// Request full state sync
    RequestState,
    /// Ping for keepalive
    Ping,
}

/// Messages sent from server to client
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Full state update
    StateUpdate(StateUpdate),
    /// Layer state changed
    LayerUpdate(LayerStateUpdate),
    /// Clip triggered
    ClipTriggered { layer: usize, column: usize },
    /// Layer stopped
    LayerStopped { layer: usize },
    /// Pong response
    Pong,
    /// Error message
    Error { message: String },
}

/// Full composition state for sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateUpdate {
    /// Composition settings
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    /// Master settings
    pub master_opacity: f32,
    pub master_speed: f32,
    /// Layer states
    pub layers: Vec<LayerStateUpdate>,
}

/// Layer state for updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerStateUpdate {
    /// Layer ID
    pub id: u32,
    /// Layer name
    pub name: String,
    /// Active column (if any)
    pub active_column: Option<usize>,
    /// Layer opacity
    pub opacity: f32,
    /// Blend mode name
    pub blend_mode: String,
    /// Bypass state
    pub bypass: bool,
    /// Solo state
    pub solo: bool,
    /// Clips in this layer (name, has_content)
    pub clips: Vec<ClipInfo>,
}

/// Minimal clip info for state sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipInfo {
    /// Clip name (empty if no clip)
    pub name: Option<String>,
    /// Whether clip is currently playing
    pub playing: bool,
    /// Playback progress (0.0-1.0)
    pub progress: f32,
}

impl StateUpdate {
    /// Create a state update from a composition
    pub fn from_composition(comp: &crate::composition::Composition) -> Self {
        Self {
            width: comp.settings.width,
            height: comp.settings.height,
            fps: comp.settings.fps,
            master_opacity: comp.master_opacity,
            master_speed: comp.master_speed,
            layers: comp
                .layers
                .iter()
                .map(LayerStateUpdate::from_layer)
                .collect(),
        }
    }
}

impl LayerStateUpdate {
    /// Create a layer update from a layer
    pub fn from_layer(layer: &crate::composition::Layer) -> Self {
        Self {
            id: layer.id,
            name: layer.name.clone(),
            active_column: layer.active_column,
            opacity: layer.opacity,
            blend_mode: layer.blend_mode.name().to_string(),
            bypass: layer.bypass,
            solo: layer.solo,
            clips: layer
                .clips
                .iter()
                .enumerate()
                .map(|(i, slot)| {
                    if let Some(clip_slot) = slot {
                        ClipInfo {
                            name: Some(clip_slot.name()),
                            playing: layer.active_column == Some(i)
                                && clip_slot.playback.is_playing(),
                            progress: clip_slot.playback.progress() as f32,
                        }
                    } else {
                        ClipInfo {
                            name: None,
                            playing: false,
                            progress: 0.0,
                        }
                    }
                })
                .collect(),
        }
    }
}


