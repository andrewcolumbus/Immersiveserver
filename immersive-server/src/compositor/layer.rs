//! Layer types for the compositor
//!
//! A Layer represents a single compositing element within the Environment.
//! Layers have a source (video, NDI, etc.), transform, opacity, and blend mode.

use std::path::PathBuf;

use crate::compositor::BlendMode;

/// 2D transform for layer positioning within the environment.
///
/// The transform is applied in the following order:
/// 1. Translate to anchor point
/// 2. Scale
/// 3. Rotate
/// 4. Translate to final position
#[derive(Debug, Clone, PartialEq)]
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
}

/// Source type for a layer's content.
///
/// Defines what content the layer displays. This is extensible
/// for future source types (NDI, OMT, images, etc.)
#[derive(Debug, Clone, PartialEq)]
pub enum LayerSource {
    /// No source - layer is empty/transparent
    None,
    /// Video file source
    Video {
        /// Path to the video file
        path: PathBuf,
    },
    // Future source types:
    // Ndi { source_name: String },
    // Omt { source_id: String },
    // Image { path: PathBuf },
    // SolidColor { color: [f32; 4] },
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
/// opacity, and blend mode.
#[derive(Debug, Clone)]
pub struct Layer {
    /// Unique identifier for this layer
    pub id: u32,
    /// Human-readable name for the layer
    pub name: String,
    /// The content source for this layer
    pub source: LayerSource,
    /// 2D transform (position, scale, rotation)
    pub transform: Transform2D,
    /// Opacity from 0.0 (transparent) to 1.0 (opaque)
    pub opacity: f32,
    /// Blend mode for compositing with layers below
    pub blend_mode: BlendMode,
    /// Whether the layer is visible
    pub visible: bool,
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
        }
    }

    /// Create a new layer with a video source
    pub fn with_video(id: u32, name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            id,
            name: name.into(),
            source: LayerSource::Video { path: path.into() },
            transform: Transform2D::default(),
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            visible: true,
        }
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
    }

    #[test]
    fn test_layer_with_video() {
        let layer = Layer::with_video(1, "Video Layer", "/path/to/video.mp4");
        assert_eq!(layer.source, LayerSource::Video {
            path: PathBuf::from("/path/to/video.mp4")
        });
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
}

