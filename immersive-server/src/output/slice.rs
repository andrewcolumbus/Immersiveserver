//! Slice definitions for input-to-output mapping
//!
//! A Slice defines how a region of the composition (or a specific layer)
//! maps to a region of the output screen.

use serde::{Deserialize, Serialize};

use super::color::SliceColorCorrection;
use super::edge_blend::EdgeBlendConfig;
use super::mask::SliceMask;
use super::warp::WarpMesh;

/// Unique identifier for a slice
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct SliceId(pub u32);

impl std::fmt::Display for SliceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Slice {}", self.0)
    }
}

/// Normalized rectangle (0.0-1.0 coordinates)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    /// Left edge (0.0-1.0)
    pub x: f32,
    /// Top edge (0.0-1.0)
    pub y: f32,
    /// Width (0.0-1.0)
    pub width: f32,
    /// Height (0.0-1.0)
    pub height: f32,
}

impl Default for Rect {
    fn default() -> Self {
        Self::full()
    }
}

impl Rect {
    /// Create a full-coverage rect (entire surface)
    pub fn full() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        }
    }

    /// Create a rect from normalized coordinates
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create a rect covering a specific quadrant
    pub fn quadrant(col: u32, row: u32, cols: u32, rows: u32) -> Self {
        let w = 1.0 / cols as f32;
        let h = 1.0 / rows as f32;
        Self {
            x: col as f32 * w,
            y: row as f32 * h,
            width: w,
            height: h,
        }
    }

    /// Check if this rect covers the full surface
    pub fn is_full(&self) -> bool {
        (self.x - 0.0).abs() < f32::EPSILON
            && (self.y - 0.0).abs() < f32::EPSILON
            && (self.width - 1.0).abs() < f32::EPSILON
            && (self.height - 1.0).abs() < f32::EPSILON
    }

    /// Get aspect ratio (width / height)
    pub fn aspect_ratio(&self) -> f32 {
        if self.height > 0.0 {
            self.width / self.height
        } else {
            1.0
        }
    }

    /// Get the right edge x coordinate
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Get the bottom edge y coordinate
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// Clamp values to valid range (0.0-1.0)
    pub fn clamped(&self) -> Self {
        Self {
            x: self.x.clamp(0.0, 1.0),
            y: self.y.clamp(0.0, 1.0),
            width: self.width.clamp(0.0, 1.0 - self.x.clamp(0.0, 1.0)),
            height: self.height.clamp(0.0, 1.0 - self.y.clamp(0.0, 1.0)),
        }
    }
}

/// Input source for a slice
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SliceInput {
    /// Sample from the full composited environment
    #[serde(rename = "Composition")]
    Composition,

    /// Sample from a specific layer (pre-composition)
    #[serde(rename = "Layer")]
    Layer {
        /// ID of the layer to sample
        #[serde(rename = "layerId")]
        layer_id: u32,
    },
}

impl Default for SliceInput {
    fn default() -> Self {
        Self::Composition
    }
}

impl SliceInput {
    /// Get a display name for this input source
    pub fn display_name(&self) -> String {
        match self {
            SliceInput::Composition => "Composition".to_string(),
            SliceInput::Layer { layer_id } => format!("Layer {}", layer_id),
        }
    }
}

/// 2D point for warp corners and control points
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2D {
    pub x: f32,
    pub y: f32,
}

impl Default for Point2D {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

impl Point2D {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Output transformation for a slice
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SliceOutput {
    /// Position and size on output (normalized 0.0-1.0)
    #[serde(rename = "rect")]
    pub rect: Rect,

    /// Rotation in degrees
    #[serde(rename = "rotation")]
    pub rotation: f32,

    /// Horizontal flip
    #[serde(rename = "flipH")]
    pub flip_h: bool,

    /// Vertical flip
    #[serde(rename = "flipV")]
    pub flip_v: bool,

    /// 4-corner perspective warp (optional, overridden by mesh)
    #[serde(rename = "perspective", skip_serializing_if = "Option::is_none")]
    pub perspective: Option<[Point2D; 4]>,

    /// Grid warp mesh (optional, overrides perspective)
    #[serde(rename = "mesh", skip_serializing_if = "Option::is_none")]
    pub mesh: Option<WarpMesh>,

    /// Edge blending configuration
    #[serde(rename = "edgeBlend", default)]
    pub edge_blend: EdgeBlendConfig,
}

impl Default for SliceOutput {
    fn default() -> Self {
        Self {
            rect: Rect::full(),
            rotation: 0.0,
            flip_h: false,
            flip_v: false,
            perspective: None,
            mesh: None,
            edge_blend: EdgeBlendConfig::default(),
        }
    }
}

impl SliceOutput {
    /// Check if any transformation is applied
    pub fn has_transform(&self) -> bool {
        !self.rect.is_full()
            || self.rotation.abs() > f32::EPSILON
            || self.flip_h
            || self.flip_v
            || self.perspective.is_some()
            || self.mesh.is_some()
    }
}

/// A slice mapping input to output
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Slice {
    /// Unique slice identifier
    #[serde(rename = "id")]
    pub id: SliceId,

    /// Display name
    #[serde(rename = "name")]
    pub name: String,

    /// What to sample (composition or specific layer)
    #[serde(rename = "input")]
    pub input: SliceInput,

    /// Where on input to sample (Input Selection rect)
    #[serde(rename = "inputRect")]
    pub input_rect: Rect,

    /// Where/how to render on output (Output Transformation)
    #[serde(rename = "output")]
    pub output: SliceOutput,

    /// Optional mask for this slice
    #[serde(rename = "mask", skip_serializing_if = "Option::is_none")]
    pub mask: Option<SliceMask>,

    /// Per-slice color correction
    #[serde(rename = "colorCorrection", default)]
    pub color: SliceColorCorrection,

    /// Whether this slice is enabled
    #[serde(rename = "enabled")]
    pub enabled: bool,

    /// Whether to output as luminance key
    #[serde(rename = "isKey")]
    pub is_key: bool,

    /// Force black background (instead of transparent)
    #[serde(rename = "blackBg")]
    pub black_bg: bool,
}

impl Default for Slice {
    fn default() -> Self {
        Self {
            id: SliceId::default(),
            name: "Slice".to_string(),
            input: SliceInput::Composition,
            input_rect: Rect::full(),
            output: SliceOutput::default(),
            mask: None,
            color: SliceColorCorrection::default(),
            enabled: true,
            is_key: false,
            black_bg: false,
        }
    }
}

impl Slice {
    /// Create a new slice
    pub fn new(id: SliceId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            ..Default::default()
        }
    }

    /// Create a slice that shows the full composition
    pub fn new_full_composition(id: SliceId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            input: SliceInput::Composition,
            input_rect: Rect::full(),
            output: SliceOutput::default(),
            ..Default::default()
        }
    }

    /// Create a slice for a specific layer
    pub fn new_layer(id: SliceId, name: impl Into<String>, layer_id: u32) -> Self {
        Self {
            id,
            name: name.into(),
            input: SliceInput::Layer { layer_id },
            input_rect: Rect::full(),
            output: SliceOutput::default(),
            ..Default::default()
        }
    }

    /// Create a quadrant slice (for multi-projector setups)
    pub fn new_quadrant(
        id: SliceId,
        name: impl Into<String>,
        col: u32,
        row: u32,
        cols: u32,
        rows: u32,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            input: SliceInput::Composition,
            input_rect: Rect::quadrant(col, row, cols, rows),
            output: SliceOutput::default(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_full() {
        let rect = Rect::full();
        assert!(rect.is_full());
        assert_eq!(rect.aspect_ratio(), 1.0);
    }

    #[test]
    fn test_rect_quadrant() {
        let rect = Rect::quadrant(0, 0, 2, 2);
        assert_eq!(rect.x, 0.0);
        assert_eq!(rect.y, 0.0);
        assert_eq!(rect.width, 0.5);
        assert_eq!(rect.height, 0.5);

        let rect2 = Rect::quadrant(1, 1, 2, 2);
        assert_eq!(rect2.x, 0.5);
        assert_eq!(rect2.y, 0.5);
    }

    #[test]
    fn test_slice_default() {
        let slice = Slice::default();
        assert!(slice.enabled);
        assert!(matches!(slice.input, SliceInput::Composition));
        assert!(slice.input_rect.is_full());
    }

    #[test]
    fn test_slice_layer_input() {
        let slice = Slice::new_layer(SliceId(1), "Test", 5);
        assert!(matches!(slice.input, SliceInput::Layer { layer_id: 5 }));
    }
}
