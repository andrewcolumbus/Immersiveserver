//! Output preset system for saving and restoring advanced output configurations
//!
//! Provides functionality to save and load complete output configurations including:
//! - Screen definitions with slices
//! - Warp mesh, edge blending, color correction
//! - Built-in preset configurations
//! - System-wide preset storage

use quick_xml::de::from_str;
use quick_xml::se::to_string;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use super::screen::Screen;
use super::slice::SliceId;
use super::ScreenId;

// ═══════════════════════════════════════════════════════════════════════════════
// OUTPUT PRESET — Complete saved output configuration
// ═══════════════════════════════════════════════════════════════════════════════

/// A complete saved output configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "ImmersiveOutputPreset")]
pub struct OutputPreset {
    /// Human-readable name for this preset
    pub name: String,

    /// Optional description
    #[serde(default)]
    pub description: Option<String>,

    /// All screens with their slices, transforms, warp, edge blend, color, masking
    #[serde(rename = "screens", default)]
    pub screens: Vec<Screen>,

    /// Whether this is a built-in preset (cannot be deleted/modified)
    #[serde(rename = "isBuiltin", default)]
    pub is_builtin: bool,
}

impl Default for OutputPreset {
    fn default() -> Self {
        Self {
            name: "Untitled".to_string(),
            description: None,
            screens: Vec::new(),
            is_builtin: false,
        }
    }
}

impl OutputPreset {
    /// Create a new output preset with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Create a preset from a set of screens
    pub fn from_screens(name: impl Into<String>, screens: Vec<Screen>) -> Self {
        Self {
            name: name.into(),
            screens,
            ..Default::default()
        }
    }

    /// Set description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Mark as built-in
    pub fn as_builtin(mut self) -> Self {
        self.is_builtin = true;
        self
    }

    /// Save to XML file
    pub fn save_to_file(&self, path: &PathBuf) -> Result<(), OutputPresetError> {
        let xml = to_string(self).map_err(OutputPresetError::XmlWrite)?;
        let formatted = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}", xml);
        fs::write(path, formatted).map_err(OutputPresetError::Io)?;
        Ok(())
    }

    /// Load from XML file
    pub fn load_from_file(path: &PathBuf) -> Result<Self, OutputPresetError> {
        let contents = fs::read_to_string(path).map_err(OutputPresetError::Io)?;
        from_str(&contents).map_err(OutputPresetError::XmlParse)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// OUTPUT PRESET REFERENCE — Reference stored in .immersive files
// ═══════════════════════════════════════════════════════════════════════════════

/// Reference to an output preset stored in .immersive files
///
/// Contains both the preset name (for UI display) and an embedded copy
/// of the screens (for portability to other systems).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OutputPresetReference {
    /// Name of the system preset (None = custom/no preset)
    #[serde(rename = "presetName", default, skip_serializing_if = "Option::is_none")]
    pub preset_name: Option<String>,

    /// Embedded copy of the screens for portability
    #[serde(rename = "embeddedScreens", default)]
    pub embedded_screens: Vec<Screen>,
}

impl OutputPresetReference {
    /// Create a new reference with just screens (no preset name)
    pub fn from_screens(screens: Vec<Screen>) -> Self {
        Self {
            preset_name: None,
            embedded_screens: screens,
        }
    }

    /// Create a new reference with a preset name and screens
    pub fn from_preset(name: impl Into<String>, screens: Vec<Screen>) -> Self {
        Self {
            preset_name: Some(name.into()),
            embedded_screens: screens,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// OUTPUT PRESET MANAGER — CRUD operations for presets
// ═══════════════════════════════════════════════════════════════════════════════

/// Manages output presets including built-in and user-created presets
pub struct OutputPresetManager {
    /// All available presets
    presets: Vec<OutputPreset>,
    /// Index of currently active preset (if any)
    active_preset_index: Option<usize>,
}

impl Default for OutputPresetManager {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputPresetManager {
    /// Create a new preset manager with built-in presets
    pub fn new() -> Self {
        let presets = vec![
            Self::create_single_screen_preset(),
            Self::create_dual_horizontal_preset(),
        ];

        Self {
            presets,
            active_preset_index: Some(0), // Single Screen is active by default
        }
    }

    /// Load user presets from the output_presets directory
    pub fn load_user_presets(&mut self) {
        let presets_dir = Self::get_presets_dir();
        if let Some(dir) = presets_dir {
            if dir.exists() {
                if let Ok(entries) = fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().map(|e| e == "xml").unwrap_or(false) {
                            if let Ok(preset) = OutputPreset::load_from_file(&path) {
                                // Don't add duplicates
                                if !self.presets.iter().any(|p| p.name == preset.name) {
                                    self.presets.push(preset);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Get the output presets directory path
    pub fn get_presets_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|mut p| {
            p.push("ImmersiveServer");
            p.push("output_presets");
            p
        })
    }

    /// Ensure the output presets directory exists
    pub fn ensure_presets_dir() -> Result<PathBuf, OutputPresetError> {
        let dir = Self::get_presets_dir().ok_or(OutputPresetError::NoConfigDir)?;
        fs::create_dir_all(&dir).map_err(OutputPresetError::Io)?;
        Ok(dir)
    }

    /// Get all presets
    pub fn presets(&self) -> &[OutputPreset] {
        &self.presets
    }

    /// Get preset by index
    pub fn get_preset(&self, index: usize) -> Option<&OutputPreset> {
        self.presets.get(index)
    }

    /// Get preset by name
    pub fn get_preset_by_name(&self, name: &str) -> Option<&OutputPreset> {
        self.presets.iter().find(|p| p.name == name)
    }

    /// Get the index of a preset by name
    pub fn get_preset_index_by_name(&self, name: &str) -> Option<usize> {
        self.presets.iter().position(|p| p.name == name)
    }

    /// Get the currently active preset
    pub fn active_preset(&self) -> Option<&OutputPreset> {
        self.active_preset_index.and_then(|i| self.presets.get(i))
    }

    /// Get the index of the active preset
    pub fn active_preset_index(&self) -> Option<usize> {
        self.active_preset_index
    }

    /// Get the name of the active preset
    pub fn active_preset_name(&self) -> Option<&str> {
        self.active_preset().map(|p| p.name.as_str())
    }

    /// Set the active preset by index
    pub fn set_active_preset(&mut self, index: usize) {
        if index < self.presets.len() {
            self.active_preset_index = Some(index);
        }
    }

    /// Set the active preset by name
    pub fn set_active_preset_by_name(&mut self, name: &str) -> bool {
        if let Some(index) = self.get_preset_index_by_name(name) {
            self.active_preset_index = Some(index);
            true
        } else {
            false
        }
    }

    /// Clear the active preset (no preset selected)
    pub fn clear_active_preset(&mut self) {
        self.active_preset_index = None;
    }

    /// Add a new preset
    pub fn add_preset(&mut self, preset: OutputPreset) {
        tracing::debug!("Adding preset '{}' to manager. Current count: {}", preset.name, self.presets.len());
        // Remove existing preset with same name (if not built-in)
        self.presets.retain(|p| p.is_builtin || p.name != preset.name);
        self.presets.push(preset);
        tracing::debug!("After add, preset count: {}", self.presets.len());
    }

    /// Save screens as a new preset
    pub fn save_as_preset(
        &mut self,
        name: impl Into<String>,
        screens: Vec<Screen>,
    ) -> Result<(), OutputPresetError> {
        let name = name.into();

        // Don't allow overwriting built-in presets
        if self.presets.iter().any(|p| p.is_builtin && p.name == name) {
            return Err(OutputPresetError::CannotOverwriteBuiltin);
        }

        let preset = OutputPreset::from_screens(&name, screens);

        // Save to file
        let dir = Self::ensure_presets_dir()?;
        let filename = Self::sanitize_filename(&name);
        let path = dir.join(format!("{}.xml", filename));
        preset.save_to_file(&path)?;

        // Add to manager
        self.add_preset(preset);

        // Set as active
        if let Some(index) = self.get_preset_index_by_name(&name) {
            self.active_preset_index = Some(index);
        }

        Ok(())
    }

    /// Delete a preset by name (cannot delete built-in presets)
    pub fn delete_preset(&mut self, name: &str) -> Result<(), OutputPresetError> {
        // Find the preset
        let index = self
            .presets
            .iter()
            .position(|p| p.name == name)
            .ok_or(OutputPresetError::NotFound)?;

        // Check if it's built-in
        if self.presets[index].is_builtin {
            return Err(OutputPresetError::CannotDeleteBuiltin);
        }

        // Remove from disk
        let dir = Self::get_presets_dir().ok_or(OutputPresetError::NoConfigDir)?;
        let filename = Self::sanitize_filename(name);
        let path = dir.join(format!("{}.xml", filename));
        if path.exists() {
            fs::remove_file(&path).map_err(OutputPresetError::Io)?;
        }

        // Remove from manager
        self.presets.remove(index);

        // Update active index if needed
        if let Some(active) = self.active_preset_index {
            if active == index {
                self.active_preset_index = None;
            } else if active > index {
                self.active_preset_index = Some(active - 1);
            }
        }

        Ok(())
    }

    /// Get built-in presets
    pub fn builtin_presets(&self) -> impl Iterator<Item = (usize, &OutputPreset)> {
        self.presets
            .iter()
            .enumerate()
            .filter(|(_, p)| p.is_builtin)
    }

    /// Get user presets (non-builtin)
    pub fn user_presets(&self) -> impl Iterator<Item = (usize, &OutputPreset)> {
        self.presets
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.is_builtin)
    }

    /// Sanitize a name for use as a filename
    pub fn sanitize_filename(name: &str) -> String {
        name.chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Built-in Presets
    // ═══════════════════════════════════════════════════════════════════════════

    /// Create the "Single Screen" built-in preset
    fn create_single_screen_preset() -> OutputPreset {
        use super::slice::Slice;

        let mut screen = Screen::new(ScreenId(1), "Screen 1");
        screen.width = 1920;
        screen.height = 1080;
        screen.slices.push(Slice::new_full_composition(SliceId(1), "Slice 1"));

        OutputPreset {
            name: "Single Screen".to_string(),
            description: Some("Single 1920x1080 virtual screen with full composition".to_string()),
            screens: vec![screen],
            is_builtin: true,
        }
    }

    /// Create the "Dual Horizontal" built-in preset
    fn create_dual_horizontal_preset() -> OutputPreset {
        use super::edge_blend::{EdgeBlendConfig, EdgeBlendRegion};
        use super::slice::{Rect, Slice, SliceOutput};

        // Left screen
        let mut left_screen = Screen::new(ScreenId(1), "Left");
        left_screen.width = 1920;
        left_screen.height = 1080;

        let mut left_slice = Slice::new_full_composition(SliceId(1), "Left Slice");
        // Input: left half of composition
        left_slice.input_rect = Rect {
            x: 0.0,
            y: 0.0,
            width: 0.5,
            height: 1.0,
        };
        // Output: full screen with right edge blend
        left_slice.output = SliceOutput {
            edge_blend: EdgeBlendConfig {
                right: EdgeBlendRegion {
                    enabled: true,
                    width: 0.1,
                    gamma: 2.2,
                    black_level: 0.0,
                },
                ..Default::default()
            },
            ..Default::default()
        };
        left_screen.slices.push(left_slice);

        // Right screen
        let mut right_screen = Screen::new(ScreenId(2), "Right");
        right_screen.width = 1920;
        right_screen.height = 1080;

        let mut right_slice = Slice::new_full_composition(SliceId(2), "Right Slice");
        // Input: right half of composition
        right_slice.input_rect = Rect {
            x: 0.5,
            y: 0.0,
            width: 0.5,
            height: 1.0,
        };
        // Output: full screen with left edge blend
        right_slice.output = SliceOutput {
            edge_blend: EdgeBlendConfig {
                left: EdgeBlendRegion {
                    enabled: true,
                    width: 0.1,
                    gamma: 2.2,
                    black_level: 0.0,
                },
                ..Default::default()
            },
            ..Default::default()
        };
        right_screen.slices.push(right_slice);

        OutputPreset {
            name: "Dual Horizontal".to_string(),
            description: Some(
                "Two 1920x1080 screens side by side with edge blending".to_string(),
            ),
            screens: vec![left_screen, right_screen],
            is_builtin: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ERROR TYPES
// ═══════════════════════════════════════════════════════════════════════════════

/// Errors that can occur during output preset operations
#[derive(Debug)]
pub enum OutputPresetError {
    Io(std::io::Error),
    XmlParse(quick_xml::DeError),
    XmlWrite(quick_xml::SeError),
    NoConfigDir,
    NotFound,
    CannotDeleteBuiltin,
    CannotOverwriteBuiltin,
}

impl std::fmt::Display for OutputPresetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputPresetError::Io(e) => write!(f, "IO error: {}", e),
            OutputPresetError::XmlParse(e) => write!(f, "XML parse error: {}", e),
            OutputPresetError::XmlWrite(e) => write!(f, "XML write error: {}", e),
            OutputPresetError::NoConfigDir => write!(f, "Could not find config directory"),
            OutputPresetError::NotFound => write!(f, "Preset not found"),
            OutputPresetError::CannotDeleteBuiltin => {
                write!(f, "Cannot delete built-in presets")
            }
            OutputPresetError::CannotOverwriteBuiltin => {
                write!(f, "Cannot overwrite built-in presets")
            }
        }
    }
}

impl std::error::Error for OutputPresetError {}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_manager_creation() {
        let manager = OutputPresetManager::new();
        assert_eq!(manager.presets().len(), 2);

        // Check built-in presets exist
        assert!(manager.get_preset_by_name("Single Screen").is_some());
        assert!(manager.get_preset_by_name("Dual Horizontal").is_some());

        // All should be builtin
        for preset in manager.presets() {
            assert!(preset.is_builtin);
        }
    }

    #[test]
    fn test_preset_default_active() {
        let manager = OutputPresetManager::new();
        assert_eq!(manager.active_preset_index(), Some(0));
        assert_eq!(manager.active_preset().unwrap().name, "Single Screen");
    }

    #[test]
    fn test_add_user_preset() {
        let mut manager = OutputPresetManager::new();
        let preset = OutputPreset::new("My Custom Output");
        manager.add_preset(preset);

        assert_eq!(manager.presets().len(), 3);
        assert!(manager.get_preset_by_name("My Custom Output").is_some());
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(
            OutputPresetManager::sanitize_filename("My Preset"),
            "My_Preset"
        );
        assert_eq!(
            OutputPresetManager::sanitize_filename("Test/Preset:v1"),
            "Test_Preset_v1"
        );
        assert_eq!(
            OutputPresetManager::sanitize_filename("valid-name_123"),
            "valid-name_123"
        );
    }

    #[test]
    fn test_builtin_preset_properties() {
        let manager = OutputPresetManager::new();
        let single_screen = manager.get_preset_by_name("Single Screen").unwrap();

        assert!(single_screen.is_builtin);
        assert!(single_screen.description.is_some());
        assert_eq!(single_screen.screens.len(), 1);
        assert_eq!(single_screen.screens[0].width, 1920);
        assert_eq!(single_screen.screens[0].height, 1080);
    }

    #[test]
    fn test_dual_horizontal_preset() {
        let manager = OutputPresetManager::new();
        let dual = manager.get_preset_by_name("Dual Horizontal").unwrap();

        assert!(dual.is_builtin);
        assert_eq!(dual.screens.len(), 2);

        // Check edge blend is configured
        let left_slice = &dual.screens[0].slices[0];
        assert!(left_slice.output.edge_blend.right.enabled);

        let right_slice = &dual.screens[1].slices[0];
        assert!(right_slice.output.edge_blend.left.enabled);
    }

    #[test]
    fn test_output_preset_reference() {
        let screens = vec![Screen::default()];
        let reference = OutputPresetReference::from_preset("Test Preset", screens.clone());

        assert_eq!(reference.preset_name, Some("Test Preset".to_string()));
        assert_eq!(reference.embedded_screens.len(), 1);
    }
}
