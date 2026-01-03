//! Previs types and settings
//!
//! Defines the configuration for 3D previsualization surfaces.

use serde::{Deserialize, Serialize};

/// Surface type for 3D preview
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SurfaceType {
    /// Flat circular surface on ground (floor projection)
    #[default]
    Circle,
    /// 4 individual walls (front, back, left, right)
    Walls,
    /// Hemisphere dome (inside view, like planetarium)
    Dome,
}

/// Settings for an individual wall
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallSettings {
    /// Whether this wall is enabled/visible
    #[serde(rename = "enabled", default = "default_wall_enabled")]
    pub enabled: bool,
    /// Width of the wall
    #[serde(rename = "width", default = "default_individual_wall_width")]
    pub width: f32,
    /// Height of the wall
    #[serde(rename = "height", default = "default_individual_wall_height")]
    pub height: f32,
}

fn default_wall_enabled() -> bool {
    true
}

fn default_individual_wall_width() -> f32 {
    4.0
}

fn default_individual_wall_height() -> f32 {
    3.0
}

impl Default for WallSettings {
    fn default() -> Self {
        Self {
            enabled: default_wall_enabled(),
            width: default_individual_wall_width(),
            height: default_individual_wall_height(),
        }
    }
}

impl SurfaceType {
    /// Get display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            SurfaceType::Circle => "Circle (Floor)",
            SurfaceType::Walls => "Walls (Cave)",
            SurfaceType::Dome => "Dome (Planetarium)",
        }
    }

    /// Get all surface types for iteration
    pub fn all() -> &'static [SurfaceType] {
        &[SurfaceType::Circle, SurfaceType::Walls, SurfaceType::Dome]
    }
}

/// Settings for the previs panel (serialized in .immersive files)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrevisSettings {
    /// Current surface type
    #[serde(rename = "surfaceType", default)]
    pub surface_type: SurfaceType,

    /// Whether previs is enabled
    #[serde(rename = "enabled", default)]
    pub enabled: bool,

    // Circle parameters
    /// Circle radius in world units
    #[serde(rename = "circleRadius", default = "default_circle_radius")]
    pub circle_radius: f32,
    /// Circle mesh segments (detail level)
    #[serde(rename = "circleSegments", default = "default_circle_segments")]
    pub circle_segments: u32,

    // Individual wall settings (4 walls: front, back, left, right)
    /// Front wall settings (facing -Z direction)
    #[serde(rename = "wallFront", default)]
    pub wall_front: WallSettings,
    /// Back wall settings (facing +Z direction)
    #[serde(rename = "wallBack", default)]
    pub wall_back: WallSettings,
    /// Left wall settings (facing +X direction)
    #[serde(rename = "wallLeft", default)]
    pub wall_left: WallSettings,
    /// Right wall settings (facing -X direction)
    #[serde(rename = "wallRight", default)]
    pub wall_right: WallSettings,

    // Dome parameters
    /// Dome radius in world units
    #[serde(rename = "domeRadius", default = "default_dome_radius")]
    pub dome_radius: f32,
    /// Dome horizontal segments (longitude divisions)
    #[serde(rename = "domeSegmentsH", default = "default_dome_segments_h")]
    pub dome_segments_horizontal: u32,
    /// Dome vertical segments (latitude divisions)
    #[serde(rename = "domeSegmentsV", default = "default_dome_segments_v")]
    pub dome_segments_vertical: u32,

    // Floor settings (for walls mode)
    /// Whether floor is enabled
    #[serde(rename = "floorEnabled", default)]
    pub floor_enabled: bool,
    /// Which layer index to display on the floor (0 = first layer)
    #[serde(rename = "floorLayerIndex", default)]
    pub floor_layer_index: usize,

    // Camera state (saved for persistence)
    /// Camera horizontal rotation (yaw) in radians
    #[serde(rename = "cameraYaw", default)]
    pub camera_yaw: f32,
    /// Camera vertical rotation (pitch) in radians
    #[serde(rename = "cameraPitch", default = "default_camera_pitch")]
    pub camera_pitch: f32,
    /// Camera distance from center
    #[serde(rename = "cameraDistance", default = "default_camera_distance")]
    pub camera_distance: f32,
}

fn default_circle_radius() -> f32 {
    5.0
}
fn default_circle_segments() -> u32 {
    32
}
fn default_dome_radius() -> f32 {
    5.0
}
fn default_dome_segments_h() -> u32 {
    32
}
fn default_dome_segments_v() -> u32 {
    16
}
fn default_camera_pitch() -> f32 {
    0.3 // Slight downward angle
}
fn default_camera_distance() -> f32 {
    10.0
}

impl Default for PrevisSettings {
    fn default() -> Self {
        Self {
            surface_type: SurfaceType::default(),
            enabled: false,
            circle_radius: default_circle_radius(),
            circle_segments: default_circle_segments(),
            wall_front: WallSettings::default(),
            wall_back: WallSettings::default(),
            wall_left: WallSettings::default(),
            wall_right: WallSettings::default(),
            dome_radius: default_dome_radius(),
            dome_segments_horizontal: default_dome_segments_h(),
            dome_segments_vertical: default_dome_segments_v(),
            floor_enabled: false,
            floor_layer_index: 0,
            camera_yaw: 0.0,
            camera_pitch: default_camera_pitch(),
            camera_distance: default_camera_distance(),
        }
    }
}
