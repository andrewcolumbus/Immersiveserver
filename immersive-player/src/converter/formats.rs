//! HAP format definitions and quality presets.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// HAP video format variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HapVariant {
    /// HAP - DXT1 compression, RGB only, smallest files
    #[default]
    Hap,
    /// HAP Alpha - DXT5 compression, RGBA with transparency
    HapAlpha,
    /// HAP Q - BC7 compression, highest quality, larger files
    HapQ,
}

impl HapVariant {
    /// Returns the FFmpeg codec format string.
    pub fn ffmpeg_format(&self) -> &'static str {
        match self {
            HapVariant::Hap => "hap",
            HapVariant::HapAlpha => "hap_alpha",
            HapVariant::HapQ => "hap_q",
        }
    }

    /// Returns a human-readable name.
    pub fn display_name(&self) -> &'static str {
        match self {
            HapVariant::Hap => "HAP",
            HapVariant::HapAlpha => "HAP Alpha",
            HapVariant::HapQ => "HAP Q",
        }
    }

    /// Returns a description of the variant.
    pub fn description(&self) -> &'static str {
        match self {
            HapVariant::Hap => "RGB video, smallest files (DXT1)",
            HapVariant::HapAlpha => "RGBA with transparency (DXT5)",
            HapVariant::HapQ => "Highest quality, larger files (BC7)",
        }
    }

    /// All available variants.
    pub fn all() -> &'static [HapVariant] {
        &[HapVariant::Hap, HapVariant::HapAlpha, HapVariant::HapQ]
    }
}

/// Quality/speed presets for encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum QualityPreset {
    /// Fastest encoding, potentially larger files
    Fast,
    /// Balanced speed and file size (default)
    #[default]
    Balanced,
    /// Best compression, slower encoding
    Quality,
}

impl QualityPreset {
    /// Returns the chunk size for this preset.
    /// Higher chunk sizes = better compression but slower.
    pub fn chunk_size(&self) -> u32 {
        match self {
            QualityPreset::Fast => 1,
            QualityPreset::Balanced => 4,
            QualityPreset::Quality => 16,
        }
    }

    /// Returns a human-readable name.
    pub fn display_name(&self) -> &'static str {
        match self {
            QualityPreset::Fast => "Fast",
            QualityPreset::Balanced => "Balanced",
            QualityPreset::Quality => "Quality",
        }
    }

    /// Returns a description of the preset.
    pub fn description(&self) -> &'static str {
        match self {
            QualityPreset::Fast => "Fastest encoding, larger files",
            QualityPreset::Balanced => "Good balance of speed and size",
            QualityPreset::Quality => "Best compression, slower encoding",
        }
    }

    /// All available presets.
    pub fn all() -> &'static [QualityPreset] {
        &[QualityPreset::Fast, QualityPreset::Balanced, QualityPreset::Quality]
    }
}

/// Supported input file extensions.
pub fn supported_input_extensions() -> &'static [&'static str] {
    &[
        "mp4", "mov", "avi", "mkv", "webm", "m4v", 
        "mxf", "prores", "dnxhd", "dxv"
    ]
}

/// Check if a file extension is supported for conversion.
pub fn is_supported_extension(ext: &str) -> bool {
    let ext_lower = ext.to_lowercase();
    supported_input_extensions().iter().any(|e| *e == ext_lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hap_variant_format() {
        assert_eq!(HapVariant::Hap.ffmpeg_format(), "hap");
        assert_eq!(HapVariant::HapAlpha.ffmpeg_format(), "hap_alpha");
        assert_eq!(HapVariant::HapQ.ffmpeg_format(), "hap_q");
    }

    #[test]
    fn test_quality_preset_chunk_size() {
        assert_eq!(QualityPreset::Fast.chunk_size(), 1);
        assert_eq!(QualityPreset::Balanced.chunk_size(), 4);
        assert_eq!(QualityPreset::Quality.chunk_size(), 16);
    }

    #[test]
    fn test_supported_extensions() {
        assert!(is_supported_extension("mp4"));
        assert!(is_supported_extension("MOV"));
        assert!(!is_supported_extension("txt"));
    }
}



