//! Layer runtime state
//!
//! This module contains the runtime GPU resources for a layer.
//! The Layer struct in the compositor module is pure data (source path, transform, etc.),
//! while LayerRuntime holds the actual GPU resources needed for rendering.

use std::time::{Duration, Instant};

use crate::compositor::layer::Transform2D;
use crate::compositor::ClipTransition;
use crate::network::NdiReceiver;
use crate::video::{VideoPlayer, VideoTexture};

/// Runtime state for a layer, including GPU resources and video playback.
///
/// This struct is stored separately from the Layer data model to keep
/// the compositor module GPU-agnostic. The App maintains a HashMap
/// mapping layer IDs to their runtime state.
pub struct LayerRuntime {
    /// The layer ID this runtime belongs to
    pub layer_id: u32,

    /// Video player for this layer (if source is a video file)
    pub player: Option<VideoPlayer>,

    /// NDI receiver for this layer (if source is an NDI stream)
    pub ndi_receiver: Option<NdiReceiver>,

    /// GPU texture for video frames
    pub texture: Option<VideoTexture>,

    /// Bind group for rendering this layer's texture
    pub bind_group: Option<wgpu::BindGroup>,

    /// Cached video dimensions for param calculation
    pub video_width: u32,
    pub video_height: u32,

    /// Whether at least one frame has been uploaded to the texture.
    /// Used to prevent rendering empty/uninitialized textures.
    pub has_frame: bool,

    // Transition state
    /// Whether a transition is currently active
    pub transition_active: bool,
    /// When the transition started
    pub transition_start: Option<Instant>,
    /// Duration of the current transition
    pub transition_duration: Duration,
    /// Type of the current transition
    pub transition_type: ClipTransition,
    /// Old bind group for crossfade (kept during transition)
    pub old_bind_group: Option<wgpu::BindGroup>,
    /// Old video dimensions for crossfade rendering
    pub old_video_width: u32,
    pub old_video_height: u32,
    /// Old clip transform for crossfade (kept during transition)
    pub old_clip_transform: Option<Transform2D>,
    /// Old params buffer for crossfade (kept during transition)
    pub old_params_buffer: Option<wgpu::Buffer>,

    /// Per-layer params buffer for GPU uniforms.
    /// Each layer needs its own buffer to avoid overwriting during multi-layer rendering.
    pub params_buffer: Option<wgpu::Buffer>,

    // Fade-out state (for stopping clips with transition)
    /// Whether a fade-out is currently active (clip is being stopped)
    pub fade_out_active: bool,
    /// When the fade-out started
    pub fade_out_start: Option<Instant>,
    /// Duration of the fade-out
    pub fade_out_duration: Duration,
}

impl LayerRuntime {
    /// Create a new empty layer runtime
    pub fn new(layer_id: u32) -> Self {
        Self {
            layer_id,
            player: None,
            ndi_receiver: None,
            texture: None,
            bind_group: None,
            video_width: 0,
            video_height: 0,
            has_frame: false,
            // Transition state
            transition_active: false,
            transition_start: None,
            transition_duration: Duration::ZERO,
            transition_type: ClipTransition::Cut,
            old_bind_group: None,
            old_video_width: 0,
            old_video_height: 0,
            old_clip_transform: None,
            old_params_buffer: None,
            params_buffer: None,
            // Fade-out state
            fade_out_active: false,
            fade_out_start: None,
            fade_out_duration: Duration::ZERO,
        }
    }

    /// Check if this runtime has an active video source (file or NDI)
    pub fn has_video(&self) -> bool {
        self.player.is_some() || self.ndi_receiver.is_some()
    }

    /// Check if this runtime has an NDI source
    pub fn has_ndi(&self) -> bool {
        self.ndi_receiver.is_some()
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
    pub fn update_texture(&mut self, queue: &wgpu::Queue) {
        self.try_update_texture(queue);
    }

    /// Take the latest decoded frame (if any) and upload to texture.
    /// Returns true if a frame was uploaded, false if no new frame was available.
    ///
    /// Note: For GPU-native frames, the texture format may need to be updated.
    /// This requires the device to recreate the texture with the right format.
    pub fn try_update_texture(&mut self, queue: &wgpu::Queue) -> bool {
        let Some(texture) = &mut self.texture else { return false };

        // Try VideoPlayer first
        if let Some(player) = &self.player {
            if let Some(frame) = player.take_frame() {
                // Check if texture format matches frame format
                if frame.is_gpu_native != texture.is_gpu_native() {
                    tracing::warn!(
                        "Texture format mismatch: frame is_gpu_native={}, texture is_gpu_native={}",
                        frame.is_gpu_native,
                        texture.is_gpu_native()
                    );
                    return false;
                }

                texture.upload(queue, &frame);
                self.has_frame = true;
                return true;
            }
        }

        // Try NdiReceiver
        if let Some(ndi_receiver) = &mut self.ndi_receiver {
            if let Some(ndi_frame) = ndi_receiver.take_frame() {
                // Convert NDI frame to DecodedFrame format (BGRA -> RGBA)
                // NDI provides BGRA, our texture expects RGBA
                let mut rgba_data = ndi_frame.data.to_vec();
                for chunk in rgba_data.chunks_exact_mut(4) {
                    chunk.swap(0, 2); // Swap B and R channels
                }

                // Update stored dimensions if they changed
                if ndi_frame.width != self.video_width || ndi_frame.height != self.video_height {
                    self.video_width = ndi_frame.width;
                    self.video_height = ndi_frame.height;
                    tracing::info!(
                        "NDI frame resolution updated: {}x{}",
                        ndi_frame.width,
                        ndi_frame.height
                    );
                }

                let decoded_frame = crate::video::DecodedFrame::new(
                    rgba_data,
                    ndi_frame.width,
                    ndi_frame.height,
                    ndi_frame.timestamp.as_secs_f64(),
                    0, // NDI doesn't have frame indices
                );

                texture.upload(queue, &decoded_frame);
                self.has_frame = true;
                return true;
            }
        }

        false
    }

    /// Clear all resources
    pub fn clear(&mut self) {
        self.player = None;
        self.ndi_receiver = None;
        self.texture = None;
        self.bind_group = None;
        self.video_width = 0;
        self.video_height = 0;
        self.has_frame = false;
        // Clear transition state
        self.transition_active = false;
        self.transition_start = None;
        self.transition_duration = Duration::ZERO;
        self.transition_type = ClipTransition::Cut;
        self.old_bind_group = None;
        self.old_video_width = 0;
        self.old_video_height = 0;
        self.old_clip_transform = None;
        self.old_params_buffer = None;
        self.params_buffer = None;
        // Clear fade-out state
        self.fade_out_active = false;
        self.fade_out_start = None;
        self.fade_out_duration = Duration::ZERO;
    }

    /// Start a transition
    pub fn start_transition(&mut self, transition: ClipTransition) {
        self.transition_active = true;
        self.transition_start = Some(Instant::now());
        self.transition_duration = Duration::from_millis(transition.duration_ms() as u64);
        self.transition_type = transition;
    }

    /// Get the current transition progress (0.0 to 1.0)
    /// Returns 1.0 if no transition is active
    pub fn transition_progress(&self) -> f32 {
        if !self.transition_active {
            return 1.0;
        }
        
        let Some(start) = self.transition_start else {
            return 1.0;
        };

        if self.transition_duration.is_zero() {
            return 1.0;
        }

        let elapsed = start.elapsed();
        let progress = elapsed.as_secs_f32() / self.transition_duration.as_secs_f32();
        progress.clamp(0.0, 1.0)
    }

    /// Check if the transition is complete
    pub fn is_transition_complete(&self) -> bool {
        self.transition_progress() >= 1.0
    }

    /// End the transition and clean up old resources
    pub fn end_transition(&mut self) {
        self.transition_active = false;
        self.transition_start = None;
        self.transition_duration = Duration::ZERO;
        self.transition_type = ClipTransition::Cut;
        self.old_bind_group = None;
        self.old_video_width = 0;
        self.old_video_height = 0;
        self.old_clip_transform = None;
        self.old_params_buffer = None;
    }

    /// Start a fade-out (for stopping clips with transition)
    pub fn start_fade_out(&mut self, duration: Duration) {
        self.fade_out_active = true;
        self.fade_out_start = Some(Instant::now());
        self.fade_out_duration = duration;
    }

    /// Get the current fade-out progress (0.0 = just started, 1.0 = complete)
    pub fn fade_out_progress(&self) -> f32 {
        if !self.fade_out_active {
            return 1.0;
        }

        let Some(start) = self.fade_out_start else {
            return 1.0;
        };

        if self.fade_out_duration.is_zero() {
            return 1.0;
        }

        let elapsed = start.elapsed();
        let progress = elapsed.as_secs_f32() / self.fade_out_duration.as_secs_f32();
        progress.clamp(0.0, 1.0)
    }

    /// Check if the fade-out is complete
    pub fn is_fade_out_complete(&self) -> bool {
        self.fade_out_active && self.fade_out_progress() >= 1.0
    }
}




