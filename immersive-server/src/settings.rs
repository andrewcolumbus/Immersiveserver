//! Settings management for Immersive Server
//!
//! Handles loading/saving of .immersive XML files and application preferences.

use quick_xml::de::from_str;
use quick_xml::se::to_string;
use serde::{Deserialize, Deserializer, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::compositor::Layer;
use crate::effects::EffectStack;
use crate::output::{OutputPresetReference, Screen, ScreenId, SliceId};
use crate::previs::PrevisSettings;
use crate::ui::tiled_layout::TiledLayout;

/// Thumbnail display mode for clip grid cells
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThumbnailMode {
    /// Fit entire frame in cell (may show letterboxing)
    #[default]
    Fit,
    /// Fill cell completely (crops to fit)
    Fill,
}

impl ThumbnailMode {
    /// Get display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            ThumbnailMode::Fit => "Fit (show whole frame)",
            ThumbnailMode::Fill => "Fill (crop to fit)",
        }
    }
}

/// Audio source type for FFT analysis
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "type", content = "value")]
pub enum AudioSourceType {
    /// No audio source (disabled)
    #[default]
    None,
    /// System default audio input device
    SystemDefault,
    /// Specific system audio device by name
    SystemDevice(String),
    /// NDI source by name
    Ndi(String),
    /// OMT source by address
    Omt(String),
}

impl AudioSourceType {
    /// Get display name for UI
    pub fn display_name(&self) -> String {
        match self {
            AudioSourceType::None => "Disabled".to_string(),
            AudioSourceType::SystemDefault => "System Default".to_string(),
            AudioSourceType::SystemDevice(name) => format!("Device: {}", name),
            AudioSourceType::Ndi(name) => format!("NDI: {}", name),
            AudioSourceType::Omt(addr) => format!("OMT: {}", addr),
        }
    }

    /// Check if audio is enabled
    pub fn is_enabled(&self) -> bool {
        !matches!(self, AudioSourceType::None)
    }
}

/// Deserialize a usize from a string, treating empty strings as the default value
fn deserialize_usize_or_default<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    if s.is_empty() {
        Ok(default_clip_columns())
    } else {
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// Environment settings stored in .immersive files
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "ImmersiveEnvironment")]
pub struct EnvironmentSettings {
    /// Target frame rate (24-240)
    #[serde(rename = "targetFps")]
    pub target_fps: u32,

    /// Whether VSYNC is enabled (syncs to display refresh rate)
    /// - true:  Use Fifo present mode, display controls timing
    /// - false: Use Immediate mode with manual FPS control
    #[serde(rename = "vsyncEnabled", default)]
    pub vsync_enabled: bool,

    /// Whether to show FPS overlay
    #[serde(rename = "showFps")]
    pub show_fps: bool,

    /// Whether to show BPM display in menu bar
    #[serde(rename = "showBpm", default = "default_show_bpm")]
    pub show_bpm: bool,

    /// Environment (composition canvas) width.
    ///
    /// If missing in older `.immersive` files, this will default to `window_width`.
    #[serde(rename = "environmentWidth", default)]
    pub environment_width: u32,

    /// Environment (composition canvas) height.
    ///
    /// If missing in older `.immersive` files, this will default to `window_height`.
    #[serde(rename = "environmentHeight", default)]
    pub environment_height: u32,

    /// Window width
    #[serde(rename = "windowWidth")]
    pub window_width: u32,

    /// Window height
    #[serde(rename = "windowHeight")]
    pub window_height: u32,

    /// Layers with their clip grids
    #[serde(rename = "layers", default)]
    pub layers: Vec<Layer>,

    /// Global clip count (number of columns in the clip grid)
    #[serde(rename = "clipColumns", default = "default_clip_columns", deserialize_with = "deserialize_usize_or_default")]
    pub global_clip_count: usize,

    /// Whether OMT broadcast is enabled
    #[serde(rename = "omtBroadcastEnabled", default)]
    pub omt_broadcast_enabled: bool,

    /// OMT capture frame rate (1-60, default 30)
    #[serde(rename = "omtCaptureFps", default = "default_omt_capture_fps")]
    pub omt_capture_fps: u32,

    /// Whether NDI broadcast is enabled
    #[serde(rename = "ndiBroadcastEnabled", default)]
    pub ndi_broadcast_enabled: bool,

    /// NDI capture frame rate (1-60, default 30)
    #[serde(rename = "ndiCaptureFps", default = "default_ndi_capture_fps")]
    pub ndi_capture_fps: u32,

    /// NDI receive buffer capacity (1-10 frames, default 3)
    /// Higher values absorb more timing jitter but add latency
    #[serde(rename = "ndiBufferCapacity", default = "default_ndi_buffer_capacity")]
    pub ndi_buffer_capacity: usize,

    /// Whether OMT source discovery is enabled (auto-discovers OMT sources on network)
    #[serde(rename = "omtDiscoveryEnabled", default = "default_discovery_enabled")]
    pub omt_discovery_enabled: bool,

    /// Whether NDI source discovery is enabled (auto-discovers NDI sources on network)
    #[serde(rename = "ndiDiscoveryEnabled", default = "default_discovery_enabled")]
    pub ndi_discovery_enabled: bool,

    /// Whether Syphon (macOS) / Spout (Windows) texture sharing is enabled
    #[serde(rename = "textureShareEnabled", default)]
    pub texture_share_enabled: bool,

    /// Whether the REST API server is enabled
    #[serde(rename = "apiServerEnabled", default = "default_api_enabled")]
    pub api_server_enabled: bool,

    /// REST API server port (default 8080)
    #[serde(rename = "apiPort", default = "default_api_port")]
    pub api_port: u16,

    /// Thumbnail display mode (Fit or Fill)
    #[serde(rename = "thumbnailMode", default)]
    pub thumbnail_mode: ThumbnailMode,

    /// Master effect stack (applied to entire composition)
    #[serde(rename = "effects", default)]
    pub effects: EffectStack,

    /// Advanced output screens (multi-display, projection mapping)
    #[serde(rename = "screens", default)]
    pub screens: Vec<Screen>,

    /// Output preset reference (name + embedded copy for portability)
    #[serde(rename = "outputPreset", default)]
    pub output_preset: Option<OutputPresetReference>,

    /// 3D previsualization settings
    #[serde(rename = "previsSettings", default)]
    pub previs_settings: PrevisSettings,

    // Performance mode settings
    /// Whether floor sync is enabled (triggering clips also triggers floor layer)
    #[serde(rename = "floorSyncEnabled", default)]
    pub floor_sync_enabled: bool,

    /// Which layer index to use as the floor layer (0 = first layer)
    #[serde(rename = "floorLayerIndex", default)]
    pub floor_layer_index: usize,

    /// Low latency mode: trades stability for reduced input lag
    /// - true:  1 frame in flight (~16ms less latency, may stutter under load)
    /// - false: 2 frames in flight (smoother, but ~16ms more latency)
    #[serde(rename = "lowLatencyMode", default)]
    pub low_latency_mode: bool,

    /// Whether test pattern mode is enabled (replaces composition with calibration pattern)
    #[serde(rename = "testPatternEnabled", default)]
    pub test_pattern_enabled: bool,

    /// BGRA pipeline mode: Use BGRA format throughout for reduced CPU overhead
    /// - true:  FFmpeg outputs BGRA, textures use Bgra8UnormSrgb (NDI/OMT native)
    /// - false: FFmpeg outputs RGBA (default, wider compatibility)
    /// Requires restart to take effect.
    #[serde(rename = "bgraPipelineEnabled", default)]
    pub bgra_pipeline_enabled: bool,

    /// Audio source for FFT analysis and audio-reactive effects
    #[serde(rename = "audioSource", default)]
    pub audio_source: AudioSourceType,

    /// FFT analysis gain multiplier (0.0 - 4.0, default 1.0)
    /// Higher values make the audio meter more sensitive
    #[serde(rename = "fftGain", default = "default_fft_gain")]
    pub fft_gain: f32,

    /// Tiled layout configuration (UI panel arrangement)
    /// Optional - if not present, uses app preferences or default layout
    #[serde(rename = "tiledLayout", default, skip_serializing_if = "Option::is_none")]
    pub tiled_layout: Option<TiledLayout>,
}

/// Default show BPM setting
fn default_show_bpm() -> bool {
    true
}

/// Default OMT capture FPS
fn default_omt_capture_fps() -> u32 {
    30
}

/// Default NDI capture FPS
fn default_ndi_capture_fps() -> u32 {
    30
}

/// Default NDI receive buffer capacity (frames)
fn default_ndi_buffer_capacity() -> usize {
    3
}

/// Default number of clip columns
fn default_clip_columns() -> usize {
    8
}

/// Default API server enabled state
fn default_api_enabled() -> bool {
    true
}

/// Default discovery enabled state (both OMT and NDI default to enabled)
fn default_discovery_enabled() -> bool {
    true
}

/// Default API server port
fn default_api_port() -> u16 {
    8080
}

/// Default FFT gain multiplier
fn default_fft_gain() -> f32 {
    1.0
}

impl Default for EnvironmentSettings {
    fn default() -> Self {
        Self {
            target_fps: 60,
            vsync_enabled: false,
            show_fps: true,
            show_bpm: true,
            environment_width: 1920,
            environment_height: 1080,
            window_width: 1920,
            window_height: 1080,
            layers: Vec::new(),
            global_clip_count: default_clip_columns(),
            omt_broadcast_enabled: false,
            omt_capture_fps: default_omt_capture_fps(),
            ndi_broadcast_enabled: false,
            ndi_capture_fps: default_ndi_capture_fps(),
            ndi_buffer_capacity: default_ndi_buffer_capacity(),
            omt_discovery_enabled: default_discovery_enabled(),
            ndi_discovery_enabled: default_discovery_enabled(),
            texture_share_enabled: false,
            api_server_enabled: default_api_enabled(),
            api_port: default_api_port(),
            thumbnail_mode: ThumbnailMode::default(),
            effects: EffectStack::new(),
            screens: vec![Screen::new_with_default_slice(ScreenId(1), "Screen 1", SliceId(1))],
            output_preset: None,
            previs_settings: PrevisSettings::default(),
            floor_sync_enabled: false,
            floor_layer_index: 0,
            low_latency_mode: false, // Default to stability (2 frames in flight)
            test_pattern_enabled: false,
            bgra_pipeline_enabled: false, // Default to RGBA for compatibility
            audio_source: AudioSourceType::default(),
            fft_gain: default_fft_gain(),
            tiled_layout: None,
        }
    }
}

impl EnvironmentSettings {
    /// Clamp FPS to valid range (24-240)
    pub fn clamp_fps(&mut self) {
        self.target_fps = self.target_fps.clamp(24, 240);
    }

    /// Load settings from an .immersive XML file
    pub fn load_from_file(path: &PathBuf) -> Result<Self, SettingsError> {
        let contents = fs::read_to_string(path).map_err(SettingsError::Io)?;
        let mut settings: Self = from_str(&contents).map_err(SettingsError::XmlParse)?;
        settings.clamp_fps();

        // Backwards compatibility: older files won't include environment resolution.
        if settings.environment_width == 0 {
            settings.environment_width = settings.window_width;
        }
        if settings.environment_height == 0 {
            settings.environment_height = settings.window_height;
        }

        // Ensure sane minimums.
        settings.window_width = settings.window_width.max(1);
        settings.window_height = settings.window_height.max(1);
        settings.environment_width = settings.environment_width.max(1);
        settings.environment_height = settings.environment_height.max(1);

        Ok(settings)
    }

    /// Save settings to an .immersive XML file
    pub fn save_to_file(&self, path: &PathBuf) -> Result<(), SettingsError> {
        // Serialize to XML
        let xml = to_string(self).map_err(SettingsError::XmlWrite)?;

        // Add XML declaration
        let formatted = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}", xml);

        fs::write(path, formatted).map_err(SettingsError::Io)?;
        Ok(())
    }

    /// Set layers from the current environment state
    pub fn set_layers(&mut self, layers: &[Layer]) {
        self.layers = layers.to_vec();
    }

    /// Get layers for restoring environment state
    pub fn get_layers(&self) -> &[Layer] {
        &self.layers
    }
}

/// Application preferences (stored in config directory)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename = "ImmersivePreferences")]
pub struct AppPreferences {
    /// Path to the last opened .immersive file
    #[serde(rename = "lastOpenedFile", default, skip_serializing_if = "Option::is_none")]
    pub last_opened_file: Option<String>,

    /// Last output folder used by HAP Converter
    #[serde(rename = "converterOutputDir", default, skip_serializing_if = "Option::is_none")]
    pub converter_output_dir: Option<String>,

    /// Whether tiled layout mode is enabled
    #[serde(rename = "useTiledLayout", default)]
    pub use_tiled_layout: bool,

    /// Saved tiled layout configuration
    #[serde(rename = "tiledLayout", default, skip_serializing_if = "Option::is_none")]
    pub tiled_layout: Option<TiledLayout>,
}

impl AppPreferences {
    /// Get the preferences file path
    fn get_prefs_path() -> Option<PathBuf> {
        dirs::config_dir().map(|mut p| {
            p.push("ImmersiveServer");
            p.push("preferences.xml");
            p
        })
    }

    /// Load preferences from config directory
    pub fn load() -> Self {
        let Some(path) = Self::get_prefs_path() else {
            return Self::default();
        };

        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(&path) {
            Ok(contents) => from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save preferences to config directory
    pub fn save(&self) -> Result<(), SettingsError> {
        let Some(path) = Self::get_prefs_path() else {
            return Err(SettingsError::NoConfigDir);
        };

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(SettingsError::Io)?;
        }

        // Use proper XML serialization to handle complex nested structures
        let xml = to_string(self).map_err(SettingsError::XmlWrite)?;
        let formatted = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}", xml);

        fs::write(&path, formatted).map_err(SettingsError::Io)?;
        Ok(())
    }

    /// Save the tiled layout to preferences
    pub fn save_tiled_layout(&mut self, layout: &TiledLayout, enabled: bool) {
        self.tiled_layout = Some(layout.clone());
        self.use_tiled_layout = enabled;
        if let Err(e) = self.save() {
            tracing::warn!("Failed to save tiled layout preferences: {:?}", e);
        }
    }

    /// Set the last opened file and save
    pub fn set_last_opened(&mut self, path: &PathBuf) {
        self.last_opened_file = Some(path.to_string_lossy().to_string());
        if let Err(e) = self.save() {
            tracing::warn!("Failed to save preferences: {:?}", e);
        }
    }

    /// Get the last opened file path if it exists
    pub fn get_last_opened(&self) -> Option<PathBuf> {
        self.last_opened_file.as_ref().map(PathBuf::from).filter(|p| p.exists())
    }
    
    /// Set the converter output directory and save
    pub fn set_converter_output_dir(&mut self, path: &PathBuf) {
        self.converter_output_dir = Some(path.to_string_lossy().to_string());
        if let Err(e) = self.save() {
            tracing::warn!("Failed to save preferences: {:?}", e);
        }
    }
    
    /// Get the converter output directory if it exists
    pub fn get_converter_output_dir(&self) -> Option<PathBuf> {
        self.converter_output_dir.as_ref().map(PathBuf::from).filter(|p| p.exists())
    }
}

/// Settings-related errors
#[derive(Debug)]
pub enum SettingsError {
    Io(std::io::Error),
    XmlParse(quick_xml::DeError),
    XmlWrite(quick_xml::SeError),
    NoConfigDir,
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettingsError::Io(e) => write!(f, "IO error: {}", e),
            SettingsError::XmlParse(e) => write!(f, "XML parse error: {}", e),
            SettingsError::XmlWrite(e) => write!(f, "XML write error: {}", e),
            SettingsError::NoConfigDir => write!(f, "Could not find config directory"),
        }
    }
}

impl std::error::Error for SettingsError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = EnvironmentSettings::default();
        assert_eq!(settings.target_fps, 60);
        assert!(settings.show_fps);
        assert_eq!(settings.environment_width, 1920);
        assert_eq!(settings.environment_height, 1080);
        assert!(settings.layers.is_empty());
    }

    #[test]
    fn test_fps_clamping() {
        let mut settings = EnvironmentSettings::default();
        settings.target_fps = 300;
        settings.clamp_fps();
        assert_eq!(settings.target_fps, 240);

        settings.target_fps = 10;
        settings.clamp_fps();
        assert_eq!(settings.target_fps, 24);
    }
}

