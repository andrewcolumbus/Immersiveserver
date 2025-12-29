//! Settings management for Immersive Server
//!
//! Handles loading/saving of .immersive XML files and application preferences.

use quick_xml::de::from_str;
use quick_xml::se::to_string;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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

    /// Window width
    #[serde(rename = "windowWidth")]
    pub window_width: u32,

    /// Window height
    #[serde(rename = "windowHeight")]
    pub window_height: u32,
}

impl Default for EnvironmentSettings {
    fn default() -> Self {
        Self {
            target_fps: 60,
            show_fps: true,
            window_width: 1920,
            window_height: 1080,
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
        Ok(settings)
    }

    /// Save settings to an .immersive XML file
    pub fn save_to_file(&self, path: &PathBuf) -> Result<(), SettingsError> {
        // Validate that serialization works
        let _xml = to_string(self).map_err(SettingsError::XmlWrite)?;

        // Add XML declaration and format nicely
        let formatted = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<ImmersiveEnvironment>
    <targetFps>{}</targetFps>
    <showFps>{}</showFps>
    <windowWidth>{}</windowWidth>
    <windowHeight>{}</windowHeight>
</ImmersiveEnvironment>
"#,
            self.target_fps, self.show_fps, self.window_width, self.window_height
        );

        fs::write(path, formatted).map_err(SettingsError::Io)?;
        Ok(())
    }
}

/// Application preferences (stored in config directory)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename = "ImmersivePreferences")]
pub struct AppPreferences {
    /// Path to the last opened .immersive file
    #[serde(rename = "lastOpenedFile")]
    pub last_opened_file: Option<String>,
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

        let formatted = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<ImmersivePreferences>
    <lastOpenedFile>{}</lastOpenedFile>
</ImmersivePreferences>
"#,
            last_file
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

