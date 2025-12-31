//! Settings management for Immersive Server
//!
//! Handles loading/saving of .immersive XML files and application preferences.

use quick_xml::de::from_str;
use quick_xml::se::to_string;
use serde::{Deserialize, Deserializer, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::compositor::Layer;

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

    /// Whether to show FPS overlay
    #[serde(rename = "showFps")]
    pub show_fps: bool,

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

    /// Thumbnail display mode (Fit or Fill)
    #[serde(rename = "thumbnailMode", default)]
    pub thumbnail_mode: ThumbnailMode,
}

/// Default OMT capture FPS
fn default_omt_capture_fps() -> u32 {
    30
}

/// Default number of clip columns
fn default_clip_columns() -> usize {
    8
}

impl Default for EnvironmentSettings {
    fn default() -> Self {
        Self {
            target_fps: 60,
            show_fps: true,
            environment_width: 1920,
            environment_height: 1080,
            window_width: 1920,
            window_height: 1080,
            layers: Vec::new(),
            global_clip_count: default_clip_columns(),
            omt_broadcast_enabled: false,
            omt_capture_fps: default_omt_capture_fps(),
            thumbnail_mode: ThumbnailMode::default(),
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
    #[serde(rename = "lastOpenedFile")]
    pub last_opened_file: Option<String>,
    
    /// Last output folder used by HAP Converter
    #[serde(rename = "converterOutputDir", default)]
    pub converter_output_dir: Option<String>,
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

        let last_file = self
            .last_opened_file
            .as_deref()
            .unwrap_or("");
        
        let converter_dir = self
            .converter_output_dir
            .as_deref()
            .unwrap_or("");

        let formatted = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<ImmersivePreferences>
    <lastOpenedFile>{}</lastOpenedFile>
    <converterOutputDir>{}</converterOutputDir>
</ImmersivePreferences>
"#,
            last_file, converter_dir
        );

        fs::write(&path, formatted).map_err(SettingsError::Io)?;
        Ok(())
    }

    /// Set the last opened file and save
    pub fn set_last_opened(&mut self, path: &PathBuf) {
        self.last_opened_file = Some(path.to_string_lossy().to_string());
        if let Err(e) = self.save() {
            log::warn!("Failed to save preferences: {:?}", e);
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
            log::warn!("Failed to save preferences: {:?}", e);
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

