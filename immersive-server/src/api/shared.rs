//! Shared state between API server and main application
//!
//! This module provides thread-safe access to application state from API handlers.
//! The state is a snapshot that gets updated by the main thread each frame.

use std::sync::{Arc, RwLock};
use tokio::sync::{broadcast, mpsc};

use super::types::*;
use crate::compositor::{BlendMode, ClipTransition};

/// Commands that can be sent from API handlers to the main application
#[derive(Debug, Clone)]
pub enum ApiCommand {
    // Environment commands
    SetEnvironmentSize { width: u32, height: u32 },
    SetTargetFps { fps: u32 },

    // Layer commands
    CreateLayer { name: String },
    DeleteLayer { id: u32 },
    UpdateLayer { id: u32, name: Option<String>, visible: Option<bool>, opacity: Option<f32>, blend_mode: Option<BlendMode> },
    ReorderLayer { id: u32, position: usize },
    CloneLayer { id: u32 },

    // Layer transform commands
    SetLayerPosition { id: u32, x: f32, y: f32 },
    SetLayerScale { id: u32, scale_x: f32, scale_y: f32 },
    SetLayerRotation { id: u32, rotation: f32 },
    SetLayerTransform { id: u32, position: Option<(f32, f32)>, scale: Option<(f32, f32)>, rotation: Option<f32>, anchor: Option<(f32, f32)> },

    // Layer property commands
    SetLayerOpacity { id: u32, opacity: f32 },
    SetLayerBlendMode { id: u32, blend_mode: BlendMode },
    SetLayerVisibility { id: u32, visible: bool },
    SetLayerTransition { id: u32, transition: ClipTransition },

    // Clip commands
    SetClip { layer_id: u32, slot: usize, source_type: String, path: Option<String>, source_id: Option<String>, label: Option<String> },
    ClearClip { layer_id: u32, slot: usize },
    TriggerClip { layer_id: u32, slot: usize },
    StopClip { layer_id: u32 },
    StopClipFade { layer_id: u32, duration_ms: u32 },
    CopyClip { layer_id: u32, slot: usize },
    PasteClip { layer_id: u32, slot: usize },

    // Grid management
    AddColumn,
    DeleteColumn { index: usize },

    // Playback commands
    PauseAll,
    ResumeAll,
    TogglePause,
    RestartAll,
    PauseLayer { id: u32 },
    ResumeLayer { id: u32 },
    RestartLayer { id: u32 },

    // Viewport commands
    ResetViewport,
    SetViewportZoom { zoom: f32 },
    SetViewportPan { x: f32, y: f32 },

    // Streaming commands
    StartOmtBroadcast { name: String, port: u16 },
    StopOmtBroadcast,
    SetOmtCaptureFps { fps: u32 },
    StartNdiBroadcast { name: String },
    StopNdiBroadcast,
    StartTextureShare,
    StopTextureShare,

    // Source discovery
    RefreshOmtSources,
    StartNdiDiscovery,
    StopNdiDiscovery,
    RefreshNdiSources,

    // File operations
    OpenFile { path: String },
    SaveFile,
    SaveFileAs { path: String },

    // Environment effects
    AddEnvironmentEffect { effect_type: String },
    RemoveEnvironmentEffect { effect_id: String },
    UpdateEnvironmentEffect { effect_id: String, parameters: serde_json::Value },
    BypassEnvironmentEffect { effect_id: String },
    SoloEnvironmentEffect { effect_id: String },
    ReorderEnvironmentEffects { order: Vec<String> },

    // Layer effects
    AddLayerEffect { layer_id: u32, effect_type: String },
    RemoveLayerEffect { layer_id: u32, effect_id: String },
    UpdateLayerEffect { layer_id: u32, effect_id: String, parameters: serde_json::Value },
    BypassLayerEffect { layer_id: u32, effect_id: String },
    SoloLayerEffect { layer_id: u32, effect_id: String },
    ReorderLayerEffects { layer_id: u32, order: Vec<String> },

    // Clip effects
    AddClipEffect { layer_id: u32, slot: usize, effect_type: String },
    RemoveClipEffect { layer_id: u32, slot: usize, effect_id: String },
    UpdateClipEffect { layer_id: u32, slot: usize, effect_id: String, parameters: serde_json::Value },
    BypassClipEffect { layer_id: u32, slot: usize, effect_id: String },
}

/// Snapshot of layer state for API reads
#[derive(Debug, Clone)]
pub struct LayerSnapshot {
    pub id: u32,
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub position: (f32, f32),
    pub scale: (f32, f32),
    pub rotation: f32,
    pub anchor: (f32, f32),
    pub transition: ClipTransition,
    pub clips: Vec<ClipSnapshot>,
    pub active_clip: Option<usize>,
    /// Effects applied to this layer
    pub effects: Vec<EffectSnapshot>,
}

/// Snapshot of clip state for API reads
#[derive(Debug, Clone)]
pub struct ClipSnapshot {
    pub slot: usize,
    pub source_type: Option<String>,
    pub source_path: Option<String>,
    pub label: Option<String>,
    /// Effects applied to this clip
    pub effects: Vec<EffectSnapshot>,
}

/// Snapshot of viewport state
#[derive(Debug, Clone)]
pub struct ViewportSnapshot {
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
}

/// Snapshot of streaming state
#[derive(Debug, Clone)]
pub struct StreamingSnapshot {
    pub omt_broadcasting: bool,
    pub omt_name: Option<String>,
    pub omt_port: Option<u16>,
    pub omt_capture_fps: u32,
    pub ndi_broadcasting: bool,
    pub ndi_name: Option<String>,
    pub texture_sharing: bool,
}

/// Snapshot of discovered source
#[derive(Debug, Clone)]
pub struct SourceSnapshot {
    pub id: String,
    pub source_type: String,
    pub name: String,
}

/// Snapshot of file state
#[derive(Debug, Clone)]
pub struct FileSnapshot {
    pub current_path: Option<String>,
    pub modified: bool,
    pub recent_files: Vec<String>,
}

/// Snapshot of effect instance
#[derive(Debug, Clone)]
pub struct EffectSnapshot {
    pub id: String,
    pub effect_type: String,
    pub enabled: bool,
    pub bypassed: bool,
    pub solo: bool,
}

/// Snapshot of output display
#[derive(Debug, Clone)]
pub struct OutputSnapshot {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
    pub refresh_rate_hz: Option<u32>,
}

/// Performance metrics snapshot
#[derive(Debug, Clone, Default)]
pub struct PerformanceSnapshot {
    /// Average frame time in milliseconds
    pub frame_time_avg_ms: f32,
    /// Minimum frame time in milliseconds
    pub frame_time_min_ms: f32,
    /// Maximum frame time in milliseconds
    pub frame_time_max_ms: f32,
    /// 95th percentile frame time in milliseconds
    pub frame_time_p95_ms: f32,
    /// 99th percentile frame time in milliseconds
    pub frame_time_p99_ms: f32,
    /// GPU timing breakdown by pass name
    pub gpu_timings: std::collections::HashMap<String, f32>,
    /// Total GPU time in milliseconds
    pub gpu_total_ms: f32,
    /// Estimated GPU memory usage in MB
    pub gpu_memory_mb: f32,
}

/// Effect parameter info for API
#[derive(Debug, Clone)]
pub struct EffectParamInfo {
    /// Parameter name
    pub name: String,
    /// Parameter type (float, color, bool, etc.)
    pub param_type: String,
    /// Default value as JSON
    pub default: serde_json::Value,
    /// Minimum value (for numeric types)
    pub min: Option<f32>,
    /// Maximum value (for numeric types)
    pub max: Option<f32>,
}

/// Effect type definition from registry
#[derive(Debug, Clone)]
pub struct EffectTypeInfo {
    /// Effect type identifier (e.g., "color_correction")
    pub effect_type: String,
    /// Display name (e.g., "Color Correction")
    pub display_name: String,
    /// Category (e.g., "Color")
    pub category: String,
    /// Parameter definitions
    pub parameters: Vec<EffectParamInfo>,
}

/// Snapshot of application state for API reads
#[derive(Debug, Clone)]
pub struct AppSnapshot {
    pub env_width: u32,
    pub env_height: u32,
    pub target_fps: u32,
    pub current_fps: f32,
    pub frame_time_ms: f32,
    pub paused: bool,
    pub layers: Vec<LayerSnapshot>,
    pub viewport: ViewportSnapshot,
    pub streaming: StreamingSnapshot,
    pub sources: Vec<SourceSnapshot>,
    pub file: FileSnapshot,
    pub environment_effects: Vec<EffectSnapshot>,
    pub clip_columns: usize,
    /// Available output displays
    pub outputs: Vec<OutputSnapshot>,
    /// Performance metrics
    pub performance: PerformanceSnapshot,
    /// Available effect types from registry
    pub effect_types: Vec<EffectTypeInfo>,
    /// Effect categories in display order
    pub effect_categories: Vec<String>,
}

impl Default for AppSnapshot {
    fn default() -> Self {
        Self {
            env_width: 1920,
            env_height: 1080,
            target_fps: 60,
            current_fps: 0.0,
            frame_time_ms: 0.0,
            paused: false,
            layers: Vec::new(),
            viewport: ViewportSnapshot {
                zoom: 1.0,
                pan_x: 0.0,
                pan_y: 0.0,
            },
            streaming: StreamingSnapshot {
                omt_broadcasting: false,
                omt_name: None,
                omt_port: None,
                omt_capture_fps: 30,
                ndi_broadcasting: false,
                ndi_name: None,
                texture_sharing: false,
            },
            sources: Vec::new(),
            file: FileSnapshot {
                current_path: None,
                modified: false,
                recent_files: Vec::new(),
            },
            environment_effects: Vec::new(),
            clip_columns: 8,
            outputs: Vec::new(),
            performance: PerformanceSnapshot::default(),
            effect_types: Vec::new(),
            effect_categories: Vec::new(),
        }
    }
}

/// WebSocket event types sent to connected clients
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "data")]
pub enum WsEvent {
    /// Full state snapshot (sent on connect and periodically)
    #[serde(rename = "snapshot")]
    Snapshot(WsSnapshot),
    /// FPS/performance update (sent frequently)
    #[serde(rename = "fps")]
    Fps { fps: f32, frame_time_ms: f32 },
    /// Layer state changed
    #[serde(rename = "layer_changed")]
    LayerChanged { layer_id: u32 },
    /// Clip triggered
    #[serde(rename = "clip_triggered")]
    ClipTriggered { layer_id: u32, slot: usize },
    /// Playback state changed
    #[serde(rename = "playback_changed")]
    PlaybackChanged { paused: bool },
    /// Streaming state changed
    #[serde(rename = "streaming_changed")]
    StreamingChanged { omt: bool, ndi: bool, texture: bool },
}

/// Lightweight snapshot for WebSocket updates
#[derive(Debug, Clone, serde::Serialize)]
pub struct WsSnapshot {
    pub env_width: u32,
    pub env_height: u32,
    pub fps: f32,
    pub paused: bool,
    pub layer_count: usize,
    pub omt_broadcasting: bool,
    pub ndi_broadcasting: bool,
    pub texture_sharing: bool,
}

impl From<&AppSnapshot> for WsSnapshot {
    fn from(snap: &AppSnapshot) -> Self {
        Self {
            env_width: snap.env_width,
            env_height: snap.env_height,
            fps: snap.current_fps,
            paused: snap.paused,
            layer_count: snap.layers.len(),
            omt_broadcasting: snap.streaming.omt_broadcasting,
            ndi_broadcasting: snap.streaming.ndi_broadcasting,
            texture_sharing: snap.streaming.texture_sharing,
        }
    }
}

/// Shared state accessible by API handlers
pub struct SharedState {
    /// Read-only snapshot of app state (updated by main thread each frame)
    pub snapshot: RwLock<AppSnapshot>,
    /// Channel to send commands to main application
    pub command_tx: mpsc::UnboundedSender<ApiCommand>,
    /// Broadcast channel for WebSocket events
    pub ws_tx: broadcast::Sender<WsEvent>,
}

impl SharedState {
    /// Create new shared state with a command channel sender
    pub fn new(command_tx: mpsc::UnboundedSender<ApiCommand>) -> Self {
        // Create broadcast channel with capacity for 64 events
        let (ws_tx, _) = broadcast::channel(64);
        Self {
            snapshot: RwLock::new(AppSnapshot::default()),
            command_tx,
            ws_tx,
        }
    }

    /// Get a clone of the current snapshot
    pub fn get_snapshot(&self) -> AppSnapshot {
        self.snapshot.read().unwrap().clone()
    }

    /// Update the snapshot (called by main thread)
    /// Uses try_write() to avoid blocking if API handlers hold read locks
    pub fn update_snapshot(&self, snapshot: AppSnapshot) {
        if let Ok(mut guard) = self.snapshot.try_write() {
            *guard = snapshot;
        }
        // If lock busy, skip update - next frame will catch up
    }

    /// Update snapshot and broadcast FPS to WebSocket clients
    /// Uses try_write() to avoid blocking if API handlers hold read locks
    pub fn update_snapshot_with_broadcast(&self, snapshot: AppSnapshot) {
        let fps_event = WsEvent::Fps {
            fps: snapshot.current_fps,
            frame_time_ms: snapshot.frame_time_ms,
        };
        // Ignore send errors (no subscribers is fine)
        let _ = self.ws_tx.send(fps_event);
        if let Ok(mut guard) = self.snapshot.try_write() {
            *guard = snapshot;
        }
    }

    /// Send a command to the main application
    pub fn send_command(&self, cmd: ApiCommand) -> Result<(), mpsc::error::SendError<ApiCommand>> {
        self.command_tx.send(cmd)
    }

    /// Subscribe to WebSocket events
    pub fn subscribe(&self) -> broadcast::Receiver<WsEvent> {
        self.ws_tx.subscribe()
    }

    /// Broadcast an event to all WebSocket clients
    pub fn broadcast(&self, event: WsEvent) {
        let _ = self.ws_tx.send(event);
    }
}

// ============================================================================
// Conversion helpers for API responses
// ============================================================================

impl LayerSnapshot {
    pub fn to_summary(&self) -> LayerSummary {
        LayerSummary {
            id: self.id,
            name: self.name.clone(),
            visible: self.visible,
            opacity: self.opacity,
            blend_mode: format!("{:?}", self.blend_mode),
            active_clip: self.active_clip,
            clips: self.clips.iter().map(|c| c.to_summary()).collect(),
        }
    }

    pub fn to_response(&self) -> LayerResponse {
        LayerResponse {
            id: self.id,
            name: self.name.clone(),
            visible: self.visible,
            opacity: self.opacity,
            blend_mode: format!("{:?}", self.blend_mode),
            transform: TransformResponse {
                position_x: self.position.0,
                position_y: self.position.1,
                scale_x: self.scale.0,
                scale_y: self.scale.1,
                rotation: self.rotation,
                anchor_x: self.anchor.0,
                anchor_y: self.anchor.1,
            },
            clip_count: self.clips.len(),
            active_clip: self.active_clip,
            transition: match &self.transition {
                ClipTransition::Cut => TransitionResponse {
                    transition_type: "Cut".to_string(),
                    duration_ms: None,
                },
                ClipTransition::Fade(duration_ms) => TransitionResponse {
                    transition_type: "Fade".to_string(),
                    duration_ms: Some(*duration_ms),
                },
            },
        }
    }
}

impl ClipSnapshot {
    pub fn to_summary(&self) -> ClipSummary {
        ClipSummary {
            slot: self.slot,
            source_type: self.source_type.clone(),
            source_path: self.source_path.clone(),
            label: self.label.clone(),
        }
    }
}

/// Type alias for the shared state handle used by API handlers
pub type SharedStateHandle = Arc<SharedState>;
