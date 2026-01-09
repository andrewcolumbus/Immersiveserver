//! Central audio manager coordinating all audio sources and FFT analysis

use super::fft::FftAnalyzer;
use super::source::{AudioSource, AudioSourceState};
use super::system_input::SystemAudioInput;
use super::types::{AudioBand, AudioSourceId, FftData};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Central manager for all audio sources and FFT analysis
pub struct AudioManager {
    /// Registered audio sources
    sources: HashMap<AudioSourceId, Box<dyn AudioSource>>,
    /// FFT analyzers per source
    analyzers: HashMap<AudioSourceId, FftAnalyzer>,
    /// Latest FFT data per source
    fft_data: Arc<RwLock<HashMap<AudioSourceId, FftData>>>,
    /// Currently selected primary source for automation
    primary_source: Option<AudioSourceId>,
    /// Last update time
    last_update: Instant,
    /// Master sensitivity (0.0 - 2.0)
    master_sensitivity: f32,
    /// Per-band sensitivity multipliers
    band_sensitivity: HashMap<AudioBand, f32>,
    /// Whether system audio is initialized
    system_audio_initialized: bool,
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioManager {
    /// Create a new audio manager
    pub fn new() -> Self {
        let mut band_sensitivity = HashMap::new();
        for band in AudioBand::all() {
            band_sensitivity.insert(*band, 1.0);
        }

        Self {
            sources: HashMap::new(),
            analyzers: HashMap::new(),
            fft_data: Arc::new(RwLock::new(HashMap::new())),
            primary_source: None,
            last_update: Instant::now(),
            master_sensitivity: 1.0,
            band_sensitivity,
            system_audio_initialized: false,
        }
    }

    /// Initialize system audio input (microphone/line-in)
    pub fn init_system_audio(&mut self) -> Result<(), String> {
        if self.system_audio_initialized {
            return Ok(());
        }

        match SystemAudioInput::new() {
            Ok(mut input) => {
                input.start_capture()?;
                let sample_rate = input.sample_rate();
                let id = input.id().clone();

                self.sources.insert(id.clone(), Box::new(input));
                self.analyzers
                    .insert(id.clone(), FftAnalyzer::new(sample_rate));

                // Set as primary source if none selected
                if self.primary_source.is_none() {
                    self.primary_source = Some(id);
                }

                self.system_audio_initialized = true;
                tracing::info!("System audio initialized");
                Ok(())
            }
            Err(e) => {
                tracing::warn!("Failed to initialize system audio: {}", e);
                Err(e)
            }
        }
    }

    /// Check if system audio is available
    pub fn has_system_audio(&self) -> bool {
        self.system_audio_initialized
    }

    /// Register an audio source
    pub fn add_source(&mut self, source: Box<dyn AudioSource>) {
        let id = source.id().clone();
        let sample_rate = source.sample_rate();

        self.sources.insert(id.clone(), source);
        self.analyzers.insert(id.clone(), FftAnalyzer::new(sample_rate));

        // Auto-select first source as primary
        if self.primary_source.is_none() {
            self.primary_source = Some(id);
        }
    }

    /// Remove an audio source
    pub fn remove_source(&mut self, id: &AudioSourceId) {
        if let Some(source) = self.sources.remove(id) {
            source.stop();
        }
        self.analyzers.remove(id);

        if let Ok(mut data) = self.fft_data.write() {
            data.remove(id);
        }

        if self.primary_source.as_ref() == Some(id) {
            self.primary_source = self.sources.keys().next().cloned();
        }

        if id == &AudioSourceId::SystemInput {
            self.system_audio_initialized = false;
        }
    }

    /// Set the primary audio source for automation
    pub fn set_primary_source(&mut self, id: AudioSourceId) {
        if self.sources.contains_key(&id) {
            self.primary_source = Some(id);
        }
    }

    /// Get the primary source ID
    pub fn primary_source(&self) -> Option<&AudioSourceId> {
        self.primary_source.as_ref()
    }

    /// Update all audio sources and FFT analysis (call once per frame)
    pub fn update(&mut self) {
        let now = Instant::now();
        let delta = now.duration_since(self.last_update);
        self.last_update = now;

        // Limit update rate (~60Hz max)
        if delta < Duration::from_millis(16) {
            return;
        }

        // Process each source
        let source_ids: Vec<_> = self.sources.keys().cloned().collect();
        for id in source_ids {
            let source = match self.sources.get(&id) {
                Some(s) => s,
                None => continue,
            };

            if !source.is_active() {
                continue;
            }

            // Get available samples
            if let Some(buffer) = source.take_samples() {
                // Get or create analyzer
                if let Some(analyzer) = self.analyzers.get_mut(&id) {
                    analyzer.set_sample_rate(buffer.sample_rate);
                    let fft = analyzer.analyze(&buffer);

                    // Store FFT data
                    if let Ok(mut data) = self.fft_data.write() {
                        data.insert(id.clone(), fft);
                    }
                }
            }
        }
    }

    /// Get FFT data for a specific source
    pub fn get_fft_data(&self, id: &AudioSourceId) -> Option<FftData> {
        self.fft_data.read().ok()?.get(id).cloned()
    }

    /// Get FFT data for the primary source
    pub fn get_primary_fft_data(&self) -> Option<FftData> {
        let id = self.primary_source.as_ref()?;
        self.get_fft_data(id)
    }

    /// Get a specific band value from primary source, with sensitivity applied
    pub fn get_band_value(&self, band: AudioBand) -> f32 {
        let fft = match self.get_primary_fft_data() {
            Some(f) => f,
            None => return 0.0,
        };

        let raw = fft.get_band(band);
        let band_sens = self.band_sensitivity.get(&band).copied().unwrap_or(1.0);

        (raw * self.master_sensitivity * band_sens).min(1.0)
    }

    /// Get a specific band value from a specific source
    pub fn get_band_value_from_source(&self, source_id: &AudioSourceId, band: AudioBand) -> f32 {
        let fft = match self.get_fft_data(source_id) {
            Some(f) => f,
            None => return 0.0,
        };

        let raw = fft.get_band(band);
        let band_sens = self.band_sensitivity.get(&band).copied().unwrap_or(1.0);

        (raw * self.master_sensitivity * band_sens).min(1.0)
    }

    /// Set master sensitivity
    pub fn set_master_sensitivity(&mut self, sensitivity: f32) {
        self.master_sensitivity = sensitivity.clamp(0.0, 2.0);
    }

    /// Get master sensitivity
    pub fn master_sensitivity(&self) -> f32 {
        self.master_sensitivity
    }

    /// Set per-band sensitivity
    pub fn set_band_sensitivity(&mut self, band: AudioBand, sensitivity: f32) {
        self.band_sensitivity.insert(band, sensitivity.clamp(0.0, 2.0));
    }

    /// Get per-band sensitivity
    pub fn band_sensitivity(&self, band: AudioBand) -> f32 {
        self.band_sensitivity.get(&band).copied().unwrap_or(1.0)
    }

    /// Get list of all source IDs
    pub fn source_ids(&self) -> Vec<AudioSourceId> {
        self.sources.keys().cloned().collect()
    }

    /// Get source display name
    pub fn get_source_name(&self, id: &AudioSourceId) -> Option<String> {
        self.sources.get(id).map(|s| s.display_name())
    }

    /// Check if a source exists
    pub fn has_source(&self, id: &AudioSourceId) -> bool {
        self.sources.contains_key(id)
    }

    /// Start all sources
    pub fn start_all(&self) {
        for source in self.sources.values() {
            let _ = source.start();
        }
    }

    /// Stop all sources
    pub fn stop_all(&self) {
        for source in self.sources.values() {
            source.stop();
        }
    }

    /// Get shared FFT data reference (for thread-safe access from automation)
    pub fn fft_data_ref(&self) -> Arc<RwLock<HashMap<AudioSourceId, FftData>>> {
        Arc::clone(&self.fft_data)
    }

    /// List available system audio devices
    pub fn list_audio_devices() -> Vec<String> {
        use std::time::Instant;
        let start = Instant::now();
        let result = SystemAudioInput::list_devices();
        tracing::debug!("[AUDIO] list_audio_devices() took {:?}", start.elapsed());
        result
    }

    /// Initialize system audio with a specific device (None = default device)
    pub fn init_system_audio_device(&mut self, device_name: Option<&str>) -> Result<(), String> {
        use std::time::Instant;
        let total_start = Instant::now();
        tracing::debug!("[AUDIO] init_system_audio_device() started, device={:?}", device_name);

        // Remove existing system audio if present
        let remove_start = Instant::now();
        self.remove_source(&AudioSourceId::SystemInput);
        tracing::debug!("[AUDIO] init_system_audio_device: remove_source() took {:?}", remove_start.elapsed());

        let with_device_start = Instant::now();
        match SystemAudioInput::with_device(device_name) {
            Ok(mut input) => {
                tracing::debug!("[AUDIO] init_system_audio_device: with_device() took {:?}", with_device_start.elapsed());

                let capture_start = Instant::now();
                input.start_capture()?;
                tracing::debug!("[AUDIO] init_system_audio_device: start_capture() took {:?}", capture_start.elapsed());

                let sample_rate = input.sample_rate();
                let id = input.id().clone();

                self.sources.insert(id.clone(), Box::new(input));
                self.analyzers
                    .insert(id.clone(), FftAnalyzer::new(sample_rate));

                // Set as primary source
                self.primary_source = Some(id);
                self.system_audio_initialized = true;

                tracing::debug!("[AUDIO] init_system_audio_device() total took {:?}", total_start.elapsed());
                tracing::info!(
                    "System audio initialized with device: {}",
                    device_name.unwrap_or("default")
                );
                Ok(())
            }
            Err(e) => {
                tracing::warn!("Failed to initialize system audio device: {}", e);
                Err(e)
            }
        }
    }

    /// Add an NDI audio source and return its state for passing to NdiReceiver
    pub fn add_ndi_source(&mut self, ndi_name: &str) -> Arc<AudioSourceState> {
        use super::ndi_source::NdiAudioSource;

        // Remove any existing NDI source with the same name
        let source_id = AudioSourceId::Ndi(ndi_name.to_string());
        self.remove_source(&source_id);

        let source = NdiAudioSource::new(ndi_name);
        let state = source.state();
        let sample_rate = source.sample_rate();
        let id = source.id().clone();

        let _ = source.start();
        self.sources.insert(id.clone(), Box::new(source));
        self.analyzers.insert(id.clone(), FftAnalyzer::new(sample_rate));

        // Set as primary source
        self.primary_source = Some(id);

        tracing::info!("Added NDI audio source: {}", ndi_name);
        state
    }

    /// Add an OMT audio source and return its state for passing to OmtReceiver
    pub fn add_omt_source(&mut self, address: &str) -> Arc<AudioSourceState> {
        use super::omt_source::OmtAudioSource;

        // Remove any existing OMT source with the same address
        let source_id = AudioSourceId::Omt(address.to_string());
        self.remove_source(&source_id);

        let source = OmtAudioSource::new(address);
        let state = source.state();
        let sample_rate = source.sample_rate();
        let id = source.id().clone();

        let _ = source.start();
        self.sources.insert(id.clone(), Box::new(source));
        self.analyzers.insert(id.clone(), FftAnalyzer::new(sample_rate));

        // Set as primary source
        self.primary_source = Some(id);

        tracing::info!("Added OMT audio source: {}", address);
        state
    }

    /// Get current audio level (0.0-1.0) for level meter display
    pub fn get_current_level(&self) -> f32 {
        self.get_primary_fft_data()
            .map(|fft| fft.full)
            .unwrap_or(0.0)
    }

    /// Get per-band levels for detailed meter (low, mid, high)
    pub fn get_band_levels(&self) -> (f32, f32, f32) {
        self.get_primary_fft_data()
            .map(|fft| (fft.low, fft.mid, fft.high))
            .unwrap_or((0.0, 0.0, 0.0))
    }

    /// Clear all audio sources
    pub fn clear_sources(&mut self) {
        use std::time::Instant;
        let total_start = Instant::now();
        tracing::debug!("[AUDIO] clear_sources() started, {} sources to clear", self.sources.len());

        // Stop all sources first
        let stop_start = Instant::now();
        self.stop_all();
        tracing::debug!("[AUDIO] clear_sources: stop_all() took {:?}", stop_start.elapsed());

        // Clear collections
        self.sources.clear();
        self.analyzers.clear();

        if let Ok(mut data) = self.fft_data.write() {
            data.clear();
        }

        self.primary_source = None;
        self.system_audio_initialized = false;

        tracing::debug!("[AUDIO] clear_sources() total took {:?}", total_start.elapsed());
        tracing::info!("Cleared all audio sources");
    }

    /// Get the default system audio device name
    pub fn default_device_name() -> Option<String> {
        SystemAudioInput::default_device_name()
    }
}
