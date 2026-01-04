//! Preview Player
//!
//! Standalone video playback for clip preview, independent from layer runtime.
//! Used by the Preview Monitor panel to show clips before triggering them live.

use std::path::Path;

use crate::video::{VideoPlayer, VideoRenderer, VideoTexture};

/// Manages preview video playback
pub struct PreviewPlayer {
    /// Video player for preview
    player: Option<VideoPlayer>,
    /// GPU texture for preview frames
    texture: Option<VideoTexture>,
    /// Bind group for rendering preview
    bind_group: Option<wgpu::BindGroup>,
    /// Params buffer for GPU uniforms
    params_buffer: Option<wgpu::Buffer>,
    /// Video dimensions
    width: u32,
    height: u32,
    /// Whether at least one frame has been uploaded
    has_frame: bool,
    /// Whether BC textures are supported (for HAP)
    bc_texture_supported: bool,
    /// egui texture ID for displaying in UI
    pub egui_texture_id: Option<egui::TextureId>,
}

impl PreviewPlayer {
    /// Create a new preview player
    pub fn new(bc_texture_supported: bool) -> Self {
        Self {
            player: None,
            texture: None,
            bind_group: None,
            params_buffer: None,
            width: 0,
            height: 0,
            has_frame: false,
            bc_texture_supported,
            egui_texture_id: None,
        }
    }

    /// Load a video file for preview
    pub fn load(
        &mut self,
        path: &Path,
        device: &wgpu::Device,
        video_renderer: &VideoRenderer,
    ) -> Result<(), String> {
        // Open video player (starts background decode thread)
        let player =
            VideoPlayer::open(path).map_err(|e| format!("Failed to open preview video: {}", e))?;

        tracing::info!(
            "Preview: Loaded video {}x{} @ {:.2}fps, duration: {:.2}s",
            player.width(),
            player.height(),
            player.frame_rate(),
            player.duration()
        );

        // Create video texture with appropriate format
        let use_gpu_native = player.is_hap() && self.bc_texture_supported;

        let video_texture = if use_gpu_native {
            tracing::info!("Preview: Using GPU-native BC texture (HAP fast path)");
            VideoTexture::new_gpu_native(device, player.width(), player.height(), player.is_bc3())
        } else {
            VideoTexture::new(device, player.width(), player.height())
        };

        // Create params buffer
        let params_buffer = video_renderer.create_params_buffer(device);

        // Create bind group
        let bind_group =
            video_renderer.create_bind_group_with_buffer(device, &video_texture, &params_buffer);

        self.width = player.width();
        self.height = player.height();
        self.player = Some(player);
        self.texture = Some(video_texture);
        self.bind_group = Some(bind_group);
        self.params_buffer = Some(params_buffer);
        self.has_frame = false;

        Ok(())
    }

    /// Update preview (upload new frames from decode thread)
    /// Returns true if a frame was uploaded
    pub fn update(&mut self, queue: &wgpu::Queue) -> bool {
        let Some(player) = &self.player else {
            return false;
        };
        let Some(texture) = &mut self.texture else {
            return false;
        };

        if let Some(frame) = player.take_frame() {
            // Check format match
            if frame.is_gpu_native != texture.is_gpu_native() {
                tracing::warn!(
                    "Preview texture format mismatch: frame is_gpu_native={}, texture is_gpu_native={}",
                    frame.is_gpu_native,
                    texture.is_gpu_native()
                );
                return false;
            }

            texture.upload(queue, &frame);
            self.has_frame = true;
            return true;
        }
        false
    }

    /// Get the bind group for rendering
    pub fn bind_group(&self) -> Option<&wgpu::BindGroup> {
        self.bind_group.as_ref()
    }

    /// Get the params buffer for rendering
    pub fn params_buffer(&self) -> Option<&wgpu::Buffer> {
        self.params_buffer.as_ref()
    }

    /// Get the texture view for egui registration
    pub fn texture_view(&self) -> Option<&wgpu::TextureView> {
        self.texture.as_ref().map(|t| t.view())
    }

    /// Get video dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Check if preview has valid frame ready to render
    pub fn has_frame(&self) -> bool {
        self.has_frame
    }

    /// Check if a video is loaded
    pub fn is_loaded(&self) -> bool {
        self.player.is_some()
    }

    /// Toggle pause state
    pub fn toggle_pause(&self) {
        if let Some(player) = &self.player {
            player.toggle_pause();
        }
    }

    /// Pause playback
    pub fn pause(&self) {
        if let Some(player) = &self.player {
            player.pause();
        }
    }

    /// Resume playback
    pub fn resume(&self) {
        if let Some(player) = &self.player {
            player.resume();
        }
    }

    /// Restart from beginning
    pub fn restart(&self) {
        if let Some(player) = &self.player {
            player.restart();
        }
    }

    /// Seek to a specific time in seconds
    pub fn seek(&self, time_secs: f64) {
        if let Some(player) = &self.player {
            player.seek(time_secs);
        }
    }

    /// Check if paused
    pub fn is_paused(&self) -> bool {
        self.player.as_ref().map(|p| p.is_paused()).unwrap_or(true)
    }

    /// Clear the preview (stop playback and release resources)
    pub fn clear(&mut self) {
        self.player = None;
        self.texture = None;
        self.bind_group = None;
        self.params_buffer = None;
        self.width = 0;
        self.height = 0;
        self.has_frame = false;
        self.egui_texture_id = None;
    }

    /// Get video info (dimensions, fps, duration, position)
    pub fn video_info(&self) -> Option<VideoInfo> {
        self.player.as_ref().map(|p| {
            let frame_index = p.frame_index();
            let frame_rate = p.frame_rate();
            let position = if frame_rate > 0.0 {
                frame_index as f64 / frame_rate
            } else {
                0.0
            };
            VideoInfo {
                width: p.width(),
                height: p.height(),
                frame_rate,
                duration: p.duration(),
                position,
                frame_index,
            }
        })
    }

    /// Get the current frame index
    pub fn frame_index(&self) -> u64 {
        self.player.as_ref().map(|p| p.frame_index()).unwrap_or(0)
    }

    /// Get the current playback position in seconds
    pub fn position(&self) -> f64 {
        self.player.as_ref().map(|p| {
            let frame_rate = p.frame_rate();
            if frame_rate > 0.0 {
                p.frame_index() as f64 / frame_rate
            } else {
                0.0
            }
        }).unwrap_or(0.0)
    }
}

/// Video information for display
#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub frame_rate: f64,
    pub duration: f64,
    /// Current playback position in seconds (approximate based on frame index)
    pub position: f64,
    /// Current frame index
    pub frame_index: u64,
}
