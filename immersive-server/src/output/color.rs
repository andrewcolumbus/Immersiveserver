//! Color correction definitions for per-output and per-slice adjustments
//!
//! Allows matching projector brightness, contrast, and color characteristics.

use serde::{Deserialize, Serialize};

/// Per-screen color correction (applied after all slices)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputColorCorrection {
    /// Brightness adjustment (-1.0 to 1.0, 0.0 = no change)
    #[serde(rename = "brightness")]
    pub brightness: f32,

    /// Contrast adjustment (0.0 to 2.0, 1.0 = no change)
    #[serde(rename = "contrast")]
    pub contrast: f32,

    /// Gamma adjustment (0.1 to 4.0, 1.0 = linear)
    #[serde(rename = "gamma")]
    pub gamma: f32,

    /// Red channel multiplier (0.0 to 2.0, 1.0 = no change)
    #[serde(rename = "red")]
    pub red: f32,

    /// Green channel multiplier (0.0 to 2.0, 1.0 = no change)
    #[serde(rename = "green")]
    pub green: f32,

    /// Blue channel multiplier (0.0 to 2.0, 1.0 = no change)
    #[serde(rename = "blue")]
    pub blue: f32,

    /// Saturation adjustment (0.0 = grayscale, 1.0 = normal, 2.0 = oversaturated)
    #[serde(rename = "saturation")]
    pub saturation: f32,
}

impl Default for OutputColorCorrection {
    fn default() -> Self {
        Self {
            brightness: 0.0,
            contrast: 1.0,
            gamma: 1.0,
            red: 1.0,
            green: 1.0,
            blue: 1.0,
            saturation: 1.0,
        }
    }
}

impl OutputColorCorrection {
    /// Create with identity values (no correction)
    pub fn identity() -> Self {
        Self::default()
    }

    /// Check if all values are at identity (no correction needed)
    pub fn is_identity(&self) -> bool {
        (self.brightness - 0.0).abs() < f32::EPSILON
            && (self.contrast - 1.0).abs() < f32::EPSILON
            && (self.gamma - 1.0).abs() < f32::EPSILON
            && (self.red - 1.0).abs() < f32::EPSILON
            && (self.green - 1.0).abs() < f32::EPSILON
            && (self.blue - 1.0).abs() < f32::EPSILON
            && (self.saturation - 1.0).abs() < f32::EPSILON
    }

    /// Reset to identity values
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Set brightness (clamped to -1.0 to 1.0)
    pub fn set_brightness(&mut self, value: f32) {
        self.brightness = value.clamp(-1.0, 1.0);
    }

    /// Set contrast (clamped to 0.0 to 2.0)
    pub fn set_contrast(&mut self, value: f32) {
        self.contrast = value.clamp(0.0, 2.0);
    }

    /// Set gamma (clamped to 0.1 to 4.0)
    pub fn set_gamma(&mut self, value: f32) {
        self.gamma = value.clamp(0.1, 4.0);
    }

    /// Set red channel (clamped to 0.0 to 2.0)
    pub fn set_red(&mut self, value: f32) {
        self.red = value.clamp(0.0, 2.0);
    }

    /// Set green channel (clamped to 0.0 to 2.0)
    pub fn set_green(&mut self, value: f32) {
        self.green = value.clamp(0.0, 2.0);
    }

    /// Set blue channel (clamped to 0.0 to 2.0)
    pub fn set_blue(&mut self, value: f32) {
        self.blue = value.clamp(0.0, 2.0);
    }

    /// Set saturation (clamped to 0.0 to 2.0)
    pub fn set_saturation(&mut self, value: f32) {
        self.saturation = value.clamp(0.0, 2.0);
    }
}

/// Per-slice color correction (applied to individual slice before compositing)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct SliceColorCorrection {
    /// Brightness adjustment (-1.0 to 1.0, 0.0 = no change)
    #[serde(rename = "brightness")]
    pub brightness: f32,

    /// Contrast adjustment (0.0 to 2.0, 1.0 = no change)
    #[serde(rename = "contrast")]
    pub contrast: f32,

    /// Gamma adjustment (0.1 to 4.0, 1.0 = linear)
    #[serde(rename = "gamma")]
    pub gamma: f32,

    /// Red channel multiplier (0.0 to 2.0, 1.0 = no change)
    #[serde(rename = "red")]
    pub red: f32,

    /// Green channel multiplier (0.0 to 2.0, 1.0 = no change)
    #[serde(rename = "green")]
    pub green: f32,

    /// Blue channel multiplier (0.0 to 2.0, 1.0 = no change)
    #[serde(rename = "blue")]
    pub blue: f32,

    /// Opacity multiplier (0.0 to 1.0, 1.0 = fully opaque)
    #[serde(rename = "opacity")]
    pub opacity: f32,
}

impl Default for SliceColorCorrection {
    fn default() -> Self {
        Self {
            brightness: 0.0,
            contrast: 1.0,
            gamma: 1.0,
            red: 1.0,
            green: 1.0,
            blue: 1.0,
            opacity: 1.0,
        }
    }
}

impl SliceColorCorrection {
    /// Create with identity values (no correction)
    pub fn identity() -> Self {
        Self::default()
    }

    /// Check if all values are at identity (no correction needed)
    pub fn is_identity(&self) -> bool {
        (self.brightness - 0.0).abs() < f32::EPSILON
            && (self.contrast - 1.0).abs() < f32::EPSILON
            && (self.gamma - 1.0).abs() < f32::EPSILON
            && (self.red - 1.0).abs() < f32::EPSILON
            && (self.green - 1.0).abs() < f32::EPSILON
            && (self.blue - 1.0).abs() < f32::EPSILON
            && (self.opacity - 1.0).abs() < f32::EPSILON
    }

    /// Reset to identity values
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Set brightness (clamped to -1.0 to 1.0)
    pub fn set_brightness(&mut self, value: f32) {
        self.brightness = value.clamp(-1.0, 1.0);
    }

    /// Set contrast (clamped to 0.0 to 2.0)
    pub fn set_contrast(&mut self, value: f32) {
        self.contrast = value.clamp(0.0, 2.0);
    }

    /// Set gamma (clamped to 0.1 to 4.0)
    pub fn set_gamma(&mut self, value: f32) {
        self.gamma = value.clamp(0.1, 4.0);
    }

    /// Set red channel (clamped to 0.0 to 2.0)
    pub fn set_red(&mut self, value: f32) {
        self.red = value.clamp(0.0, 2.0);
    }

    /// Set green channel (clamped to 0.0 to 2.0)
    pub fn set_green(&mut self, value: f32) {
        self.green = value.clamp(0.0, 2.0);
    }

    /// Set blue channel (clamped to 0.0 to 2.0)
    pub fn set_blue(&mut self, value: f32) {
        self.blue = value.clamp(0.0, 2.0);
    }

    /// Set opacity (clamped to 0.0 to 1.0)
    pub fn set_opacity(&mut self, value: f32) {
        self.opacity = value.clamp(0.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_color_default() {
        let color = OutputColorCorrection::default();
        assert!(color.is_identity());
    }

    #[test]
    fn test_output_color_modification() {
        let mut color = OutputColorCorrection::default();
        color.set_brightness(0.5);
        assert!(!color.is_identity());
        assert_eq!(color.brightness, 0.5);

        color.reset();
        assert!(color.is_identity());
    }

    #[test]
    fn test_output_color_clamping() {
        let mut color = OutputColorCorrection::default();
        color.set_brightness(2.0);
        assert_eq!(color.brightness, 1.0);

        color.set_gamma(0.0);
        assert_eq!(color.gamma, 0.1);
    }

    #[test]
    fn test_slice_color_default() {
        let color = SliceColorCorrection::default();
        assert!(color.is_identity());
        assert_eq!(color.opacity, 1.0);
    }
}
