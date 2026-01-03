//! Configuration and serialization module.

use serde::{Deserialize, Serialize};

/// Project configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Project name.
    pub name: String,
    /// Canvas width (combined projector output).
    pub canvas_width: u32,
    /// Canvas height.
    pub canvas_height: u32,
    /// List of projectors.
    pub projectors: Vec<ProjectorConfig>,
    /// NDI camera source name.
    pub camera_source: Option<String>,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            name: "New Project".to_string(),
            canvas_width: 1920,
            canvas_height: 1080,
            projectors: vec![ProjectorConfig::default()],
            camera_source: None,
        }
    }
}

/// Per-projector configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectorConfig {
    /// Unique ID.
    pub id: u32,
    /// Display name.
    pub name: String,
    /// Native resolution width.
    pub width: u32,
    /// Native resolution height.
    pub height: u32,
    /// Display adapter index.
    pub display_index: usize,
    /// Position in canvas (top-left X).
    pub canvas_x: i32,
    /// Position in canvas (top-left Y).
    pub canvas_y: i32,
    /// Computed homography (3x3 matrix, row-major).
    pub homography: Option<[f64; 9]>,
    /// Edge blend settings.
    pub blend: BlendConfig,
}

impl Default for ProjectorConfig {
    fn default() -> Self {
        Self {
            id: 1,
            name: "Projector 1".to_string(),
            width: 1920,
            height: 1080,
            display_index: 0,
            canvas_x: 0,
            canvas_y: 0,
            homography: None,
            blend: BlendConfig::default(),
        }
    }
}

/// Edge blend configuration for a projector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlendConfig {
    /// Left edge blend width.
    pub left_width: u32,
    /// Right edge blend width.
    pub right_width: u32,
    /// Top edge blend width.
    pub top_width: u32,
    /// Bottom edge blend width.
    pub bottom_width: u32,
    /// Gamma correction.
    pub gamma: f32,
    /// Blend curve type.
    pub curve: BlendCurve,
}

impl Default for BlendConfig {
    fn default() -> Self {
        Self {
            left_width: 0,
            right_width: 0,
            top_width: 0,
            bottom_width: 0,
            gamma: 2.2,
            curve: BlendCurve::Gamma,
        }
    }
}

/// Blend curve types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendCurve {
    Linear,
    Gamma,
    Cosine,
    Smoothstep,
}

impl Default for BlendCurve {
    fn default() -> Self {
        BlendCurve::Gamma
    }
}
