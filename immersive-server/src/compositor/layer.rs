//! Layer types for the compositor
//!
//! A Layer represents a single compositing element within the Environment.
//! Layers have a source (video, NDI, etc.), transform, opacity, blend mode,
//! and clip slots for triggering video playback.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::compositor::clip::{ClipCell, DEFAULT_CLIP_SLOTS};
use crate::compositor::BlendMode;
use crate::effects::EffectStack;

/// 2D transform for layer positioning within the environment.
///
/// The transform is applied in the following order:
/// 1. Translate to anchor point
/// 2. Scale
/// 3. Rotate
/// 4. Translate to final position
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transform2D {
    /// Position in pixels relative to environment origin (top-left)
    pub position: (f32, f32),
    /// Scale factors (1.0 = 100%, 2.0 = 200%, etc.)
    pub scale: (f32, f32),
    /// Rotation in radians (clockwise)
    pub rotation: f32,
    /// Anchor point for rotation and scaling (0.0-1.0, where 0.5,0.5 = center)
    pub anchor: (f32, f32),
}

impl Default for Transform2D {
    fn default() -> Self {
        Self {
            position: (0.0, 0.0),
            scale: (1.0, 1.0),
            rotation: 0.0,
            anchor: (0.5, 0.5), // Center anchor by default
        }
    }
}

impl Transform2D {
    /// Create a new transform with default values (identity transform at origin)
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a transform at a specific position
    pub fn at_position(x: f32, y: f32) -> Self {
        Self {
            position: (x, y),
            ..Default::default()
        }
    }

    /// Create a transform with specific scale
    pub fn with_scale(scale_x: f32, scale_y: f32) -> Self {
        Self {
            scale: (scale_x, scale_y),
            ..Default::default()
        }
    }

    /// Create a transform with uniform scale
    pub fn with_uniform_scale(scale: f32) -> Self {
        Self::with_scale(scale, scale)
    }

    /// Check if this is an identity transform (no change from default)
    pub fn is_identity(&self) -> bool {
        self.position == (0.0, 0.0)
            && self.scale == (1.0, 1.0)
            && self.rotation == 0.0
            && self.anchor == (0.5, 0.5)
    }
}

/// Source type for a layer's content.
///
/// Defines what content the layer displays. This is extensible
/// for future source types (NDI, OMT, images, etc.)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayerSource {
    /// No source - layer is empty/transparent
    None,
    /// Video file source (path to video file)
    Video(PathBuf),
    // Future source types:
    // Ndi(String),       // source_name
    // Omt(String),       // source_id
    // Image(PathBuf),    // path
    // SolidColor([f32; 4]), // RGBA color
}

impl Default for LayerSource {
    fn default() -> Self {
        Self::None
    }
}

/// A compositing layer within the Environment.
///
/// Layers are rendered back-to-front based on their order in the
/// Environment's layer list. Each layer has its own source, transform,
/// opacity, blend mode, and clip slots for video triggering.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Layer {
    /// Unique identifier for this layer
    pub id: u32,
    /// Human-readable name for the layer
    pub name: String,
    /// The content source for this layer (set automatically when clip is triggered)
    /// This is runtime state, not saved - on load, source is always None
    #[serde(skip)]
    pub source: LayerSource,
    /// 2D transform (position, scale, rotation)
    pub transform: Transform2D,
    /// Opacity from 0.0 (transparent) to 1.0 (opaque)
    pub opacity: f32,
    /// Blend mode for compositing with layers below
    pub blend_mode: BlendMode,
    /// Whether the layer is visible
    pub visible: bool,
    /// Clip slots for this layer (1D array of clips)
    pub clips: Vec<Option<ClipCell>>,
    /// Currently active/playing clip slot index, if any
    /// This is runtime state, not saved - on load, no clip is active
    #[serde(skip)]
    pub active_clip: Option<usize>,
    /// Transition mode for clips on this layer
    #[serde(default)]
    pub transition: crate::compositor::ClipTransition,
    /// Horizontal tiling (1 = no repeat, 2 = 2x repeat, etc.)
    #[serde(default = "default_tile")]
    pub tile_x: u32,
    /// Vertical tiling (1 = no repeat, 2 = 2x repeat, etc.)
    #[serde(default = "default_tile")]
    pub tile_y: u32,
    /// Effect stack for this layer
    #[serde(default)]
    pub effects: EffectStack,
}

fn default_tile() -> u32 {
    1
}

impl Default for Layer {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            source: LayerSource::None,
            transform: Transform2D::default(),
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            visible: true,
            clips: Vec::new(),
            active_clip: None,
            transition: crate::compositor::ClipTransition::Cut,
            tile_x: 1,
            tile_y: 1,
            effects: EffectStack::default(),
        }
    }
}

impl Layer {
    /// Create a new layer with the given ID and name
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            source: LayerSource::None,
            transform: Transform2D::default(),
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            visible: true,
            clips: vec![None; DEFAULT_CLIP_SLOTS],
            active_clip: None,
            transition: crate::compositor::ClipTransition::Cut,
            tile_x: 1,
            tile_y: 1,
            effects: EffectStack::new(),
        }
    }

    /// Create a new layer with a video source
    pub fn with_video(id: u32, name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            id,
            name: name.into(),
            source: LayerSource::Video(path.into()),
            transform: Transform2D::default(),
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            visible: true,
            clips: vec![None; DEFAULT_CLIP_SLOTS],
            active_clip: None,
            transition: crate::compositor::ClipTransition::Cut,
            tile_x: 1,
            tile_y: 1,
            effects: EffectStack::new(),
        }
    }

    /// Set the layer's tiling
    pub fn set_tiling(&mut self, tile_x: u32, tile_y: u32) {
        self.tile_x = tile_x.max(1);
        self.tile_y = tile_y.max(1);
    }

    /// Get the number of clip slots
    pub fn clip_count(&self) -> usize {
        self.clips.len()
    }

    /// Get a clip at the given slot index (returns None for invalid/empty clips)
    pub fn get_clip(&self, slot: usize) -> Option<&ClipCell> {
        self.clips.get(slot).and_then(|c| c.as_ref()).filter(|c| c.is_valid())
    }

    /// Get a mutable reference to a clip at the given slot index
    pub fn get_clip_mut(&mut self, slot: usize) -> Option<&mut ClipCell> {
        self.clips.get_mut(slot).and_then(|c| c.as_mut())
    }

    /// Set a clip at the given slot index
    pub fn set_clip(&mut self, slot: usize, cell: ClipCell) -> bool {
        if slot < self.clips.len() {
            self.clips[slot] = Some(cell);
            true
        } else {
            false
        }
    }

    /// Clear a clip at the given slot index
    pub fn clear_clip(&mut self, slot: usize) -> bool {
        if slot < self.clips.len() {
            self.clips[slot] = None;
            true
        } else {
            false
        }
    }

    /// Check if this layer has an active clip
    pub fn has_active_clip(&self) -> bool {
        self.active_clip.is_some()
    }

    /// Get the active clip slot index, if any
    pub fn active_clip_slot(&self) -> Option<usize> {
        self.active_clip
    }

    /// Clear the active clip (stop playback indicator)
    pub fn clear_active_clip(&mut self) {
        self.active_clip = None;
    }

    /// Set the layer's source
    pub fn set_source(&mut self, source: LayerSource) {
        self.source = source;
    }

    /// Set the layer's position
    pub fn set_position(&mut self, x: f32, y: f32) {
        self.transform.position = (x, y);
    }

    /// Set the layer's scale
    pub fn set_scale(&mut self, scale_x: f32, scale_y: f32) {
        self.transform.scale = (scale_x, scale_y);
    }

    /// Set uniform scale for the layer
    pub fn set_uniform_scale(&mut self, scale: f32) {
        self.transform.scale = (scale, scale);
    }

    /// Set the layer's rotation in radians
    pub fn set_rotation(&mut self, radians: f32) {
        self.transform.rotation = radians;
    }

    /// Set the layer's rotation in degrees
    pub fn set_rotation_degrees(&mut self, degrees: f32) {
        self.transform.rotation = degrees.to_radians();
    }

    /// Set the layer's opacity (clamped to 0.0-1.0)
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }

    /// Set the layer's blend mode
    pub fn set_blend_mode(&mut self, mode: BlendMode) {
        self.blend_mode = mode;
    }

    /// Show the layer
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hide the layer
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Toggle layer visibility
    pub fn toggle_visibility(&mut self) {
        self.visible = !self.visible;
    }

    /// Resize the clips array, preserving existing clips
    pub fn resize_clips(&mut self, new_count: usize) {
        if new_count > self.clips.len() {
            self.clips.resize(new_count, None);
        } else if new_count < self.clips.len() {
            self.clips.truncate(new_count);
            // If active clip is now out of bounds, clear it
            if let Some(active) = self.active_clip {
                if active >= new_count {
                    self.active_clip = None;
                }
            }
        }
    }

    /// Iterate over all clip slots
    pub fn iter_clips(&self) -> impl Iterator<Item = (usize, Option<&ClipCell>)> {
        self.clips.iter().enumerate().map(|(i, c)| (i, c.as_ref()))
    }

    /// Count non-empty clip slots
    pub fn filled_clip_count(&self) -> usize {
        self.clips.iter().filter(|c| c.is_some()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_default() {
        let t = Transform2D::default();
        assert_eq!(t.position, (0.0, 0.0));
        assert_eq!(t.scale, (1.0, 1.0));
        assert_eq!(t.rotation, 0.0);
        assert_eq!(t.anchor, (0.5, 0.5));
    }

    #[test]
    fn test_transform_at_position() {
        let t = Transform2D::at_position(100.0, 200.0);
        assert_eq!(t.position, (100.0, 200.0));
        assert_eq!(t.scale, (1.0, 1.0));
    }

    #[test]
    fn test_layer_new() {
        let layer = Layer::new(1, "Test Layer");
        assert_eq!(layer.id, 1);
        assert_eq!(layer.name, "Test Layer");
        assert_eq!(layer.source, LayerSource::None);
        assert_eq!(layer.opacity, 1.0);
        assert!(layer.visible);
        assert_eq!(layer.clip_count(), DEFAULT_CLIP_SLOTS);
    }

    #[test]
    fn test_layer_with_video() {
        let layer = Layer::with_video(1, "Video Layer", "/path/to/video.mp4");
        assert_eq!(layer.source, LayerSource::Video(
            PathBuf::from("/path/to/video.mp4")
        ));
    }

    #[test]
    fn test_layer_opacity_clamping() {
        let mut layer = Layer::new(1, "Test");
        
        layer.set_opacity(1.5);
        assert_eq!(layer.opacity, 1.0);
        
        layer.set_opacity(-0.5);
        assert_eq!(layer.opacity, 0.0);
        
        layer.set_opacity(0.5);
        assert_eq!(layer.opacity, 0.5);
    }

    #[test]
    fn test_layer_visibility() {
        let mut layer = Layer::new(1, "Test");
        assert!(layer.visible);
        
        layer.hide();
        assert!(!layer.visible);
        
        layer.show();
        assert!(layer.visible);
        
        layer.toggle_visibility();
        assert!(!layer.visible);
    }

    #[test]
    fn test_layer_rotation_degrees() {
        let mut layer = Layer::new(1, "Test");
        layer.set_rotation_degrees(90.0);
        assert!((layer.transform.rotation - std::f32::consts::FRAC_PI_2).abs() < 0.001);
    }

    #[test]
    fn test_layer_clips() {
        let mut layer = Layer::new(1, "Test");
        assert_eq!(layer.clip_count(), DEFAULT_CLIP_SLOTS);
        assert!(layer.get_clip(0).is_none());

        // Set a clip
        let cell = ClipCell::new("/path/to/video.mp4");
        assert!(layer.set_clip(0, cell));
        assert!(layer.get_clip(0).is_some());
        assert_eq!(layer.filled_clip_count(), 1);

        // Clear the clip
        assert!(layer.clear_clip(0));
        assert!(layer.get_clip(0).is_none());
        assert_eq!(layer.filled_clip_count(), 0);
    }

    #[test]
    fn test_layer_resize_clips() {
        let mut layer = Layer::new(1, "Test");
        layer.set_clip(3, ClipCell::new("/path.mp4"));
        layer.active_clip = Some(3);

        // Resize smaller (clip and active should be lost)
        layer.resize_clips(2);
        assert_eq!(layer.clip_count(), 2);
        assert!(layer.get_clip(3).is_none()); // Out of bounds
        assert!(layer.active_clip.is_none()); // Cleared

        // Resize larger
        layer.resize_clips(10);
        assert_eq!(layer.clip_count(), 10);
    }
}
