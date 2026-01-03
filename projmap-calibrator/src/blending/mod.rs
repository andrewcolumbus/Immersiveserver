//! Edge blending module for multi-projector setups.

mod overlap;

use crate::config::BlendCurve;

pub use overlap::{OverlapConfig, OverlapDetectionResult, OverlapDetector};

/// Overlap region between two projectors.
#[derive(Debug, Clone)]
pub struct OverlapRegion {
    /// First projector ID.
    pub projector_a: u32,
    /// Second projector ID.
    pub projector_b: u32,
    /// Overlap width in pixels.
    pub overlap_width: u32,
    /// Edge of projector_a that overlaps (Left, Right, Top, Bottom).
    pub edge: OverlapEdge,
}

/// Which edge overlaps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlapEdge {
    Left,
    Right,
    Top,
    Bottom,
}

/// Blend mask for a projector.
#[derive(Debug, Clone)]
pub struct BlendMask {
    /// Width of the mask texture.
    pub width: u32,
    /// Height of the mask texture.
    pub height: u32,
    /// Blend values 0.0-1.0 (row-major storage).
    pub data: Vec<f32>,
    /// Blend curve type used.
    pub curve: BlendCurve,
}

impl BlendMask {
    /// Create a new blend mask filled with 1.0 (no blending).
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![1.0; (width * height) as usize],
            curve: BlendCurve::default(),
        }
    }

    /// Apply blend curve to normalized distance (0-1).
    pub fn apply_curve(t: f32, curve: BlendCurve) -> f32 {
        match curve {
            BlendCurve::Linear => t,
            BlendCurve::Gamma => t.powf(2.2),
            BlendCurve::Cosine => 0.5 - 0.5 * (std::f32::consts::PI * t).cos(),
            BlendCurve::Smoothstep => {
                let t2 = t * t;
                3.0 * t2 - 2.0 * t * t2
            }
        }
    }

    /// Generate edge blend falloff on the right edge.
    pub fn apply_right_blend(&mut self, blend_width: u32, curve: BlendCurve) {
        if blend_width == 0 {
            return;
        }

        self.curve = curve;
        let start_x = self.width.saturating_sub(blend_width);

        for y in 0..self.height {
            for x in start_x..self.width {
                let t = (x - start_x) as f32 / blend_width as f32;
                let blend = 1.0 - Self::apply_curve(t, curve);
                let idx = (y * self.width + x) as usize;
                self.data[idx] *= blend;
            }
        }
    }

    /// Generate edge blend falloff on the left edge.
    pub fn apply_left_blend(&mut self, blend_width: u32, curve: BlendCurve) {
        if blend_width == 0 {
            return;
        }

        self.curve = curve;

        for y in 0..self.height {
            for x in 0..blend_width.min(self.width) {
                let t = 1.0 - (x as f32 / blend_width as f32);
                let blend = 1.0 - Self::apply_curve(t, curve);
                let idx = (y * self.width + x) as usize;
                self.data[idx] *= blend;
            }
        }
    }
}

/// Per-projector color correction.
#[derive(Debug, Clone)]
pub struct ColorCorrection {
    /// Brightness multiplier (0.0-2.0, 1.0 = no change).
    pub brightness: f32,
    /// Per-channel gain (R, G, B).
    pub gain: [f32; 3],
    /// Gamma correction.
    pub gamma: f32,
    /// Black level offset.
    pub black_level: f32,
}

impl Default for ColorCorrection {
    fn default() -> Self {
        Self {
            brightness: 1.0,
            gain: [1.0, 1.0, 1.0],
            gamma: 1.0,
            black_level: 0.0,
        }
    }
}
