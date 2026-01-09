//! System audio input via cpal

use super::source::{AudioSource, AudioSourceState};
use super::types::{AudioBuffer, AudioSourceId};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::Arc;
use std::sync::Mutex;

/// Buffer size in samples (~100ms at 48kHz stereo)
const BUFFER_SIZE: usize = 48000 * 2 / 10;

/// Wrapper for cpal::Stream that implements Send
/// Safety: Stream is only accessed from the main thread after initialization
struct StreamWrapper(cpal::Stream);

// cpal::Stream is not Send/Sync by default, but we only access it from main thread
// The audio callback uses Arc<AudioSourceState> which is thread-safe
unsafe impl Send for StreamWrapper {}
unsafe impl Sync for StreamWrapper {}

/// System audio input source using cpal
pub struct SystemAudioInput {
    id: AudioSourceId,
    state: Arc<AudioSourceState>,
    stream: Mutex<Option<StreamWrapper>>,
    device_name: String,
}

impl SystemAudioInput {
    /// Create system input using default input device
    pub fn new() -> Result<Self, String> {
        Self::with_device(None)
    }

    /// Create system input with specific device name (None = default)
    pub fn with_device(device_name: Option<&str>) -> Result<Self, String> {
        use std::time::Instant;
        let total_start = Instant::now();
        tracing::debug!("[AUDIO] with_device() started, device_name={:?}", device_name);

        let host_start = Instant::now();
        let host = cpal::default_host();
        tracing::debug!("[AUDIO] default_host() took {:?}", host_start.elapsed());

        let device_start = Instant::now();
        let device = if let Some(name) = device_name {
            let enum_start = Instant::now();
            let mut devices = host.input_devices()
                .map_err(|e| format!("Failed to enumerate devices: {}", e))?;
            tracing::debug!("[AUDIO] input_devices() enumeration took {:?}", enum_start.elapsed());

            let find_start = Instant::now();
            let found = devices.find(|d| d.name().map(|n| n == name).unwrap_or(false))
                .ok_or_else(|| format!("Device '{}' not found", name))?;
            tracing::debug!("[AUDIO] find device by name took {:?}", find_start.elapsed());
            found
        } else {
            let default_start = Instant::now();
            let default = host.default_input_device()
                .ok_or_else(|| "No default input device".to_string())?;
            tracing::debug!("[AUDIO] default_input_device() took {:?}", default_start.elapsed());
            default
        };
        tracing::debug!("[AUDIO] device lookup total took {:?}", device_start.elapsed());

        let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());

        // Get default config
        let config_start = Instant::now();
        let config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get input config: {}", e))?;
        tracing::debug!("[AUDIO] default_input_config() took {:?}", config_start.elapsed());

        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as u32;

        tracing::info!(
            "SystemAudioInput: {} @ {}Hz, {} channels",
            device_name,
            sample_rate,
            channels
        );

        let state = Arc::new(AudioSourceState::new(BUFFER_SIZE, sample_rate, channels));

        tracing::debug!("[AUDIO] with_device() total took {:?}", total_start.elapsed());

        Ok(Self {
            id: AudioSourceId::SystemInput,
            state,
            stream: Mutex::new(None),
            device_name,
        })
    }

    /// List available input devices
    pub fn list_devices() -> Vec<String> {
        use std::time::Instant;
        let start = Instant::now();
        tracing::debug!("[AUDIO] list_devices() started");

        let host_start = Instant::now();
        let host = cpal::default_host();
        tracing::debug!("[AUDIO] list_devices: default_host() took {:?}", host_start.elapsed());

        let enum_start = Instant::now();
        let result: Vec<String> = host.input_devices()
            .map(|devices| devices.filter_map(|d| d.name().ok()).collect())
            .unwrap_or_default();
        tracing::debug!("[AUDIO] list_devices: input_devices() took {:?}", enum_start.elapsed());
        tracing::debug!("[AUDIO] list_devices() total took {:?}, found {} devices", start.elapsed(), result.len());

        result
    }

    /// Get the default input device name
    pub fn default_device_name() -> Option<String> {
        let host = cpal::default_host();
        host.default_input_device()
            .and_then(|d| d.name().ok())
    }

    /// Build and start the audio stream
    fn build_stream(&mut self) -> Result<(), String> {
        use std::time::Instant;
        let total_start = Instant::now();
        tracing::debug!("[AUDIO] build_stream() started");

        let host_start = Instant::now();
        let host = cpal::default_host();
        tracing::debug!("[AUDIO] build_stream: default_host() took {:?}", host_start.elapsed());

        let device_start = Instant::now();
        let device = host
            .default_input_device()
            .ok_or_else(|| "No default input device".to_string())?;
        tracing::debug!("[AUDIO] build_stream: default_input_device() took {:?}", device_start.elapsed());

        let config_start = Instant::now();
        let config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get input config: {}", e))?;
        tracing::debug!("[AUDIO] build_stream: default_input_config() took {:?}", config_start.elapsed());

        let state = Arc::clone(&self.state);

        // Update state format
        {
            let mut buffer = state.buffer.lock().unwrap();
            buffer.set_format(config.sample_rate().0, config.channels() as u32);
        }

        let err_fn = |err| tracing::error!("Audio input error: {}", err);

        let build_start = Instant::now();
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                let state_clone = Arc::clone(&state);
                device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if state_clone.is_running() {
                            if let Ok(mut buffer) = state_clone.buffer.lock() {
                                buffer.write(data);
                            }
                        }
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::I16 => {
                let state_clone = Arc::clone(&state);
                device.build_input_stream(
                    &config.into(),
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        if state_clone.is_running() {
                            // Convert i16 to f32
                            let float_data: Vec<f32> = data
                                .iter()
                                .map(|&s| s as f32 / i16::MAX as f32)
                                .collect();
                            if let Ok(mut buffer) = state_clone.buffer.lock() {
                                buffer.write(&float_data);
                            }
                        }
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::U16 => {
                let state_clone = Arc::clone(&state);
                device.build_input_stream(
                    &config.into(),
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        if state_clone.is_running() {
                            // Convert u16 to f32
                            let float_data: Vec<f32> = data
                                .iter()
                                .map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0)
                                .collect();
                            if let Ok(mut buffer) = state_clone.buffer.lock() {
                                buffer.write(&float_data);
                            }
                        }
                    },
                    err_fn,
                    None,
                )
            }
            _ => {
                return Err(format!(
                    "Unsupported sample format: {:?}",
                    config.sample_format()
                ))
            }
        }
        .map_err(|e| format!("Failed to build stream: {}", e))?;
        tracing::debug!("[AUDIO] build_stream: build_input_stream() took {:?}", build_start.elapsed());

        *self.stream.lock().unwrap() = Some(StreamWrapper(stream));
        tracing::debug!("[AUDIO] build_stream() total took {:?}", total_start.elapsed());
        Ok(())
    }
}

impl AudioSource for SystemAudioInput {
    fn id(&self) -> &AudioSourceId {
        &self.id
    }

    fn display_name(&self) -> String {
        format!("System: {}", self.device_name)
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

        // FFT needs 2048 mono samples. For stereo input, we need 2048 * 2 = 4096 interleaved samples
        const FFT_MIN_SAMPLES: usize = 2048 * 2;
        if available < FFT_MIN_SAMPLES {
            // Still apply peak decay even without enough samples
            guard.decay_peak_level();
            return None;
        }

        let sample_rate = guard.sample_rate();
        let channels = guard.channels();

        let mut samples = Vec::new();
        guard.read(&mut samples);

        // Apply peak decay after reading (once per update frame)
        guard.decay_peak_level();

        Some(AudioBuffer {
            samples,
            sample_rate,
            channels,
        })
    }

    fn get_peak_level(&self) -> f32 {
        self.state
            .buffer
            .lock()
            .map(|mut b| b.take_peak_level())
            .unwrap_or(0.0)
    }

    fn start(&self) -> Result<(), String> {
        // Note: We need &mut self to build stream, but trait requires &self
        // This is handled by having build_stream called separately
        self.state.set_running(true);
        self.state.set_active(true);

        if let Ok(guard) = self.stream.lock() {
            if let Some(ref wrapper) = *guard {
                wrapper.0.play().map_err(|e| format!("Failed to start stream: {}", e))?;
            }
        }

        Ok(())
    }

    fn stop(&self) {
        self.state.set_running(false);
        self.state.set_active(false);

        if let Ok(guard) = self.stream.lock() {
            if let Some(ref wrapper) = *guard {
                let _ = wrapper.0.pause();
            }
        }
    }
}

impl SystemAudioInput {
    /// Initialize and start the audio capture
    pub fn start_capture(&mut self) -> Result<(), String> {
        use std::time::Instant;
        let total_start = Instant::now();
        tracing::debug!("[AUDIO] start_capture() started");

        let build_start = Instant::now();
        self.build_stream()?;
        tracing::debug!("[AUDIO] start_capture: build_stream() took {:?}", build_start.elapsed());

        self.state.set_running(true);
        self.state.set_active(true);

        let play_start = Instant::now();
        if let Ok(guard) = self.stream.lock() {
            if let Some(ref wrapper) = *guard {
                wrapper.0
                    .play()
                    .map_err(|e| format!("Failed to start stream: {}", e))?;
            }
        }
        tracing::debug!("[AUDIO] start_capture: play() took {:?}", play_start.elapsed());
        tracing::debug!("[AUDIO] start_capture() total took {:?}", total_start.elapsed());

        Ok(())
    }
}

impl Drop for SystemAudioInput {
    fn drop(&mut self) {
        self.stop();
    }
}
