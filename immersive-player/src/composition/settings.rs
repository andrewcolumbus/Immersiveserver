//! Composition settings
//!
//! Defines the core settings for a composition including resolution, FPS, and bit depth.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Bit depth for the composition render target
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BitDepth {
    /// 8-bit per channel (32-bit RGBA)
    Bit8,
    /// 10-bit per channel (for HDR workflows)
    Bit10,
    /// 16-bit per channel (half-float)
    Bit16,
    /// 32-bit per channel (full float)
    Bit32,
}

impl Default for BitDepth {
    fn default() -> Self {
        Self::Bit8
    }
}

impl BitDepth {
    /// Get the wgpu texture format for this bit depth
    pub fn texture_format(&self) -> wgpu::TextureFormat {
        match self {
            BitDepth::Bit8 => wgpu::TextureFormat::Rgba8UnormSrgb,
            BitDepth::Bit10 => wgpu::TextureFormat::Rgb10a2Unorm,
            BitDepth::Bit16 => wgpu::TextureFormat::Rgba16Float,
            BitDepth::Bit32 => wgpu::TextureFormat::Rgba32Float,
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            BitDepth::Bit8 => "8-bit",
            BitDepth::Bit10 => "10-bit",
            BitDepth::Bit16 => "16-bit",
            BitDepth::Bit32 => "32-bit Float",
        }
    }

    /// Get all available bit depths
    pub fn all() -> &'static [BitDepth] {
        &[BitDepth::Bit8, BitDepth::Bit10, BitDepth::Bit16, BitDepth::Bit32]
    }
}

/// Settings for the composition canvas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionSettings {
    /// Canvas width in pixels
    pub width: u32,
    /// Canvas height in pixels
    pub height: u32,
    /// Frame rate in frames per second
    pub fps: f32,
    /// Bit depth for rendering
    pub bit_depth: BitDepth,
    /// Background color (RGBA, 0.0-1.0)
    pub background_color: [f32; 4],
}

impl Default for CompositionSettings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 60.0,
            bit_depth: BitDepth::Bit8,
            background_color: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

impl CompositionSettings {
    /// Create new settings with specified resolution
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            ..Default::default()
        }
    }

    /// Create 1080p settings
    pub fn hd() -> Self {
        Self::new(1920, 1080)
    }

    /// Create 4K settings
    pub fn uhd() -> Self {
        Self::new(3840, 2160)
    }

    /// Create 720p settings
    pub fn hd720() -> Self {
        Self::new(1280, 720)
    }

    /// Get aspect ratio
    pub fn aspect_ratio(&self) -> f32 {
        self.width as f32 / self.height as f32
    }

    /// Get total pixel count
    pub fn pixel_count(&self) -> u32 {
        self.width * self.height
    }

    /// Check if this is a standard resolution
    pub fn is_standard_resolution(&self) -> bool {
        matches!(
            (self.width, self.height),
            (1280, 720) | (1920, 1080) | (2560, 1440) | (3840, 2160) | (4096, 2160)
        )
    }
}

/// Common resolution presets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionPreset {
    HD720,
    HD1080,
    QHD,
    UHD4K,
    DCI4K,
    Custom(u32, u32),
}

impl ResolutionPreset {
    /// Get the resolution as (width, height)
    pub fn resolution(&self) -> (u32, u32) {
        match self {
            ResolutionPreset::HD720 => (1280, 720),
            ResolutionPreset::HD1080 => (1920, 1080),
            ResolutionPreset::QHD => (2560, 1440),
            ResolutionPreset::UHD4K => (3840, 2160),
            ResolutionPreset::DCI4K => (4096, 2160),
            ResolutionPreset::Custom(w, h) => (*w, *h),
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> String {
        match self {
            ResolutionPreset::HD720 => "720p".to_string(),
            ResolutionPreset::HD1080 => "1080p".to_string(),
            ResolutionPreset::QHD => "1440p".to_string(),
            ResolutionPreset::UHD4K => "4K UHD".to_string(),
            ResolutionPreset::DCI4K => "4K DCI".to_string(),
            ResolutionPreset::Custom(w, h) => format!("{}×{}", w, h),
        }
    }

    /// Get all standard presets
    pub fn standard_presets() -> &'static [ResolutionPreset] {
        &[
            ResolutionPreset::HD720,
            ResolutionPreset::HD1080,
            ResolutionPreset::QHD,
            ResolutionPreset::UHD4K,
            ResolutionPreset::DCI4K,
        ]
    }
}

/// Common FPS presets
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FpsPreset {
    Fps24,
    Fps25,
    Fps30,
    Fps50,
    Fps60,
    Fps120,
    Custom(f32),
}

impl FpsPreset {
    /// Get the FPS value
    pub fn value(&self) -> f32 {
        match self {
            FpsPreset::Fps24 => 24.0,
            FpsPreset::Fps25 => 25.0,
            FpsPreset::Fps30 => 30.0,
            FpsPreset::Fps50 => 50.0,
            FpsPreset::Fps60 => 60.0,
            FpsPreset::Fps120 => 120.0,
            FpsPreset::Custom(v) => *v,
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> String {
        match self {
            FpsPreset::Fps24 => "24 fps".to_string(),
            FpsPreset::Fps25 => "25 fps".to_string(),
            FpsPreset::Fps30 => "30 fps".to_string(),
            FpsPreset::Fps50 => "50 fps".to_string(),
            FpsPreset::Fps60 => "60 fps".to_string(),
            FpsPreset::Fps120 => "120 fps".to_string(),
            FpsPreset::Custom(v) => format!("{} fps", v),
        }
    }

    /// Get all standard presets
    pub fn standard_presets() -> &'static [FpsPreset] {
        &[
            FpsPreset::Fps24,
            FpsPreset::Fps25,
            FpsPreset::Fps30,
            FpsPreset::Fps50,
            FpsPreset::Fps60,
            FpsPreset::Fps120,
        ]
    }
}


