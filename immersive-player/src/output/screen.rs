//! Screen definition for output targets
//!
//! A Screen represents a physical output target (projector, display, etc.)

#![allow(dead_code)]

use super::{BlendConfig, OutputDevice, Slice};
use serde::{Deserialize, Serialize};

/// A screen represents a physical output target
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screen {
    /// Unique identifier
    pub id: u32,
    /// Display name
    pub name: String,
    /// Associated display ID (legacy, use device instead)
    pub display_id: u32,
    /// Output device configuration
    pub device: OutputDevice,
    /// Resolution in pixels
    pub resolution: (u32, u32),
    /// Position on the canvas (for dragging/arranging)
    pub position: (f32, f32),
    /// Slices (regions) within this screen
    pub slices: Vec<Slice>,
    /// Edge blending configuration
    pub blend_config: BlendConfig,
    /// Overall opacity (0.0 to 1.0)
    pub opacity: f32,
    /// Brightness adjustment (-1.0 to 1.0)
    pub brightness: f32,
    /// Contrast adjustment (0.0 to 2.0)
    pub contrast: f32,
    /// RGB channel adjustments
    pub rgb_adjust: RgbAdjust,
    /// Output delay in frames (for sync)
    pub delay_frames: u32,
    /// Whether this screen is enabled
    pub enabled: bool,
}

/// RGB channel adjustments
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RgbAdjust {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}

impl Default for RgbAdjust {
    fn default() -> Self {
        Self {
            red: 1.0,
            green: 1.0,
            blue: 1.0,
        }
    }
}

impl Screen {
    /// Create a new screen with auto-positioned based on index
    pub fn new(name: String, display_id: u32, resolution: (u32, u32)) -> Self {
        Self::new_at_position(name, display_id, resolution, (0.0, 0.0))
    }

    /// Create a new screen at a specific position
    pub fn new_at_position(name: String, display_id: u32, resolution: (u32, u32), position: (f32, f32)) -> Self {
        Self {
            id: 0,
            name,
            display_id,
            device: OutputDevice::new_virtual(resolution.0, resolution.1),
            resolution,
            position,
            slices: Vec::new(),
            blend_config: BlendConfig::default(),
            opacity: 1.0,
            brightness: 0.0,
            contrast: 1.0,
            rgb_adjust: RgbAdjust::default(),
            delay_frames: 0,
            enabled: true,
        }
    }

    /// Create a new screen with a specific output device
    pub fn new_with_device(name: String, device: OutputDevice, resolution: (u32, u32), position: (f32, f32)) -> Self {
        Self {
            id: 0,
            name,
            display_id: 0,
            device,
            resolution,
            position,
            slices: Vec::new(),
            blend_config: BlendConfig::default(),
            opacity: 1.0,
            brightness: 0.0,
            contrast: 1.0,
            rgb_adjust: RgbAdjust::default(),
            delay_frames: 0,
            enabled: true,
        }
    }

    /// Add a slice to this screen
    pub fn add_slice(&mut self, slice: Slice) {
        self.slices.push(slice);
    }

    /// Remove a slice by index
    pub fn remove_slice(&mut self, index: usize) -> Option<Slice> {
        if index < self.slices.len() {
            Some(self.slices.remove(index))
        } else {
            None
        }
    }

    /// Get the total number of slices
    pub fn slice_count(&self) -> usize {
        self.slices.len()
    }

    /// Get the aspect ratio
    pub fn aspect_ratio(&self) -> f32 {
        self.resolution.0 as f32 / self.resolution.1 as f32
    }

    /// Check if any blending is enabled
    pub fn has_blending(&self) -> bool {
        self.blend_config.left.is_some()
            || self.blend_config.right.is_some()
            || self.blend_config.top.is_some()
            || self.blend_config.bottom.is_some()
    }
}

impl Default for Screen {
    fn default() -> Self {
        Self::new("Screen".to_string(), 0, (1920, 1080))
    }
}

/// Flip mode for slices
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FlipMode {
    #[default]
    None,
    Horizontal,
    Vertical,
    Both,
}

