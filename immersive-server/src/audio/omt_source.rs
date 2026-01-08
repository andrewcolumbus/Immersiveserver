//! OMT audio source for FFT analysis
//!
//! Extracts audio from OMT receiver streams and provides it to the FFT analyzer.

use super::source::{AudioSource, AudioSourceState};
use super::types::{AudioBuffer, AudioSourceId};
use std::sync::Arc;

/// Buffer size in samples (~100ms at 48kHz stereo)
const BUFFER_SIZE: usize = 48000 * 2 / 10;

/// OMT audio source - extracts audio from OMT receiver
pub struct OmtAudioSource {
    id: AudioSourceId,
    state: Arc<AudioSourceState>,
    source_address: String,
}

impl OmtAudioSource {
    /// Create a new OMT audio source
    pub fn new(address: &str) -> Self {
        let state = Arc::new(AudioSourceState::new(BUFFER_SIZE, 48000, 2));
        Self {
            id: AudioSourceId::Omt(address.to_string()),
            state,
            source_address: address.to_string(),
        }
    }

    /// Get the shared audio state (for passing to OMT receiver thread)
    pub fn state(&self) -> Arc<AudioSourceState> {
        Arc::clone(&self.state)
    }

    /// Get the source address
    pub fn source_address(&self) -> &str {
        &self.source_address
    }
}

impl AudioSource for OmtAudioSource {
    fn id(&self) -> &AudioSourceId {
        &self.id
    }

    fn display_name(&self) -> String {
        format!("OMT: {}", self.source_address)
    }

    fn sample_rate(&self) -> u32 {
        self.state
            .buffer
            .lock()
            .map(|b| b.sample_rate())
            .unwrap_or(48000)
    }

    fn channels(&self) -> u32 {
        self.state
            .buffer
            .lock()
            .map(|b| b.channels())
            .unwrap_or(2)
    }

    fn is_active(&self) -> bool {
        self.state.is_active()
    }

    fn take_samples(&self) -> Option<AudioBuffer> {
        let mut guard = self.state.buffer.lock().ok()?;
        let available = guard.available();

        if available < 1024 {
            // Minimum samples for useful FFT
            return None;
        }

        let sample_rate = guard.sample_rate();
        let channels = guard.channels();

        let mut samples = Vec::new();
        guard.read(&mut samples);

        Some(AudioBuffer {
            samples,
            sample_rate,
            channels,
        })
    }

    fn start(&self) -> Result<(), String> {
        self.state.set_running(true);
        Ok(())
    }

    fn stop(&self) {
        self.state.set_running(false);
        self.state.set_active(false);
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
