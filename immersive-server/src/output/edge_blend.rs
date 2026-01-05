//! Edge blending configuration for seamless projector overlap
//!
//! Provides soft gradient edges for blending multiple projectors together.

use serde::{Deserialize, Serialize};

/// Edge blending configuration for all four edges
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EdgeBlendConfig {
    /// Left edge blend
    #[serde(rename = "left")]
    pub left: EdgeBlendRegion,

    /// Right edge blend
    #[serde(rename = "right")]
    pub right: EdgeBlendRegion,

    /// Top edge blend
    #[serde(rename = "top")]
    pub top: EdgeBlendRegion,

    /// Bottom edge blend
    #[serde(rename = "bottom")]
    pub bottom: EdgeBlendRegion,
}

impl EdgeBlendConfig {
    /// Check if any edge blending is enabled
    pub fn is_any_enabled(&self) -> bool {
        self.left.enabled || self.right.enabled || self.top.enabled || self.bottom.enabled
    }

    /// Enable blending on left and right edges (horizontal overlap)
    pub fn horizontal(width: f32, gamma: f32) -> Self {
        Self {
            left: EdgeBlendRegion::new(width, gamma),
            right: EdgeBlendRegion::new(width, gamma),
            top: EdgeBlendRegion::default(),
            bottom: EdgeBlendRegion::default(),
        }
    }

    /// Enable blending on top and bottom edges (vertical overlap)
    pub fn vertical(width: f32, gamma: f32) -> Self {
        Self {
            left: EdgeBlendRegion::default(),
            right: EdgeBlendRegion::default(),
            top: EdgeBlendRegion::new(width, gamma),
            bottom: EdgeBlendRegion::new(width, gamma),
        }
    }

    /// Enable blending on all edges
    pub fn all(width: f32, gamma: f32) -> Self {
        let region = EdgeBlendRegion::new(width, gamma);
        Self {
            left: region.clone(),
            right: region.clone(),
            top: region.clone(),
            bottom: region,
        }
    }

    /// Disable all edge blending
    pub fn disable_all(&mut self) {
        self.left.enabled = false;
        self.right.enabled = false;
        self.top.enabled = false;
        self.bottom.enabled = false;
    }
}

/// Configuration for a single edge blend region
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EdgeBlendRegion {
    /// Whether this edge blend is enabled
    #[serde(rename = "enabled")]
    pub enabled: bool,

    /// Blend region width (0.0-0.5 of output width/height)
    #[serde(rename = "width")]
    pub width: f32,

    /// Gamma curve for blend falloff (default 2.2)
    #[serde(rename = "gamma")]
    pub gamma: f32,

    /// Black level compensation (reduces visible "halo" in overlap)
    #[serde(rename = "blackLevel", default)]
    pub black_level: f32,
}

impl Default for EdgeBlendRegion {
    fn default() -> Self {
        Self {
            enabled: false,
            width: 0.15,      // 15% overlap is typical
            gamma: 2.2,       // Standard gamma
            black_level: 0.0, // No black level compensation
        }
    }
}

impl EdgeBlendRegion {
    /// Create an enabled blend region with specified width and gamma
    pub fn new(width: f32, gamma: f32) -> Self {
        Self {
            enabled: true,
            width: width.clamp(0.0, 0.5),
            gamma: gamma.clamp(0.1, 4.0),
            black_level: 0.0,
        }
    }

    /// Create a typical 15% overlap blend region
    pub fn typical() -> Self {
        Self::new(0.15, 2.2)
    }

    /// Set the blend width (clamped to 0.0-0.5)
    pub fn set_width(&mut self, width: f32) {
        self.width = width.clamp(0.0, 0.5);
    }

    /// Set the gamma curve (clamped to 0.1-4.0)
    pub fn set_gamma(&mut self, gamma: f32) {
        self.gamma = gamma.clamp(0.1, 4.0);
    }

    /// Set the black level compensation (clamped to 0.0-0.5)
    pub fn set_black_level(&mut self, level: f32) {
        self.black_level = level.clamp(0.0, 0.5);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = EdgeBlendConfig::default();
        assert!(!config.is_any_enabled());
    }

    #[test]
    fn test_horizontal_blend() {
        let config = EdgeBlendConfig::horizontal(0.15, 2.2);
        assert!(config.is_any_enabled());
        assert!(config.left.enabled);
        assert!(config.right.enabled);
        assert!(!config.top.enabled);
        assert!(!config.bottom.enabled);
    }

    #[test]
    fn test_region_clamping() {
        let mut region = EdgeBlendRegion::default();
        region.set_width(1.0);
        assert_eq!(region.width, 0.5);

        region.set_gamma(0.0);
        assert_eq!(region.gamma, 0.1);

        region.set_gamma(10.0);
        assert_eq!(region.gamma, 4.0);
    }
}
