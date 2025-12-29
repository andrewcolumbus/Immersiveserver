//! Clip types for composition
//!
//! Clips are the content sources that can be placed in layer columns.

#![allow(dead_code)]

use super::{ClipPlayback, PlaybackState};
use crate::video::{HapFrame, LoopMode, VideoPlayer};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Trigger mode determines how a clip responds to activation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TriggerMode {
    /// Click to start, click again to stop
    #[default]
    Toggle,
    /// Play while held/pressed
    Flash,
    /// Play once then stop
    OneShot,
}

impl TriggerMode {
    /// Get all trigger modes
    pub fn all() -> &'static [TriggerMode] {
        &[TriggerMode::Toggle, TriggerMode::Flash, TriggerMode::OneShot]
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            TriggerMode::Toggle => "Toggle",
            TriggerMode::Flash => "Flash",
            TriggerMode::OneShot => "One-Shot",
        }
    }
}

/// A slot in a layer column that holds a clip and its playback state
#[derive(Debug, Serialize, Deserialize)]
pub struct ClipSlot {
    /// Unique identifier for this slot
    pub id: Uuid,
    /// The clip content
    pub clip: Clip,
    /// How the clip responds to triggering
    pub trigger_mode: TriggerMode,
    /// Playback state
    pub playback: ClipPlayback,
    /// Clip-specific speed multiplier
    pub speed: f32,
    /// Clip-specific opacity
    pub opacity: f32,
    /// Runtime video player (not serialized, created on demand)
    #[serde(skip)]
    pub video_player: Option<VideoPlayer>,
}

impl Clone for ClipSlot {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            clip: self.clip.clone(),
            trigger_mode: self.trigger_mode,
            playback: self.playback.clone(),
            speed: self.speed,
            opacity: self.opacity,
            // Video player is runtime state; cloned slots start without a player
            video_player: None,
        }
    }
}

impl ClipSlot {
    /// Create a new clip slot with a clip
    pub fn new(clip: Clip) -> Self {
        let duration = clip.duration();
        Self {
            id: Uuid::new_v4(),
            clip,
            trigger_mode: TriggerMode::Toggle,
            playback: ClipPlayback::new(duration),
            speed: 1.0,
            opacity: 1.0,
            video_player: None,
        }
    }

    /// Create a clip slot with a specific trigger mode
    pub fn with_trigger_mode(mut self, mode: TriggerMode) -> Self {
        self.trigger_mode = mode;
        self
    }

    /// Update playback (also updates the video player if present)
    pub fn update(&mut self, delta_time: f64) {
        let effective_delta = delta_time * self.speed as f64;
        self.playback.update(effective_delta);

        // Update video player if we have one
        if let Some(player) = &mut self.video_player {
            player.update();
        }

        // Handle one-shot mode
        if self.trigger_mode == TriggerMode::OneShot
            && self.playback.state == PlaybackState::Playing
            && self.playback.progress() >= 1.0
        {
            self.playback.stop();
        }
    }

    /// Initialize the video player for video clips
    /// Call this when the clip is triggered to start playback
    pub fn init_video_player(&mut self) {
        if let Clip::Video(video_clip) = &self.clip {
            let mut player = VideoPlayer::new();
            if player.load(&video_clip.path).is_ok() {
                player.loop_mode = video_clip.loop_mode;
                player.speed = self.speed;
                self.video_player = Some(player);
                log::info!("Initialized video player for clip: {}", video_clip.name);
            } else {
                // Load a test pattern as fallback
                player.load_test_pattern(video_clip.dimensions.0, video_clip.dimensions.1);
                player.loop_mode = video_clip.loop_mode;
                self.video_player = Some(player);
                log::warn!("Using test pattern fallback for clip: {}", video_clip.name);
            }
        }
    }

    /// Start video playback
    pub fn start_video(&mut self) {
        // Initialize player if not already done
        if self.video_player.is_none() {
            self.init_video_player();
        }
        if let Some(player) = &mut self.video_player {
            player.play();
        }
    }

    /// Stop video playback
    pub fn stop_video(&mut self) {
        if let Some(player) = &mut self.video_player {
            player.stop();
        }
    }

    /// Get the current video frame if available
    pub fn current_frame(&self) -> Option<&HapFrame> {
        self.video_player.as_ref().and_then(|p| p.current_frame())
    }

    /// Get the clip's display name
    pub fn name(&self) -> String {
        self.clip.name()
    }

    /// Check if this clip has a thumbnail
    pub fn has_thumbnail(&self) -> bool {
        matches!(self.clip, Clip::Video(_) | Clip::Image(_))
    }
}

/// Content types that can be placed in a clip slot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Clip {
    /// Video clip (HAP format)
    Video(VideoClip),
    /// Still image
    Image(ImageClip),
    /// Solid color
    SolidColor(SolidColorClip),
    /// Procedural generator
    Generator(GeneratorClip),
}

impl Clip {
    /// Get the clip's display name
    pub fn name(&self) -> String {
        match self {
            Clip::Video(v) => v.name.clone(),
            Clip::Image(i) => i.name.clone(),
            Clip::SolidColor(s) => s.name.clone(),
            Clip::Generator(g) => g.generator_type.name().to_string(),
        }
    }

    /// Get the clip's duration in seconds
    pub fn duration(&self) -> f64 {
        match self {
            Clip::Video(v) => v.duration,
            Clip::Image(_) => f64::INFINITY, // Images have infinite duration
            Clip::SolidColor(_) => f64::INFINITY,
            Clip::Generator(_) => f64::INFINITY,
        }
    }

    /// Check if this clip is a video
    pub fn is_video(&self) -> bool {
        matches!(self, Clip::Video(_))
    }

    /// Check if this clip is an image
    pub fn is_image(&self) -> bool {
        matches!(self, Clip::Image(_))
    }

    /// Check if this clip is a generator
    pub fn is_generator(&self) -> bool {
        matches!(self, Clip::Generator(_))
    }
}

/// Video clip (HAP format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoClip {
    /// Display name
    pub name: String,
    /// Path to the video file
    pub path: PathBuf,
    /// Video duration in seconds
    pub duration: f64,
    /// Video dimensions
    pub dimensions: (u32, u32),
    /// Frame rate
    pub frame_rate: f64,
    /// Loop mode
    pub loop_mode: LoopMode,
}

impl VideoClip {
    /// Create a new video clip
    pub fn new(path: PathBuf) -> Self {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Video")
            .to_string();

        Self {
            name,
            path,
            duration: 10.0, // Will be updated when decoded
            dimensions: (1920, 1080),
            frame_rate: 30.0,
            loop_mode: LoopMode::Loop,
        }
    }

    /// Create with metadata
    pub fn with_metadata(
        path: PathBuf,
        duration: f64,
        dimensions: (u32, u32),
        frame_rate: f64,
    ) -> Self {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Video")
            .to_string();

        Self {
            name,
            path,
            duration,
            dimensions,
            frame_rate,
            loop_mode: LoopMode::Loop,
        }
    }
}

/// Still image clip
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageClip {
    /// Display name
    pub name: String,
    /// Path to the image file
    pub path: PathBuf,
    /// Image dimensions
    pub dimensions: (u32, u32),
}

impl ImageClip {
    /// Create a new image clip
    pub fn new(path: PathBuf) -> Self {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Image")
            .to_string();

        Self {
            name,
            path,
            dimensions: (1920, 1080), // Will be updated when loaded
        }
    }

    /// Create with known dimensions
    pub fn with_dimensions(path: PathBuf, dimensions: (u32, u32)) -> Self {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Image")
            .to_string();

        Self {
            name,
            path,
            dimensions,
        }
    }
}

/// Solid color clip
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolidColorClip {
    /// Display name
    pub name: String,
    /// Color (RGBA, 0.0-1.0)
    pub color: [f32; 4],
}

impl SolidColorClip {
    /// Create a new solid color clip
    pub fn new(color: [f32; 4]) -> Self {
        Self {
            name: "Solid Color".to_string(),
            color,
        }
    }

    /// Create with a name
    pub fn with_name(name: impl Into<String>, color: [f32; 4]) -> Self {
        Self {
            name: name.into(),
            color,
        }
    }

    /// Create black
    pub fn black() -> Self {
        Self::with_name("Black", [0.0, 0.0, 0.0, 1.0])
    }

    /// Create white
    pub fn white() -> Self {
        Self::with_name("White", [1.0, 1.0, 1.0, 1.0])
    }

    /// Create red
    pub fn red() -> Self {
        Self::with_name("Red", [1.0, 0.0, 0.0, 1.0])
    }

    /// Create green
    pub fn green() -> Self {
        Self::with_name("Green", [0.0, 1.0, 0.0, 1.0])
    }

    /// Create blue
    pub fn blue() -> Self {
        Self::with_name("Blue", [0.0, 0.0, 1.0, 1.0])
    }
}

/// Procedural generator clip
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratorClip {
    /// Generator type with parameters
    pub generator_type: GeneratorType,
    /// Speed multiplier for animation
    pub speed: f32,
}

impl GeneratorClip {
    /// Create a new generator clip
    pub fn new(generator_type: GeneratorType) -> Self {
        Self {
            generator_type,
            speed: 1.0,
        }
    }
}

/// Types of procedural generators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GeneratorType {
    /// Noise pattern
    Noise {
        seed: u64,
        scale: f32,
        octaves: u32,
    },
    /// Linear or radial gradient
    Gradient {
        colors: Vec<[f32; 4]>,
        angle: f32,
        gradient_type: GradientType,
    },
    /// Test pattern (grid, color bars, etc.)
    TestPattern(TestPatternType),
    /// Plasma effect
    Plasma { speed: f32, scale: f32 },
    /// Solid bars (for alignment)
    ColorBars,
}

impl GeneratorType {
    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            GeneratorType::Noise { .. } => "Noise",
            GeneratorType::Gradient { .. } => "Gradient",
            GeneratorType::TestPattern(_) => "Test Pattern",
            GeneratorType::Plasma { .. } => "Plasma",
            GeneratorType::ColorBars => "Color Bars",
        }
    }

    /// Create a simple noise generator
    pub fn simple_noise() -> Self {
        GeneratorType::Noise {
            seed: 0,
            scale: 1.0,
            octaves: 4,
        }
    }

    /// Create a horizontal gradient
    pub fn horizontal_gradient(start: [f32; 4], end: [f32; 4]) -> Self {
        GeneratorType::Gradient {
            colors: vec![start, end],
            angle: 0.0,
            gradient_type: GradientType::Linear,
        }
    }

    /// Create a vertical gradient
    pub fn vertical_gradient(start: [f32; 4], end: [f32; 4]) -> Self {
        GeneratorType::Gradient {
            colors: vec![start, end],
            angle: 90.0,
            gradient_type: GradientType::Linear,
        }
    }
}

/// Gradient types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GradientType {
    /// Linear gradient
    Linear,
    /// Radial gradient from center
    Radial,
    /// Angular/conical gradient
    Angular,
}

/// Test pattern types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestPatternType {
    /// Grid pattern
    Grid,
    /// SMPTE color bars
    ColorBars,
    /// Checkerboard
    Checkerboard,
    /// Crosshatch
    Crosshatch,
    /// Resolution chart
    ResolutionChart,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clip_slot_creation() {
        let clip = Clip::SolidColor(SolidColorClip::black());
        let slot = ClipSlot::new(clip);
        assert_eq!(slot.trigger_mode, TriggerMode::Toggle);
        assert_eq!(slot.speed, 1.0);
    }

    #[test]
    fn test_video_clip() {
        let clip = VideoClip::new(PathBuf::from("/path/to/video.mov"));
        assert_eq!(clip.name, "video");
        assert_eq!(clip.loop_mode, LoopMode::Loop);
    }
}


