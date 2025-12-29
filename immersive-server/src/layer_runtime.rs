//! Layer runtime state
//!
//! This module contains the runtime GPU resources for a layer.
//! The Layer struct in the compositor module is pure data (source path, transform, etc.),
//! while LayerRuntime holds the actual GPU resources needed for rendering.

use crate::video::{VideoPlayer, VideoTexture};

/// Runtime state for a layer, including GPU resources and video playback.
///
/// This struct is stored separately from the Layer data model to keep
/// the compositor module GPU-agnostic. The App maintains a HashMap
/// mapping layer IDs to their runtime state.
pub struct LayerRuntime {
    /// The layer ID this runtime belongs to
    pub layer_id: u32,

    /// Video player for this layer (if source is a video)
    pub player: Option<VideoPlayer>,

    /// GPU texture for video frames
    pub texture: Option<VideoTexture>,

    /// Bind group for rendering this layer's texture
    pub bind_group: Option<wgpu::BindGroup>,

    /// Cached video dimensions for param calculation
    pub video_width: u32,
    pub video_height: u32,
}

impl LayerRuntime {
    /// Create a new empty layer runtime
    pub fn new(layer_id: u32) -> Self {
        Self {
            layer_id,
            player: None,
            texture: None,
            bind_group: None,
            video_width: 0,
            video_height: 0,
        }
    }

    /// Check if this runtime has an active video source
    pub fn has_video(&self) -> bool {
        self.player.is_some()
    }

    /// Check if video is paused
    pub fn is_paused(&self) -> bool {
        self.player.as_ref().map(|p| p.is_paused()).unwrap_or(true)
    }

    /// Toggle video pause state
    pub fn toggle_pause(&self) {
        if let Some(player) = &self.player {
            player.toggle_pause();
        }
    }

    /// Restart video from beginning
    pub fn restart(&self) {
        if let Some(player) = &self.player {
            player.restart();
        }
    }

    /// Take the latest decoded frame (if any) and upload to texture
    pub fn update_texture(&self, queue: &wgpu::Queue) {
        let Some(player) = &self.player else { return };
        let Some(texture) = &self.texture else { return };

        if let Some(frame) = player.take_frame() {
            texture.upload(queue, &frame);
        }
    }

    /// Clear all resources
    pub fn clear(&mut self) {
        self.player = None;
        self.texture = None;
        self.bind_group = None;
        self.video_width = 0;
        self.video_height = 0;
    }
}

