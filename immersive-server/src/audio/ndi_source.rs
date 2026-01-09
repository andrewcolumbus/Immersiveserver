//! NDI audio source for FFT analysis
//!
//! Extracts audio from NDI receiver streams and provides it to the FFT analyzer.

use super::source::{AudioSource, AudioSourceState, BaseAudioSource};
use super::types::{AudioBuffer, AudioSourceId};
use std::sync::Arc;

/// NDI audio source - extracts audio from NDI receiver
pub struct NdiAudioSource {
    base: BaseAudioSource,
    source_name: String,
}

impl NdiAudioSource {
    /// Create a new NDI audio source
    pub fn new(ndi_name: &str) -> Self {
        Self {
            base: BaseAudioSource::new(AudioSourceId::Ndi(ndi_name.to_string())),
            source_name: ndi_name.to_string(),
        }
    }

    /// Get the shared audio state (for passing to NDI receiver thread)
    pub fn state(&self) -> Arc<AudioSourceState> {
        self.base.state()
    }

    /// Get the source name
    pub fn source_name(&self) -> &str {
        &self.source_name
    }
}

impl AudioSource for NdiAudioSource {
    fn id(&self) -> &AudioSourceId {
        &self.base.id
    }

    fn display_name(&self) -> String {
        format!("NDI: {}", self.source_name)
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
