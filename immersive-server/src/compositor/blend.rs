//! Blend mode definitions and utilities
//!
//! Defines the available blend modes for layer compositing and provides
//! conversion to wgpu BlendState for GPU rendering.

use serde::{Deserialize, Serialize};

/// Blend modes for layer compositing.
///
/// These modes determine how a layer's pixels are combined with the
/// pixels of layers below it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum BlendMode {
    /// Standard alpha blending (Porter-Duff source-over)
    /// Result = Source × SourceAlpha + Dest × (1 - SourceAlpha)
    #[default]
    Normal,

    /// Additive blending (linear dodge)
    /// Result = Source + Dest
    /// Good for glow effects, light beams, fire
    Additive,

    /// Multiply blending
    /// Result = Source × Dest
    /// Darkens the image, good for shadows
    Multiply,

    /// Screen blending
    /// Result = 1 - (1 - Source) × (1 - Dest)
    /// Lightens the image, opposite of multiply
    Screen,
}

impl BlendMode {
    /// Convert blend mode to wgpu BlendState for GPU rendering.
    ///
    /// Returns the appropriate blend state configuration for the
    /// fragment shader output.
    pub fn to_blend_state(self) -> wgpu::BlendState {
        match self {
            BlendMode::Normal => wgpu::BlendState::ALPHA_BLENDING,

            BlendMode::Additive => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            },

            BlendMode::Multiply => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Dst,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::DstAlpha,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
            },

            BlendMode::Screen => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrc,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            },
        }
    }

    /// Get a human-readable name for the blend mode
    pub fn name(&self) -> &'static str {
        match self {
            BlendMode::Normal => "Normal",
            BlendMode::Additive => "Additive",
            BlendMode::Multiply => "Multiply",
            BlendMode::Screen => "Screen",
        }
    }

    /// Get all available blend modes
    pub fn all() -> &'static [BlendMode] {
        &[
            BlendMode::Normal,
            BlendMode::Additive,
            BlendMode::Multiply,
            BlendMode::Screen,
        ]
    }
}

impl std::fmt::Display for BlendMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blend_mode_default() {
        assert_eq!(BlendMode::default(), BlendMode::Normal);
    }

    #[test]
    fn test_blend_mode_names() {
        assert_eq!(BlendMode::Normal.name(), "Normal");
        assert_eq!(BlendMode::Additive.name(), "Additive");
        assert_eq!(BlendMode::Multiply.name(), "Multiply");
        assert_eq!(BlendMode::Screen.name(), "Screen");
    }

    #[test]
    fn test_blend_mode_display() {
        assert_eq!(format!("{}", BlendMode::Normal), "Normal");
        assert_eq!(format!("{}", BlendMode::Additive), "Additive");
    }

    #[test]
    fn test_blend_mode_all() {
        let all = BlendMode::all();
        assert_eq!(all.len(), 4);
        assert!(all.contains(&BlendMode::Normal));
        assert!(all.contains(&BlendMode::Additive));
        assert!(all.contains(&BlendMode::Multiply));
        assert!(all.contains(&BlendMode::Screen));
    }

    #[test]
    fn test_to_blend_state() {
        // Just verify each mode converts without panic
        let _ = BlendMode::Normal.to_blend_state();
        let _ = BlendMode::Additive.to_blend_state();
        let _ = BlendMode::Multiply.to_blend_state();
        let _ = BlendMode::Screen.to_blend_state();
    }
}



