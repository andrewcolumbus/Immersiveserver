//! Audio input and FFT analysis module
//!
//! Provides audio capture from multiple sources and real-time frequency
//! band analysis for audio-reactive effects.

mod fft;
mod manager;
mod ndi_source;
mod omt_source;
mod source;
mod system_input;
mod types;

// Re-export public API
pub use fft::FftAnalyzer;
pub use manager::AudioManager;
pub use ndi_source::{push_ndi_audio_to_state, NdiAudioSource};
pub use omt_source::{push_omt_audio_to_state, OmtAudioSource};
pub use source::{AudioRingBuffer, AudioSource, AudioSourceState};
pub use system_input::SystemAudioInput;
pub use types::{AudioBand, AudioBuffer, AudioSourceId, FftData};
