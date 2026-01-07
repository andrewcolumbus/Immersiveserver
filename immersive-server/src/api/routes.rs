//! API route definitions

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Html,
    routing::{delete, get, post, put},
    Json, Router,
};

use super::shared::{ApiCommand, SharedStateHandle};
use super::types::*;
use crate::compositor::BlendMode;

/// Embedded dashboard HTML
const DASHBOARD_HTML: &str = include_str!("dashboard.html");

/// Create the API router with all endpoints
pub fn create_router(state: SharedStateHandle) -> Router {
    Router::new()
        // Dashboard at root
        .route("/", get(|| async { Html(DASHBOARD_HTML) }))
        // Status endpoints
        .route("/api/status", get(status_handler))
        .route("/api/status/fps", get(fps_handler))
        .route("/api/status/performance", get(performance_handler))
        .route("/api/status/connections", get(connections_handler))
        // Environment endpoints
        .route("/api/environment", get(get_environment))
        .route("/api/environment", put(update_environment))
        // Environment effects
        .route("/api/environment/effects", get(list_environment_effects))
        .route("/api/environment/effects", post(add_environment_effect))
        .route("/api/environment/effects/:id", put(update_environment_effect))
        .route("/api/environment/effects/:id", delete(remove_environment_effect))
        .route("/api/environment/effects/:id/bypass", post(bypass_environment_effect))
        .route("/api/environment/effects/:id/solo", post(solo_environment_effect))
        .route("/api/environment/effects/reorder", post(reorder_environment_effects))
        // Layer endpoints
        .route("/api/layers", get(list_layers))
        .route("/api/layers", post(create_layer))
        .route("/api/layers/:id", get(get_layer))
        .route("/api/layers/:id", put(update_layer))
        .route("/api/layers/:id", delete(delete_layer))
        .route("/api/layers/:id/clone", post(clone_layer))
        .route("/api/layers/reorder", post(reorder_layers))
        // Layer transform endpoints
        .route("/api/layers/:id/transform", put(update_layer_transform))
        .route("/api/layers/:id/position", put(update_layer_position))
        .route("/api/layers/:id/scale", put(update_layer_scale))
        .route("/api/layers/:id/rotation", put(update_layer_rotation))
        // Layer property endpoints
        .route("/api/layers/:id/opacity", put(update_layer_opacity))
        .route("/api/layers/:id/blend", put(update_layer_blend))
        .route("/api/layers/:id/visibility", put(update_layer_visibility))
        .route("/api/layers/:id/transition", put(update_layer_transition))
        // Layer effects
        .route("/api/layers/:id/effects", get(list_layer_effects))
        .route("/api/layers/:id/effects", post(add_layer_effect))
        .route("/api/layers/:id/effects/:eid", put(update_layer_effect))
        .route("/api/layers/:id/effects/:eid", delete(remove_layer_effect))
        .route("/api/layers/:id/effects/:eid/bypass", post(bypass_layer_effect))
        .route("/api/layers/:id/effects/:eid/solo", post(solo_layer_effect))
        .route("/api/layers/:id/effects/reorder", post(reorder_layer_effects))
        // Clip endpoints
        .route("/api/layers/:id/clips", get(list_clips))
        .route("/api/layers/:id/clips/:slot", get(get_clip))
        .route("/api/layers/:id/clips/:slot", put(set_clip))
        .route("/api/layers/:id/clips/:slot", delete(clear_clip))
        .route("/api/layers/:id/clips/:slot/trigger", post(trigger_clip))
        .route("/api/layers/:id/clips/:slot/copy", post(copy_clip))
        .route("/api/layers/:id/clips/:slot/paste", post(paste_clip))
        .route("/api/layers/:id/clips/stop", post(stop_clip))
        .route("/api/layers/:id/clips/stop-fade", post(stop_clip_fade))
        // Clip effects
        .route("/api/layers/:id/clips/:slot/effects", get(list_clip_effects))
        .route("/api/layers/:id/clips/:slot/effects", post(add_clip_effect))
        .route("/api/layers/:id/clips/:slot/effects/:eid", put(update_clip_effect))
        .route("/api/layers/:id/clips/:slot/effects/:eid", delete(remove_clip_effect))
        .route("/api/layers/:id/clips/:slot/effects/:eid/bypass", post(bypass_clip_effect))
        // Grid management
        .route("/api/layers/columns", post(add_column))
        .route("/api/layers/columns/:index", delete(delete_column))
        // Playback endpoints
        .route("/api/playback/pause", post(pause_all))
        .route("/api/playback/resume", post(resume_all))
        .route("/api/playback/toggle", post(toggle_pause))
        .route("/api/playback/restart", post(restart_all))
        .route("/api/playback/status", get(playback_status))
        // Per-layer playback
        .route("/api/layers/:id/playback/pause", post(pause_layer))
        .route("/api/layers/:id/playback/resume", post(resume_layer))
        .route("/api/layers/:id/playback/restart", post(restart_layer))
        // Effects registry
        .route("/api/effects", get(list_effect_types))
        .route("/api/effects/categories", get(list_effect_categories))
        .route("/api/effects/:type", get(get_effect_definition))
        // Source discovery
        .route("/api/sources", get(list_sources))
        .route("/api/sources/omt", get(list_omt_sources))
        .route("/api/sources/omt/refresh", post(refresh_omt_sources))
        .route("/api/sources/ndi", get(list_ndi_sources))
        .route("/api/sources/ndi/start", post(start_ndi_discovery))
        .route("/api/sources/ndi/stop", post(stop_ndi_discovery))
        .route("/api/sources/ndi/refresh", post(refresh_ndi_sources))
        // Viewport endpoints
        .route("/api/viewport", get(get_viewport))
        .route("/api/viewport/reset", post(reset_viewport))
        .route("/api/viewport/zoom", put(set_viewport_zoom))
        .route("/api/viewport/pan", put(set_viewport_pan))
        // Streaming - OMT
        .route("/api/streaming/omt", get(get_omt_status))
        .route("/api/streaming/omt/start", post(start_omt_broadcast))
        .route("/api/streaming/omt/stop", post(stop_omt_broadcast))
        .route("/api/streaming/omt/fps", put(set_omt_fps))
        // Streaming - NDI
        .route("/api/streaming/ndi", get(get_ndi_status))
        .route("/api/streaming/ndi/start", post(start_ndi_broadcast))
        .route("/api/streaming/ndi/stop", post(stop_ndi_broadcast))
        // Streaming - Texture sharing
        .route("/api/streaming/texture", get(get_texture_status))
        .route("/api/streaming/texture/start", post(start_texture_share))
        .route("/api/streaming/texture/stop", post(stop_texture_share))
        // Output displays
        .route("/api/outputs", get(list_outputs))
        .route("/api/outputs/:id", get(get_output))
        .route("/api/outputs/:id", put(update_output))
        // File operations
        .route("/api/files/current", get(get_current_file))
        .route("/api/files/open", post(open_file))
        .route("/api/files/save", post(save_file))
        .route("/api/files/save-as", post(save_file_as))
        .route("/api/files/recent", get(list_recent_files))
        // WebSocket endpoint for real-time updates
        .route("/ws", get(super::websocket::ws_handler))
        // Add state to all routes
        .with_state(state)
}

// ============================================================================
// Status Handlers
// ============================================================================

async fn status_handler(State(_state): State<SharedStateHandle>) -> Json<StatusResponse> {
    Json(StatusResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: 0,
    })
}

async fn fps_handler(State(state): State<SharedStateHandle>) -> Json<FpsResponse> {
    let snapshot = state.get_snapshot();
    Json(FpsResponse {
        fps: snapshot.current_fps,
        frame_time_ms: snapshot.frame_time_ms,
        target_fps: snapshot.target_fps,
    })
}

async fn performance_handler(State(state): State<SharedStateHandle>) -> Json<PerformanceResponse> {
    let snapshot = state.get_snapshot();
    let active_clips = snapshot.layers.iter().filter(|l| l.active_clip.is_some()).count();
    // Count all effects: environment + layer + clip
    let effect_count = snapshot.environment_effects.len()
        + snapshot.layers.iter().map(|l| {
            l.effects.len() + l.clips.iter().map(|c| c.effects.len()).sum::<usize>()
        }).sum::<usize>();

    Json(PerformanceResponse {
        fps: snapshot.current_fps,
        frame_time_ms: snapshot.frame_time_ms,
        target_fps: snapshot.target_fps,
        frame_time_avg_ms: snapshot.performance.frame_time_avg_ms,
        frame_time_min_ms: snapshot.performance.frame_time_min_ms,
        frame_time_max_ms: snapshot.performance.frame_time_max_ms,
        frame_time_p95_ms: snapshot.performance.frame_time_p95_ms,
        frame_time_p99_ms: snapshot.performance.frame_time_p99_ms,
        gpu_timings: snapshot.performance.gpu_timings.clone(),
        gpu_total_ms: snapshot.performance.gpu_total_ms,
        layer_count: snapshot.layers.len(),
        active_clips,
        effect_count,
        gpu_memory_mb: snapshot.performance.gpu_memory_mb,
    })
}

async fn connections_handler(State(state): State<SharedStateHandle>) -> Json<ConnectionsResponse> {
    let snapshot = state.get_snapshot();
    let omt_sources = snapshot.sources.iter().filter(|s| s.source_type == "omt").count();
    let ndi_sources = snapshot.sources.iter().filter(|s| s.source_type == "ndi").count();
    Json(ConnectionsResponse {
        omt_sources,
        ndi_sources,
        omt_broadcasting: snapshot.streaming.omt_broadcasting,
        ndi_broadcasting: snapshot.streaming.ndi_broadcasting,
    })
}

// ============================================================================
// Environment Handlers
// ============================================================================

async fn get_environment(State(state): State<SharedStateHandle>) -> Json<EnvironmentResponse> {
    let snapshot = state.get_snapshot();
    Json(EnvironmentResponse {
        width: snapshot.env_width,
        height: snapshot.env_height,
        target_fps: snapshot.target_fps,
        layer_count: snapshot.layers.len(),
    })
}

async fn update_environment(
    State(state): State<SharedStateHandle>,
    Json(req): Json<EnvironmentUpdateRequest>,
) -> Result<Json<EnvironmentResponse>, (StatusCode, Json<ApiError>)> {
    if let (Some(width), Some(height)) = (req.width, req.height) {
        let _ = state.send_command(ApiCommand::SetEnvironmentSize { width, height });
    }
    if let Some(fps) = req.target_fps {
        let _ = state.send_command(ApiCommand::SetTargetFps { fps });
    }

    let snapshot = state.get_snapshot();
    Ok(Json(EnvironmentResponse {
        width: req.width.unwrap_or(snapshot.env_width),
        height: req.height.unwrap_or(snapshot.env_height),
        target_fps: req.target_fps.unwrap_or(snapshot.target_fps),
        layer_count: snapshot.layers.len(),
    }))
}

// ============================================================================
// Environment Effects Handlers
// ============================================================================

async fn list_environment_effects(State(state): State<SharedStateHandle>) -> Json<EffectsResponse> {
    let snapshot = state.get_snapshot();
    Json(EffectsResponse {
        effects: snapshot.environment_effects.iter().map(|e| EffectInstance {
            id: e.id.clone(),
            effect_type: e.effect_type.clone(),
            enabled: e.enabled,
            bypassed: e.bypassed,
            solo: e.solo,
            parameters: serde_json::json!({}),
        }).collect(),
    })
}

async fn add_environment_effect(
    State(state): State<SharedStateHandle>,
    Json(req): Json<AddEffectRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let _ = state.send_command(ApiCommand::AddEnvironmentEffect { effect_type: req.effect_type });
    (StatusCode::CREATED, Json(serde_json::json!({ "message": "Effect add requested" })))
}

async fn update_environment_effect(
    State(state): State<SharedStateHandle>,
    Path(id): Path<String>,
    Json(req): Json<UpdateEffectRequest>,
) -> Json<serde_json::Value> {
    if let Some(params) = req.parameters {
        let _ = state.send_command(ApiCommand::UpdateEnvironmentEffect { effect_id: id, parameters: params });
    }
    Json(serde_json::json!({ "message": "Effect update requested" }))
}

async fn remove_environment_effect(
    State(state): State<SharedStateHandle>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::RemoveEnvironmentEffect { effect_id: id });
    Json(serde_json::json!({ "message": "Effect remove requested" }))
}

async fn bypass_environment_effect(
    State(state): State<SharedStateHandle>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::BypassEnvironmentEffect { effect_id: id });
    Json(serde_json::json!({ "message": "Effect bypass toggled" }))
}

async fn solo_environment_effect(
    State(state): State<SharedStateHandle>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::SoloEnvironmentEffect { effect_id: id });
    Json(serde_json::json!({ "message": "Effect solo toggled" }))
}

async fn reorder_environment_effects(
    State(state): State<SharedStateHandle>,
    Json(req): Json<ReorderEffectsRequest>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::ReorderEnvironmentEffects { order: req.order });
    Json(serde_json::json!({ "message": "Effects reorder requested" }))
}

// ============================================================================
// Layer Handlers
// ============================================================================

async fn list_layers(State(state): State<SharedStateHandle>) -> Json<LayersResponse> {
    let snapshot = state.get_snapshot();
    Json(LayersResponse {
        layers: snapshot.layers.iter().map(|l| l.to_summary()).collect(),
    })
}

async fn create_layer(
    State(state): State<SharedStateHandle>,
    Json(req): Json<CreateLayerRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let name = req.name.unwrap_or_else(|| "New Layer".to_string());
    let _ = state.send_command(ApiCommand::CreateLayer { name: name.clone() });
    (StatusCode::CREATED, Json(serde_json::json!({ "message": "Layer creation requested", "name": name })))
}

async fn get_layer(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
) -> Result<Json<LayerResponse>, (StatusCode, Json<ApiError>)> {
    let snapshot = state.get_snapshot();
    snapshot.layers.iter().find(|l| l.id == id)
        .map(|l| Json(l.to_response()))
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ApiError::not_found(format!("Layer {} not found", id)))))
}

async fn update_layer(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
    Json(req): Json<UpdateLayerRequest>,
) -> Json<serde_json::Value> {
    let blend_mode = req.blend_mode.as_ref().and_then(|s| parse_blend_mode(s));
    let _ = state.send_command(ApiCommand::UpdateLayer {
        id, name: req.name, visible: req.visible, opacity: req.opacity, blend_mode,
    });
    Json(serde_json::json!({ "message": "Layer update requested", "id": id }))
}

async fn delete_layer(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::DeleteLayer { id });
    Json(serde_json::json!({ "message": "Layer deletion requested", "id": id }))
}

async fn clone_layer(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
) -> (StatusCode, Json<serde_json::Value>) {
    let _ = state.send_command(ApiCommand::CloneLayer { id });
    (StatusCode::CREATED, Json(serde_json::json!({ "message": "Layer clone requested", "id": id })))
}

async fn reorder_layers(
    State(state): State<SharedStateHandle>,
    Json(req): Json<ReorderLayersRequest>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::ReorderLayer { id: req.layer_id, position: req.position });
    Json(serde_json::json!({ "message": "Layer reorder requested" }))
}

// ============================================================================
// Layer Transform Handlers
// ============================================================================

async fn update_layer_transform(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
    Json(req): Json<UpdateTransformRequest>,
) -> Json<serde_json::Value> {
    let position = req.position_x.zip(req.position_y);
    let scale = req.scale_x.zip(req.scale_y);
    let anchor = req.anchor_x.zip(req.anchor_y);
    let _ = state.send_command(ApiCommand::SetLayerTransform { id, position, scale, rotation: req.rotation, anchor });
    Json(serde_json::json!({ "message": "Transform update requested" }))
}

#[derive(serde::Deserialize)]
struct PositionRequest { x: f32, y: f32 }

async fn update_layer_position(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
    Json(req): Json<PositionRequest>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::SetLayerPosition { id, x: req.x, y: req.y });
    Json(serde_json::json!({ "message": "Position update requested" }))
}

#[derive(serde::Deserialize)]
struct ScaleRequest { scale_x: f32, scale_y: f32 }

async fn update_layer_scale(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
    Json(req): Json<ScaleRequest>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::SetLayerScale { id, scale_x: req.scale_x, scale_y: req.scale_y });
    Json(serde_json::json!({ "message": "Scale update requested" }))
}

#[derive(serde::Deserialize)]
struct RotationRequest { rotation: f32 }

async fn update_layer_rotation(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
    Json(req): Json<RotationRequest>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::SetLayerRotation { id, rotation: req.rotation });
    Json(serde_json::json!({ "message": "Rotation update requested" }))
}

// ============================================================================
// Layer Property Handlers
// ============================================================================

#[derive(serde::Deserialize)]
struct OpacityRequest { opacity: f32 }

async fn update_layer_opacity(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
    Json(req): Json<OpacityRequest>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::SetLayerOpacity { id, opacity: req.opacity });
    Json(serde_json::json!({ "message": "Opacity update requested" }))
}

#[derive(serde::Deserialize)]
struct BlendModeRequest { blend_mode: String }

async fn update_layer_blend(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
    Json(req): Json<BlendModeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    let blend_mode = parse_blend_mode(&req.blend_mode).ok_or_else(|| {
        (StatusCode::BAD_REQUEST, Json(ApiError::bad_request(format!("Invalid blend mode: {}", req.blend_mode))))
    })?;
    let _ = state.send_command(ApiCommand::SetLayerBlendMode { id, blend_mode });
    Ok(Json(serde_json::json!({ "message": "Blend mode update requested" })))
}

#[derive(serde::Deserialize)]
struct VisibilityRequest { visible: bool }

async fn update_layer_visibility(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
    Json(req): Json<VisibilityRequest>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::SetLayerVisibility { id, visible: req.visible });
    Json(serde_json::json!({ "message": "Visibility update requested" }))
}

async fn update_layer_transition(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
    Json(req): Json<SetTransitionRequest>,
) -> Json<serde_json::Value> {
    use crate::compositor::ClipTransition;
    let transition = match req.transition_type.to_lowercase().as_str() {
        "cut" => ClipTransition::Cut,
        "fade" => ClipTransition::Fade(req.duration_ms.unwrap_or(500)),
        _ => ClipTransition::Cut,
    };
    let _ = state.send_command(ApiCommand::SetLayerTransition { id, transition });
    Json(serde_json::json!({ "message": "Transition update requested" }))
}

// ============================================================================
// Layer Effects Handlers
// ============================================================================

async fn list_layer_effects(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
) -> Json<EffectsResponse> {
    let snapshot = state.get_snapshot();
    let effects = snapshot.layers.iter()
        .find(|l| l.id == id)
        .map(|layer| {
            layer.effects.iter().map(|e| EffectInstance {
                id: e.id.clone(),
                effect_type: e.effect_type.clone(),
                enabled: e.enabled,
                bypassed: e.bypassed,
                solo: e.solo,
                parameters: serde_json::json!({}),
            }).collect()
        })
        .unwrap_or_default();
    Json(EffectsResponse { effects })
}

async fn add_layer_effect(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
    Json(req): Json<AddEffectRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let _ = state.send_command(ApiCommand::AddLayerEffect { layer_id: id, effect_type: req.effect_type });
    (StatusCode::CREATED, Json(serde_json::json!({ "message": "Layer effect add requested" })))
}

async fn update_layer_effect(
    State(state): State<SharedStateHandle>,
    Path((id, eid)): Path<(u32, String)>,
    Json(req): Json<UpdateEffectRequest>,
) -> Json<serde_json::Value> {
    if let Some(params) = req.parameters {
        let _ = state.send_command(ApiCommand::UpdateLayerEffect { layer_id: id, effect_id: eid, parameters: params });
    }
    Json(serde_json::json!({ "message": "Layer effect update requested" }))
}

async fn remove_layer_effect(
    State(state): State<SharedStateHandle>,
    Path((id, eid)): Path<(u32, String)>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::RemoveLayerEffect { layer_id: id, effect_id: eid });
    Json(serde_json::json!({ "message": "Layer effect remove requested" }))
}

async fn bypass_layer_effect(
    State(state): State<SharedStateHandle>,
    Path((id, eid)): Path<(u32, String)>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::BypassLayerEffect { layer_id: id, effect_id: eid });
    Json(serde_json::json!({ "message": "Layer effect bypass toggled" }))
}

async fn solo_layer_effect(
    State(state): State<SharedStateHandle>,
    Path((id, eid)): Path<(u32, String)>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::SoloLayerEffect { layer_id: id, effect_id: eid });
    Json(serde_json::json!({ "message": "Layer effect solo toggled" }))
}

async fn reorder_layer_effects(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
    Json(req): Json<ReorderEffectsRequest>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::ReorderLayerEffects { layer_id: id, order: req.order });
    Json(serde_json::json!({ "message": "Layer effects reorder requested" }))
}

// ============================================================================
// Clip Handlers
// ============================================================================

async fn list_clips(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
) -> Result<Json<ClipsResponse>, (StatusCode, Json<ApiError>)> {
    let snapshot = state.get_snapshot();
    let layer = snapshot.layers.iter().find(|l| l.id == id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ApiError::not_found(format!("Layer {} not found", id)))))?;
    Ok(Json(ClipsResponse {
        active_slot: layer.active_clip,
        clips: layer.clips.iter().map(|c| c.to_summary()).collect(),
    }))
}

async fn get_clip(
    State(state): State<SharedStateHandle>,
    Path((id, slot)): Path<(u32, usize)>,
) -> Result<Json<ClipSummary>, (StatusCode, Json<ApiError>)> {
    let snapshot = state.get_snapshot();
    let layer = snapshot.layers.iter().find(|l| l.id == id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ApiError::not_found(format!("Layer {} not found", id)))))?;
    layer.clips.iter().find(|c| c.slot == slot)
        .map(|c| Json(c.to_summary()))
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ApiError::not_found(format!("Clip slot {} not found", slot)))))
}

async fn set_clip(
    State(state): State<SharedStateHandle>,
    Path((id, slot)): Path<(u32, usize)>,
    Json(req): Json<SetClipRequest>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::SetClip {
        layer_id: id, slot, source_type: req.source_type, path: req.path, source_id: req.source_id, label: req.label,
    });
    Json(serde_json::json!({ "message": "Clip set requested" }))
}

async fn clear_clip(
    State(state): State<SharedStateHandle>,
    Path((id, slot)): Path<(u32, usize)>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::ClearClip { layer_id: id, slot });
    Json(serde_json::json!({ "message": "Clip clear requested" }))
}

async fn trigger_clip(
    State(state): State<SharedStateHandle>,
    Path((id, slot)): Path<(u32, usize)>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::TriggerClip { layer_id: id, slot });
    Json(serde_json::json!({ "message": "Clip trigger requested" }))
}

async fn copy_clip(
    State(state): State<SharedStateHandle>,
    Path((id, slot)): Path<(u32, usize)>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::CopyClip { layer_id: id, slot });
    Json(serde_json::json!({ "message": "Clip copy requested" }))
}

async fn paste_clip(
    State(state): State<SharedStateHandle>,
    Path((id, slot)): Path<(u32, usize)>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::PasteClip { layer_id: id, slot });
    Json(serde_json::json!({ "message": "Clip paste requested" }))
}

async fn stop_clip(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::StopClip { layer_id: id });
    Json(serde_json::json!({ "message": "Clip stop requested" }))
}

async fn stop_clip_fade(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
    Json(req): Json<StopFadeRequest>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::StopClipFade { layer_id: id, duration_ms: req.duration_ms.unwrap_or(500) });
    Json(serde_json::json!({ "message": "Clip stop-fade requested" }))
}

// ============================================================================
// Clip Effects Handlers
// ============================================================================

async fn list_clip_effects(
    State(state): State<SharedStateHandle>,
    Path((id, slot)): Path<(u32, usize)>,
) -> Json<EffectsResponse> {
    let snapshot = state.get_snapshot();
    let effects = snapshot.layers.iter()
        .find(|l| l.id == id)
        .and_then(|layer| layer.clips.get(slot))
        .map(|clip| {
            clip.effects.iter().map(|e| EffectInstance {
                id: e.id.clone(),
                effect_type: e.effect_type.clone(),
                enabled: e.enabled,
                bypassed: e.bypassed,
                solo: e.solo,
                parameters: serde_json::json!({}),
            }).collect()
        })
        .unwrap_or_default();
    Json(EffectsResponse { effects })
}

async fn add_clip_effect(
    State(state): State<SharedStateHandle>,
    Path((id, slot)): Path<(u32, usize)>,
    Json(req): Json<AddEffectRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let _ = state.send_command(ApiCommand::AddClipEffect { layer_id: id, slot, effect_type: req.effect_type });
    (StatusCode::CREATED, Json(serde_json::json!({ "message": "Clip effect add requested" })))
}

async fn update_clip_effect(
    State(state): State<SharedStateHandle>,
    Path((id, slot, eid)): Path<(u32, usize, String)>,
    Json(req): Json<UpdateEffectRequest>,
) -> Json<serde_json::Value> {
    if let Some(params) = req.parameters {
        let _ = state.send_command(ApiCommand::UpdateClipEffect { layer_id: id, slot, effect_id: eid, parameters: params });
    }
    Json(serde_json::json!({ "message": "Clip effect update requested" }))
}

async fn remove_clip_effect(
    State(state): State<SharedStateHandle>,
    Path((id, slot, eid)): Path<(u32, usize, String)>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::RemoveClipEffect { layer_id: id, slot, effect_id: eid });
    Json(serde_json::json!({ "message": "Clip effect remove requested" }))
}

async fn bypass_clip_effect(
    State(state): State<SharedStateHandle>,
    Path((id, slot, eid)): Path<(u32, usize, String)>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::BypassClipEffect { layer_id: id, slot, effect_id: eid });
    Json(serde_json::json!({ "message": "Clip effect bypass toggled" }))
}

// ============================================================================
// Grid Management Handlers
// ============================================================================

async fn add_column(State(state): State<SharedStateHandle>) -> (StatusCode, Json<serde_json::Value>) {
    let _ = state.send_command(ApiCommand::AddColumn);
    (StatusCode::CREATED, Json(serde_json::json!({ "message": "Column add requested" })))
}

async fn delete_column(
    State(state): State<SharedStateHandle>,
    Path(index): Path<usize>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::DeleteColumn { index });
    Json(serde_json::json!({ "message": "Column delete requested" }))
}

// ============================================================================
// Playback Handlers
// ============================================================================

async fn pause_all(State(state): State<SharedStateHandle>) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::PauseAll);
    Json(serde_json::json!({ "message": "Pause requested" }))
}

async fn resume_all(State(state): State<SharedStateHandle>) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::ResumeAll);
    Json(serde_json::json!({ "message": "Resume requested" }))
}

async fn toggle_pause(State(state): State<SharedStateHandle>) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::TogglePause);
    Json(serde_json::json!({ "message": "Toggle pause requested" }))
}

async fn restart_all(State(state): State<SharedStateHandle>) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::RestartAll);
    Json(serde_json::json!({ "message": "Restart requested" }))
}

async fn playback_status(State(state): State<SharedStateHandle>) -> Json<PlaybackResponse> {
    let snapshot = state.get_snapshot();
    Json(PlaybackResponse {
        paused: snapshot.paused,
        playing_layers: snapshot.layers.iter().filter(|l| l.active_clip.is_some()).map(|l| l.id).collect(),
    })
}

async fn pause_layer(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::PauseLayer { id });
    Json(serde_json::json!({ "message": "Layer pause requested" }))
}

async fn resume_layer(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::ResumeLayer { id });
    Json(serde_json::json!({ "message": "Layer resume requested" }))
}

async fn restart_layer(
    State(state): State<SharedStateHandle>,
    Path(id): Path<u32>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::RestartLayer { id });
    Json(serde_json::json!({ "message": "Layer restart requested" }))
}

// ============================================================================
// Effects Registry Handlers
// ============================================================================

async fn list_effect_types(State(state): State<SharedStateHandle>) -> Json<EffectTypesResponse> {
    let snapshot = state.get_snapshot();
    Json(EffectTypesResponse {
        effects: snapshot.effect_types.iter().map(|e| EffectTypeSummary {
            effect_type: e.effect_type.clone(),
            name: e.display_name.clone(),
            category: e.category.clone(),
        }).collect(),
    })
}

async fn list_effect_categories(State(state): State<SharedStateHandle>) -> Json<EffectCategoriesResponse> {
    let snapshot = state.get_snapshot();
    Json(EffectCategoriesResponse {
        categories: snapshot.effect_categories.clone(),
    })
}

async fn get_effect_definition(
    State(state): State<SharedStateHandle>,
    Path(effect_type): Path<String>,
) -> Result<Json<EffectDefinitionResponse>, (StatusCode, Json<ApiError>)> {
    let snapshot = state.get_snapshot();
    snapshot.effect_types.iter()
        .find(|e| e.effect_type == effect_type)
        .map(|e| Json(EffectDefinitionResponse {
            effect_type: e.effect_type.clone(),
            name: e.display_name.clone(),
            category: e.category.clone(),
            parameters: e.parameters.iter().map(|p| EffectParameterDef {
                name: p.name.clone(),
                param_type: p.param_type.clone(),
                default: p.default.clone(),
                min: p.min,
                max: p.max,
            }).collect(),
        }))
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ApiError::not_found(format!("Effect type '{}' not found", effect_type)))))
}

// ============================================================================
// Source Discovery Handlers
// ============================================================================

async fn list_sources(State(state): State<SharedStateHandle>) -> Json<SourcesResponse> {
    let snapshot = state.get_snapshot();
    Json(SourcesResponse {
        sources: snapshot.sources.iter().map(|s| SourceSummary {
            id: s.id.clone(), source_type: s.source_type.clone(), name: s.name.clone(),
        }).collect(),
    })
}

async fn list_omt_sources(State(state): State<SharedStateHandle>) -> Json<SourcesResponse> {
    let snapshot = state.get_snapshot();
    Json(SourcesResponse {
        sources: snapshot.sources.iter().filter(|s| s.source_type == "omt").map(|s| SourceSummary {
            id: s.id.clone(), source_type: s.source_type.clone(), name: s.name.clone(),
        }).collect(),
    })
}

async fn refresh_omt_sources(State(state): State<SharedStateHandle>) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::RefreshOmtSources);
    Json(serde_json::json!({ "message": "OMT source refresh requested" }))
}

async fn list_ndi_sources(State(state): State<SharedStateHandle>) -> Json<SourcesResponse> {
    let snapshot = state.get_snapshot();
    Json(SourcesResponse {
        sources: snapshot.sources.iter().filter(|s| s.source_type == "ndi").map(|s| SourceSummary {
            id: s.id.clone(), source_type: s.source_type.clone(), name: s.name.clone(),
        }).collect(),
    })
}

async fn start_ndi_discovery(State(state): State<SharedStateHandle>) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::StartNdiDiscovery);
    Json(serde_json::json!({ "message": "NDI discovery start requested" }))
}

async fn stop_ndi_discovery(State(state): State<SharedStateHandle>) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::StopNdiDiscovery);
    Json(serde_json::json!({ "message": "NDI discovery stop requested" }))
}

async fn refresh_ndi_sources(State(state): State<SharedStateHandle>) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::RefreshNdiSources);
    Json(serde_json::json!({ "message": "NDI source refresh requested" }))
}

// ============================================================================
// Viewport Handlers
// ============================================================================

async fn get_viewport(State(state): State<SharedStateHandle>) -> Json<ViewportResponse> {
    let snapshot = state.get_snapshot();
    Json(ViewportResponse { zoom: snapshot.viewport.zoom, pan_x: snapshot.viewport.pan_x, pan_y: snapshot.viewport.pan_y })
}

async fn reset_viewport(State(state): State<SharedStateHandle>) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::ResetViewport);
    Json(serde_json::json!({ "message": "Viewport reset requested" }))
}

#[derive(serde::Deserialize)]
struct ZoomRequest { zoom: f32 }

async fn set_viewport_zoom(
    State(state): State<SharedStateHandle>,
    Json(req): Json<ZoomRequest>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::SetViewportZoom { zoom: req.zoom });
    Json(serde_json::json!({ "message": "Viewport zoom requested" }))
}

#[derive(serde::Deserialize)]
struct PanRequest { x: f32, y: f32 }

async fn set_viewport_pan(
    State(state): State<SharedStateHandle>,
    Json(req): Json<PanRequest>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::SetViewportPan { x: req.x, y: req.y });
    Json(serde_json::json!({ "message": "Viewport pan requested" }))
}

// ============================================================================
// Streaming - OMT Handlers
// ============================================================================

async fn get_omt_status(State(state): State<SharedStateHandle>) -> Json<OmtStatusResponse> {
    let snapshot = state.get_snapshot();
    Json(OmtStatusResponse {
        broadcasting: snapshot.streaming.omt_broadcasting,
        name: snapshot.streaming.omt_name,
        port: snapshot.streaming.omt_port,
        capture_fps: snapshot.streaming.omt_capture_fps,
    })
}

#[derive(serde::Deserialize)]
struct StartOmtRequest { name: String, port: u16 }

async fn start_omt_broadcast(
    State(state): State<SharedStateHandle>,
    Json(req): Json<StartOmtRequest>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::StartOmtBroadcast { name: req.name, port: req.port });
    Json(serde_json::json!({ "message": "OMT broadcast start requested" }))
}

async fn stop_omt_broadcast(State(state): State<SharedStateHandle>) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::StopOmtBroadcast);
    Json(serde_json::json!({ "message": "OMT broadcast stop requested" }))
}

#[derive(serde::Deserialize)]
struct OmtFpsRequest { fps: u32 }

async fn set_omt_fps(
    State(state): State<SharedStateHandle>,
    Json(req): Json<OmtFpsRequest>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::SetOmtCaptureFps { fps: req.fps });
    Json(serde_json::json!({ "message": "OMT FPS update requested" }))
}

// ============================================================================
// Streaming - NDI Handlers
// ============================================================================

async fn get_ndi_status(State(state): State<SharedStateHandle>) -> Json<NdiStatusResponse> {
    let snapshot = state.get_snapshot();
    Json(NdiStatusResponse {
        broadcasting: snapshot.streaming.ndi_broadcasting,
        name: snapshot.streaming.ndi_name,
    })
}

async fn start_ndi_broadcast(
    State(state): State<SharedStateHandle>,
    Json(req): Json<StartNdiRequest>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::StartNdiBroadcast { name: req.name });
    Json(serde_json::json!({ "message": "NDI broadcast start requested" }))
}

async fn stop_ndi_broadcast(State(state): State<SharedStateHandle>) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::StopNdiBroadcast);
    Json(serde_json::json!({ "message": "NDI broadcast stop requested" }))
}

// ============================================================================
// Streaming - Texture Share Handlers
// ============================================================================

async fn get_texture_status(State(state): State<SharedStateHandle>) -> Json<TextureShareResponse> {
    let snapshot = state.get_snapshot();
    Json(TextureShareResponse {
        sharing: snapshot.streaming.texture_sharing,
        name: Some("Immersive Server".into()),
    })
}

async fn start_texture_share(State(state): State<SharedStateHandle>) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::StartTextureShare);
    Json(serde_json::json!({ "message": "Texture sharing start requested" }))
}

async fn stop_texture_share(State(state): State<SharedStateHandle>) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::StopTextureShare);
    Json(serde_json::json!({ "message": "Texture sharing stop requested" }))
}

// ============================================================================
// Output Handlers
// ============================================================================

async fn list_outputs(State(state): State<SharedStateHandle>) -> Json<OutputsResponse> {
    let snapshot = state.get_snapshot();
    Json(OutputsResponse {
        outputs: snapshot.outputs.iter().map(|o| OutputSummary {
            id: o.id as usize,
            name: o.name.clone(),
            width: o.width,
            height: o.height,
            primary: o.is_primary,
        }).collect(),
    })
}

async fn get_output(
    State(state): State<SharedStateHandle>,
    Path(id): Path<usize>,
) -> Result<Json<OutputSummary>, (StatusCode, Json<ApiError>)> {
    let snapshot = state.get_snapshot();
    snapshot.outputs.iter()
        .find(|o| o.id as usize == id)
        .map(|o| Json(OutputSummary {
            id: o.id as usize,
            name: o.name.clone(),
            width: o.width,
            height: o.height,
            primary: o.is_primary,
        }))
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ApiError::not_found(format!("Output {} not found", id)))))
}

async fn update_output(
    State(_state): State<SharedStateHandle>,
    Path(_id): Path<usize>,
    Json(_req): Json<UpdateOutputRequest>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "message": "Output update requested" }))
}

// ============================================================================
// File Handlers
// ============================================================================

async fn get_current_file(State(state): State<SharedStateHandle>) -> Json<CurrentFileResponse> {
    let snapshot = state.get_snapshot();
    Json(CurrentFileResponse {
        path: snapshot.file.current_path,
        modified: snapshot.file.modified,
    })
}

async fn open_file(
    State(state): State<SharedStateHandle>,
    Json(req): Json<OpenFileRequest>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::OpenFile { path: req.path });
    Json(serde_json::json!({ "message": "File open requested" }))
}

async fn save_file(State(state): State<SharedStateHandle>) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::SaveFile);
    Json(serde_json::json!({ "message": "File save requested" }))
}

async fn save_file_as(
    State(state): State<SharedStateHandle>,
    Json(req): Json<SaveAsRequest>,
) -> Json<serde_json::Value> {
    let _ = state.send_command(ApiCommand::SaveFileAs { path: req.path });
    Json(serde_json::json!({ "message": "File save-as requested" }))
}

async fn list_recent_files(State(state): State<SharedStateHandle>) -> Json<RecentFilesResponse> {
    let snapshot = state.get_snapshot();
    Json(RecentFilesResponse { files: snapshot.file.recent_files })
}

// ============================================================================
// Helpers
// ============================================================================

fn parse_blend_mode(s: &str) -> Option<BlendMode> {
    match s.to_lowercase().as_str() {
        "normal" => Some(BlendMode::Normal),
        "additive" | "add" => Some(BlendMode::Additive),
        "multiply" => Some(BlendMode::Multiply),
        "screen" => Some(BlendMode::Screen),
        _ => None,
    }
}
