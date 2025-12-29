//! Edge blending for multi-projector setups
//!
//! Implements soft-edge blending for overlapping projector regions.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Configuration for a single blend edge
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EdgeBlend {
    /// Width of the blend region in pixels
    pub width: u32,
    /// Power/gamma curve for the blend (typically 2.0-2.5)
    pub power: f32,
    /// Gamma correction for the blend curve
    pub gamma: f32,
    /// Black level compensation (0.0 to 1.0)
    pub black_level: f32,
}

impl Default for EdgeBlend {
    fn default() -> Self {
        Self {
            width: 100,
            power: 2.2,
            gamma: 1.0,
            black_level: 0.0,
        }
    }
}

impl EdgeBlend {
    /// Create a new edge blend with specified width
    pub fn new(width: u32) -> Self {
        Self {
            width,
            ..Default::default()
        }
    }

    /// Calculate blend factor at a given position (0.0 to 1.0)
    pub fn blend_factor(&self, t: f32) -> f32 {
        // Apply power curve with gamma correction
        let t_clamped = t.clamp(0.0, 1.0);
        let blended = t_clamped.powf(self.power);
        
        // Apply gamma
        let gamma_corrected = blended.powf(1.0 / self.gamma);
        
        // Add black level compensation
        gamma_corrected * (1.0 - self.black_level) + self.black_level * t_clamped
    }

    /// Get the blend factor for a pixel position
    pub fn get_factor_for_pixel(&self, pixel: u32, is_start_edge: bool) -> f32 {
        if self.width == 0 {
            return 1.0;
        }
        
        let t = if is_start_edge {
            pixel as f32 / self.width as f32
        } else {
            1.0 - (pixel as f32 / self.width as f32)
        };
        
        self.blend_factor(t)
    }
}

/// Configuration for all edges of a screen
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlendConfig {
    /// Left edge blend
    pub left: Option<EdgeBlend>,
    /// Right edge blend
    pub right: Option<EdgeBlend>,
    /// Top edge blend
    pub top: Option<EdgeBlend>,
    /// Bottom edge blend
    pub bottom: Option<EdgeBlend>,
}

impl BlendConfig {
    /// Create a new blend config with no blending
    pub fn none() -> Self {
        Self::default()
    }

    /// Create horizontal blend (left-right)
    pub fn horizontal(overlap_pixels: u32) -> Self {
        Self {
            left: None,
            right: Some(EdgeBlend::new(overlap_pixels)),
            top: None,
            bottom: None,
        }
    }

    /// Create matching blend for the other projector
    pub fn horizontal_pair(overlap_pixels: u32) -> (Self, Self) {
        let left_proj = Self {
            left: None,
            right: Some(EdgeBlend::new(overlap_pixels)),
            top: None,
            bottom: None,
        };
        
        let right_proj = Self {
            left: Some(EdgeBlend::new(overlap_pixels)),
            right: None,
            top: None,
            bottom: None,
        };
        
        (left_proj, right_proj)
    }

    /// Check if any blending is configured
    pub fn has_blend(&self) -> bool {
        self.left.is_some() || self.right.is_some() || self.top.is_some() || self.bottom.is_some()
    }

    /// Get the total blend width for all edges
    pub fn total_blend_area(&self) -> u32 {
        let mut total = 0;
        if let Some(b) = &self.left { total += b.width; }
        if let Some(b) = &self.right { total += b.width; }
        if let Some(b) = &self.top { total += b.width; }
        if let Some(b) = &self.bottom { total += b.width; }
        total
    }
}

/// Blend curve presets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendPreset {
    Linear,
    Smooth,
    Aggressive,
    Custom,
}

impl BlendPreset {
    /// Get the power value for this preset
    pub fn power(&self) -> f32 {
        match self {
            BlendPreset::Linear => 1.0,
            BlendPreset::Smooth => 2.0,
            BlendPreset::Aggressive => 3.0,
            BlendPreset::Custom => 2.2,
        }
    }

    /// Create an EdgeBlend from this preset
    pub fn to_edge_blend(&self, width: u32) -> EdgeBlend {
        EdgeBlend {
            width,
            power: self.power(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blend_factor() {
        let blend = EdgeBlend::default();
        
        // At start of blend, factor should be low
        assert!(blend.blend_factor(0.0) < 0.1);
        
        // At end of blend, factor should be high
        assert!(blend.blend_factor(1.0) > 0.9);
        
        // At middle, should be somewhere in between
        let mid = blend.blend_factor(0.5);
        assert!(mid > 0.1 && mid < 0.9);
    }

    #[test]
    fn test_horizontal_pair() {
        let (left, right) = BlendConfig::horizontal_pair(200);
        
        assert!(left.right.is_some());
        assert!(left.left.is_none());
        assert!(right.left.is_some());
        assert!(right.right.is_none());
    }
}



