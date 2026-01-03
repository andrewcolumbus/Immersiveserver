//! API request/response types
//!
//! These types are used for JSON serialization in API endpoints.

use serde::{Deserialize, Serialize};

// ============================================================================
// Status Types
// ============================================================================

/// Server status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub uptime_seconds: u64,
}

/// FPS status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FpsResponse {
    pub fps: f32,
    pub frame_time_ms: f32,
    pub target_fps: u32,
}

/// Performance metrics response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceResponse {
    // Basic
    pub fps: f32,
    pub frame_time_ms: f32,
    pub target_fps: u32,

    // Frame timing percentiles
    pub frame_time_avg_ms: f32,
    pub frame_time_min_ms: f32,
    pub frame_time_max_ms: f32,
    pub frame_time_p95_ms: f32,
    pub frame_time_p99_ms: f32,

    // GPU timing breakdown
    pub gpu_timings: std::collections::HashMap<String, f32>,
    pub gpu_total_ms: f32,

    // Resource counts
    pub layer_count: usize,
    pub active_clips: usize,
    pub effect_count: usize,

    // Memory (MB)
    pub gpu_memory_mb: f32,
}

impl PerformanceResponse {
    /// Create from telemetry PerformanceMetrics
    pub fn from_metrics(metrics: &crate::telemetry::PerformanceMetrics) -> Self {
        Self {
            fps: metrics.fps as f32,
            frame_time_ms: if metrics.fps > 0.0 {
                1000.0 / metrics.fps as f32
            } else {
                0.0
            },
            target_fps: metrics.target_fps,
            frame_time_avg_ms: metrics.frame_stats.avg_ms as f32,
            frame_time_min_ms: metrics.frame_stats.min_ms as f32,
            frame_time_max_ms: metrics.frame_stats.max_ms as f32,
            frame_time_p95_ms: metrics.frame_stats.p95_ms as f32,
            frame_time_p99_ms: metrics.frame_stats.p99_ms as f32,
            gpu_timings: metrics
                .gpu_timings
                .iter()
                .map(|(k, v)| (k.clone(), *v as f32))
                .collect(),
            gpu_total_ms: metrics.gpu_total_ms as f32,
            layer_count: metrics.layer_count,
            active_clips: metrics.active_clip_count,
            effect_count: metrics.effect_count,
            gpu_memory_mb: metrics.gpu_memory.total_mb() as f32,
        }
    }
}

// ============================================================================
// Environment Types
// ============================================================================

/// Environment state response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentResponse {
    pub width: u32,
    pub height: u32,
    pub target_fps: u32,
    pub layer_count: usize,
}

/// Environment update request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentUpdateRequest {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub target_fps: Option<u32>,
}

// ============================================================================
// Layer Types
// ============================================================================

/// Layer summary for list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerSummary {
    pub id: u32,
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    pub blend_mode: String,
    pub active_clip: Option<usize>,
    pub clips: Vec<ClipSummary>,
}

/// Full layer details response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerResponse {
    pub id: u32,
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    pub blend_mode: String,
    pub transform: TransformResponse,
    pub tile_x: u32,
    pub tile_y: u32,
    pub clip_count: usize,
    pub active_clip: Option<usize>,
    pub transition: TransitionResponse,
}

/// Transform state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformResponse {
    pub position_x: f32,
    pub position_y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation: f32,
    pub anchor_x: f32,
    pub anchor_y: f32,
}

/// Transition settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionResponse {
    #[serde(rename = "type")]
    pub transition_type: String,
    pub duration_ms: Option<u32>,
}

/// Layer list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayersResponse {
    pub layers: Vec<LayerSummary>,
}

/// Create layer request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLayerRequest {
    pub name: Option<String>,
}

/// Update layer request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateLayerRequest {
    pub name: Option<String>,
    pub visible: Option<bool>,
    pub opacity: Option<f32>,
    pub blend_mode: Option<String>,
}

/// Update transform request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTransformRequest {
    pub position_x: Option<f32>,
    pub position_y: Option<f32>,
    pub scale_x: Option<f32>,
    pub scale_y: Option<f32>,
    pub rotation: Option<f32>,
    pub anchor_x: Option<f32>,
    pub anchor_y: Option<f32>,
}

/// Reorder layers request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReorderLayersRequest {
    pub layer_id: u32,
    pub position: usize,
}

// ============================================================================
// Clip Types
// ============================================================================

/// Clip summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipSummary {
    pub slot: usize,
    pub source_type: Option<String>,
    pub source_path: Option<String>,
    pub label: Option<String>,
}

/// Clips list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipsResponse {
    pub active_slot: Option<usize>,
    pub clips: Vec<ClipSummary>,
}

/// Set clip request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetClipRequest {
    pub source_type: String,
    pub path: Option<String>,
    pub source_id: Option<String>,
    pub label: Option<String>,
}

/// Trigger clip request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerClipRequest {
    pub transition: Option<String>,
    pub duration_ms: Option<u32>,
}

// ============================================================================
// Playback Types
// ============================================================================

/// Playback status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackResponse {
    pub paused: bool,
    pub playing_layers: Vec<u32>,
}

// ============================================================================
// Viewport Types
// ============================================================================

/// Viewport state response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportResponse {
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
}

/// Update viewport request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateViewportRequest {
    pub zoom: Option<f32>,
    pub pan_x: Option<f32>,
    pub pan_y: Option<f32>,
}

// ============================================================================
// Effects Types
// ============================================================================

/// Effect type summary (for registry)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectTypeSummary {
    #[serde(rename = "type")]
    pub effect_type: String,
    pub name: String,
    pub category: String,
}

/// Effects registry list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectTypesResponse {
    pub effects: Vec<EffectTypeSummary>,
}

/// Effect categories response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectCategoriesResponse {
    pub categories: Vec<String>,
}

/// Effect parameter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectParameterDef {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
    pub default: serde_json::Value,
    pub min: Option<f32>,
    pub max: Option<f32>,
}

/// Effect definition response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectDefinitionResponse {
    #[serde(rename = "type")]
    pub effect_type: String,
    pub name: String,
    pub category: String,
    pub parameters: Vec<EffectParameterDef>,
}

/// Effect instance (applied to layer/clip/environment)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectInstance {
    pub id: String,
    #[serde(rename = "type")]
    pub effect_type: String,
    pub enabled: bool,
    pub bypassed: bool,
    pub solo: bool,
    pub parameters: serde_json::Value,
}

/// Effects list response (for layer/clip/environment)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectsResponse {
    pub effects: Vec<EffectInstance>,
}

/// Add effect request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddEffectRequest {
    #[serde(rename = "type")]
    pub effect_type: String,
}

/// Update effect request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEffectRequest {
    pub parameters: Option<serde_json::Value>,
    pub enabled: Option<bool>,
}

/// Reorder effects request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReorderEffectsRequest {
    pub order: Vec<String>,
}

// ============================================================================
// Source Types
// ============================================================================

/// Discovered source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSummary {
    pub id: String,
    pub source_type: String,
    pub name: String,
}

/// Sources list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcesResponse {
    pub sources: Vec<SourceSummary>,
}

// ============================================================================
// Streaming Types
// ============================================================================

/// OMT broadcast status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmtStatusResponse {
    pub broadcasting: bool,
    pub name: Option<String>,
    pub port: Option<u16>,
    pub capture_fps: u32,
}

/// NDI broadcast status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NdiStatusResponse {
    pub broadcasting: bool,
    pub name: Option<String>,
}

/// Start NDI request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartNdiRequest {
    pub name: String,
}

/// Texture sharing status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureShareResponse {
    pub sharing: bool,
    pub name: Option<String>,
}

// ============================================================================
// Output Types
// ============================================================================

/// Output display summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputSummary {
    pub id: usize,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

/// Outputs list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputsResponse {
    pub outputs: Vec<OutputSummary>,
}

/// Update output request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateOutputRequest {
    pub enabled: Option<bool>,
}

// ============================================================================
// File Types
// ============================================================================

/// Current file response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentFileResponse {
    pub path: Option<String>,
    pub modified: bool,
}

/// Open file request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenFileRequest {
    pub path: String,
}

/// Save as request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveAsRequest {
    pub path: String,
}

/// Recent files response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentFilesResponse {
    pub files: Vec<String>,
}

// ============================================================================
// Connection Status Types
// ============================================================================

/// Connections status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionsResponse {
    pub omt_sources: usize,
    pub ndi_sources: usize,
    pub omt_broadcasting: bool,
    pub ndi_broadcasting: bool,
}

// ============================================================================
// Transition Types
// ============================================================================

/// Set transition request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetTransitionRequest {
    #[serde(rename = "type")]
    pub transition_type: String,
    pub duration_ms: Option<u32>,
}

/// Stop with fade request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopFadeRequest {
    pub duration_ms: Option<u32>,
}

// ============================================================================
// Error Types
// ============================================================================

/// API error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
    pub code: u16,
}

impl ApiError {
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            error: "Not Found".to_string(),
            message: message.into(),
            code: 404,
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            error: "Bad Request".to_string(),
            message: message.into(),
            code: 400,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            error: "Internal Server Error".to_string(),
            message: message.into(),
            code: 500,
        }
    }
}
