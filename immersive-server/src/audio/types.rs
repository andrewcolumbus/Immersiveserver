//! Audio types shared across the audio module

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Frequency bands for FFT analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum AudioBand {
    /// Sub-bass and bass (20-250 Hz)
    #[default]
    Low,
    /// Midrange (250-4000 Hz)
    Mid,
    /// Treble and high frequencies (4000-20000 Hz)
    High,
    /// Full spectrum (all frequencies combined)
    Full,
}

impl AudioBand {
    /// Get all bands for iteration
    pub fn all() -> &'static [AudioBand] {
        &[AudioBand::Low, AudioBand::Mid, AudioBand::High, AudioBand::Full]
    }

    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            AudioBand::Low => "Low",
            AudioBand::Mid => "Mid",
            AudioBand::High => "High",
            AudioBand::Full => "Full",
        }
    }

    /// Get display name with frequency range
    pub fn name_with_range(&self) -> &'static str {
        match self {
            AudioBand::Low => "Low (20-250 Hz)",
            AudioBand::Mid => "Mid (250-4k Hz)",
            AudioBand::High => "High (4k-20k Hz)",
            AudioBand::Full => "Full Spectrum",
        }
    }

    /// Get frequency range (min_hz, max_hz)
    pub fn frequency_range(&self) -> (f32, f32) {
        match self {
            AudioBand::Low => (20.0, 250.0),
            AudioBand::Mid => (250.0, 4000.0),
            AudioBand::High => (4000.0, 20000.0),
            AudioBand::Full => (20.0, 20000.0),
        }
    }
}

/// FFT analysis results for a single audio source
#[derive(Debug, Clone)]
pub struct FftData {
    /// Normalized amplitude for low band (0.0 - 1.0)
    pub low: f32,
    /// Normalized amplitude for mid band (0.0 - 1.0)
    pub mid: f32,
    /// Normalized amplitude for high band (0.0 - 1.0)
    pub high: f32,
    /// Combined/RMS level across all bands (0.0 - 1.0)
    pub full: f32,
    /// Timestamp of this analysis
    pub timestamp: Instant,
}

impl Default for FftData {
    fn default() -> Self {
        Self {
            low: 0.0,
            mid: 0.0,
            high: 0.0,
            full: 0.0,
            timestamp: Instant::now(),
        }
    }
}

impl FftData {
    /// Create new FftData with current timestamp
    pub fn new() -> Self {
        Self {
            low: 0.0,
            mid: 0.0,
            high: 0.0,
            full: 0.0,
            timestamp: Instant::now(),
        }
    }

    /// Get band value by enum
    pub fn get_band(&self, band: AudioBand) -> f32 {
        match band {
            AudioBand::Low => self.low,
            AudioBand::Mid => self.mid,
            AudioBand::High => self.high,
            AudioBand::Full => self.full,
        }
    }

    /// Set band value by enum
    pub fn set_band(&mut self, band: AudioBand, value: f32) {
        match band {
            AudioBand::Low => self.low = value,
            AudioBand::Mid => self.mid = value,
            AudioBand::High => self.high = value,
            AudioBand::Full => self.full = value,
        }
    }
}

/// Audio sample buffer for processing
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    /// Interleaved or mono f32 samples
    pub samples: Vec<f32>,
    /// Sample rate (e.g., 48000)
    pub sample_rate: u32,
    /// Number of channels (typically 1 after downmix)
    pub channels: u32,
}

impl AudioBuffer {
    /// Create empty buffer
    pub fn new(sample_rate: u32, channels: u32) -> Self {
        Self {
            samples: Vec::new(),
            sample_rate,
            channels,
        }
    }

    /// Create with capacity
    pub fn with_capacity(capacity: usize, sample_rate: u32, channels: u32) -> Self {
        Self {
            samples: Vec::with_capacity(capacity),
            sample_rate,
            channels,
        }
    }

    /// Downmix to mono if stereo/multi-channel
    pub fn to_mono(&self) -> Vec<f32> {
        if self.channels == 1 {
            return self.samples.clone();
        }

        let channels = self.channels as usize;
        self.samples
            .chunks(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    }

    /// Get duration in seconds
    pub fn duration_secs(&self) -> f32 {
        if self.sample_rate == 0 || self.channels == 0 {
            return 0.0;
        }
        self.samples.len() as f32 / (self.sample_rate * self.channels) as f32
    }
}

/// Identifier for different audio sources
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AudioSourceId {
    /// System audio input (mic/line-in)
    SystemInput,
    /// NDI source by name
    Ndi(String),
    /// OMT source by name
    Omt(String),
    /// Video layer audio by layer ID
    VideoLayer(u32),
    /// Video clip audio by layer ID and slot
    VideoClip { layer_id: u32, slot: usize },
}

impl std::fmt::Display for AudioSourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioSourceId::SystemInput => write!(f, "System Input"),
            AudioSourceId::Ndi(name) => write!(f, "NDI: {}", name),
            AudioSourceId::Omt(name) => write!(f, "OMT: {}", name),
            AudioSourceId::VideoLayer(id) => write!(f, "Layer {} Audio", id),
            AudioSourceId::VideoClip { layer_id, slot } => {
                write!(f, "Layer {} Clip {} Audio", layer_id, slot)
            }
        }
    }
}

impl Default for AudioSourceId {
    fn default() -> Self {
        AudioSourceId::SystemInput
    }
}
