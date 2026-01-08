//! NDI audio source for FFT analysis
//!
//! Extracts audio from NDI receiver streams and provides it to the FFT analyzer.

use super::source::{AudioSource, AudioSourceState};
use super::types::{AudioBuffer, AudioSourceId};
use std::sync::Arc;

/// Buffer size in samples (~100ms at 48kHz stereo)
const BUFFER_SIZE: usize = 48000 * 2 / 10;

/// NDI audio source - extracts audio from NDI receiver
pub struct NdiAudioSource {
    id: AudioSourceId,
    state: Arc<AudioSourceState>,
    source_name: String,
}

impl NdiAudioSource {
    /// Create a new NDI audio source
    pub fn new(ndi_name: &str) -> Self {
        let state = Arc::new(AudioSourceState::new(BUFFER_SIZE, 48000, 2));
        Self {
            id: AudioSourceId::Ndi(ndi_name.to_string()),
            state,
            source_name: ndi_name.to_string(),
        }
    }

    /// Get the shared audio state (for passing to NDI receiver thread)
    pub fn state(&self) -> Arc<AudioSourceState> {
        Arc::clone(&self.state)
    }

    /// Get the source name
    pub fn source_name(&self) -> &str {
        &self.source_name
    }
}

impl AudioSource for NdiAudioSource {
    fn id(&self) -> &AudioSourceId {
        &self.id
    }

    fn display_name(&self) -> String {
        format!("NDI: {}", self.source_name)
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

/// Push audio data from NDI frame to the audio source state
///
/// This function is called by the NDI receiver thread when audio frames are received.
/// It converts planar float32 NDI audio to interleaved format and pushes to the ring buffer.
pub fn push_ndi_audio_to_state(
    state: &Arc<AudioSourceState>,
    planar_data: *const f32,
    sample_rate: u32,
    channels: u32,
    samples_per_channel: usize,
    channel_stride_samples: usize,
) {
    if !state.is_running() || planar_data.is_null() || samples_per_channel == 0 {
        return;
    }

    // Convert planar to interleaved
    let mut interleaved = Vec::with_capacity(samples_per_channel * channels as usize);
    for i in 0..samples_per_channel {
        for ch in 0..channels as usize {
            let sample = unsafe { *planar_data.add(ch * channel_stride_samples + i) };
            interleaved.push(sample);
        }
    }

    // Push to ring buffer
    if let Ok(mut buffer) = state.buffer.lock() {
        buffer.set_format(sample_rate, channels);
        buffer.write(&interleaved);
    }
    state.set_active(true);
}
