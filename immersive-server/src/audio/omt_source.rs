//! OMT audio source for FFT analysis
//!
//! Extracts audio from OMT receiver streams and provides it to the FFT analyzer.

use super::source::{AudioSource, AudioSourceState, BaseAudioSource};
use super::types::{AudioBuffer, AudioSourceId};
use std::sync::Arc;

/// OMT audio source - extracts audio from OMT receiver
pub struct OmtAudioSource {
    base: BaseAudioSource,
    source_address: String,
}

impl OmtAudioSource {
    /// Create a new OMT audio source
    pub fn new(address: &str) -> Self {
        Self {
            base: BaseAudioSource::new(AudioSourceId::Omt(address.to_string())),
            source_address: address.to_string(),
        }
    }

    /// Get the shared audio state (for passing to OMT receiver thread)
    pub fn state(&self) -> Arc<AudioSourceState> {
        self.base.state()
    }

    /// Get the source address
    pub fn source_address(&self) -> &str {
        &self.source_address
    }
}

impl AudioSource for OmtAudioSource {
    fn id(&self) -> &AudioSourceId {
        &self.base.id
    }

    fn display_name(&self) -> String {
        format!("OMT: {}", self.source_address)
    }

    fn sample_rate(&self) -> u32 {
        self.base.sample_rate()
    }

    fn channels(&self) -> u32 {
        self.base.channels()
    }

    fn is_active(&self) -> bool {
        self.base.is_active()
    }

    fn take_samples(&self) -> Option<AudioBuffer> {
        self.base.take_samples()
    }

    fn start(&self) -> Result<(), String> {
        self.base.start()
    }

    fn stop(&self) {
        self.base.stop()
    }
}

/// Push audio data from OMT frame to the audio source state
///
/// This function is called by the OMT receiver thread when audio frames are received.
/// OMT audio is already interleaved float32, so we just push it directly.
pub fn push_omt_audio_to_state(
    state: &Arc<AudioSourceState>,
    interleaved_data: &[f32],
    sample_rate: u32,
    channels: u32,
) {
    if !state.is_running() || interleaved_data.is_empty() {
        return;
    }

    // Push to ring buffer
    if let Ok(mut buffer) = state.buffer.lock() {
        buffer.set_format(sample_rate, channels);
        buffer.write(interleaved_data);
    }
    state.set_active(true);
}
