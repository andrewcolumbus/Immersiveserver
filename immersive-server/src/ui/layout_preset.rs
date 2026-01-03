//! Layout preset system for saving and restoring UI arrangements
//!
//! Provides functionality to save and load complete UI layouts including:
//! - Panel positions and dock zones
//! - Tab group configurations
//! - Window geometry
//! - Built-in preset layouts

use quick_xml::de::from_str;
use quick_xml::se::to_string;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use super::dock::{
    DockManager, DockZone, DockablePanel, PanelGeometry, PanelPlacement, TabGroup,
    TabGroupPlacement,
};

// ═══════════════════════════════════════════════════════════════════════════════
// PANEL LAYOUT STATE — Saved state for a single panel
// ═══════════════════════════════════════════════════════════════════════════════

/// Saved state for a panel in a layout preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelLayoutState {
    /// Whether the panel is open/visible
    pub open: bool,
    /// Panel placement (Docked, Floating, Undocked, Tabbed)
    pub placement: PanelPlacement,
    /// Dock zone (for backward compatibility and when docked)
    pub zone: DockZone,
    /// Geometry when floating
    pub floating_geometry: PanelGeometry,
    /// Geometry when undocked
    pub undocked_geometry: PanelGeometry,
    /// Width when docked horizontally
    pub dock_width: f32,
    /// Height when docked vertically
    pub dock_height: f32,
}

impl Default for PanelLayoutState {
    fn default() -> Self {
        Self {
            open: true,
            placement: PanelPlacement::default(),
            zone: DockZone::Right,
            floating_geometry: PanelGeometry::default(),
            undocked_geometry: PanelGeometry::default(),
            dock_width: 300.0,
            dock_height: 200.0,
        }
    }
}

impl PanelLayoutState {
    /// Create from a DockablePanel
    pub fn from_panel(panel: &DockablePanel) -> Self {
        Self {
            open: panel.open,
            placement: panel.placement,
            zone: panel.zone,
            floating_geometry: panel.floating_geometry.clone(),
            undocked_geometry: panel.undocked_geometry.clone(),
            dock_width: panel.dock_width,
            dock_height: panel.dock_height,
        }
    }

    /// Apply this state to a DockablePanel
    pub fn apply_to_panel(&self, panel: &mut DockablePanel) {
        panel.open = self.open;
        panel.placement = self.placement;
        panel.zone = self.zone;
        panel.floating_geometry = self.floating_geometry.clone();
        panel.undocked_geometry = self.undocked_geometry.clone();
        panel.dock_width = self.dock_width;
        panel.dock_height = self.dock_height;

        // Sync legacy fields
        panel.floating_pos = Some(self.floating_geometry.position);
        panel.floating_size = Some(self.floating_geometry.size);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TAB GROUP LAYOUT STATE — Saved state for a tab group
// ═══════════════════════════════════════════════════════════════════════════════

/// Saved state for a tab group in a layout preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabGroupLayoutState {
    /// Panel IDs in this group (order = tab order)
    pub panel_ids: Vec<String>,
    /// Currently active tab index
    pub active_tab: usize,
    /// Where the tab group container lives
    pub placement: TabGroupPlacement,
    /// Geometry of the tab group container
    pub geometry: PanelGeometry,
}

impl TabGroupLayoutState {
    /// Create from a TabGroup
    pub fn from_tab_group(group: &TabGroup) -> Self {
        Self {
            panel_ids: group.panel_ids.clone(),
            active_tab: group.active_tab,
            placement: group.placement,
            geometry: group.geometry.clone(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// LAYOUT PRESET — Complete saved UI arrangement
// ═══════════════════════════════════════════════════════════════════════════════

/// A complete saved UI layout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "ImmersiveLayout")]
pub struct LayoutPreset {
    /// Human-readable name for this preset
    pub name: String,
    /// Optional description
    #[serde(default)]
    pub description: Option<String>,
    /// Panel states keyed by panel ID
    pub panels: HashMap<String, PanelLayoutState>,
    /// Tab group configurations
    #[serde(default)]
    pub tab_groups: Vec<TabGroupLayoutState>,
    /// Main window geometry (if saved)
    #[serde(default)]
    pub main_window_geometry: Option<PanelGeometry>,
    /// Whether this is a built-in preset (cannot be deleted/modified)
    #[serde(default)]
    pub is_builtin: bool,
}

impl Default for LayoutPreset {
    fn default() -> Self {
        Self {
            name: "Untitled".to_string(),
            description: None,
            panels: HashMap::new(),
            tab_groups: Vec::new(),
            main_window_geometry: None,
            is_builtin: false,
        }
    }
}

impl LayoutPreset {
    /// Create a new layout preset with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Create a preset from the current DockManager state
    pub fn from_dock_manager(name: impl Into<String>, dock_manager: &DockManager) -> Self {
        let mut panels = HashMap::new();

        // Capture all panel states
        for panel in dock_manager.all_panels() {
            panels.insert(panel.id.clone(), PanelLayoutState::from_panel(panel));
        }

        // Capture tab group states
        let tab_groups = dock_manager
            .tab_groups()
            .iter()
            .map(TabGroupLayoutState::from_tab_group)
            .collect();

        Self {
            name: name.into(),
            description: None,
            panels,
            tab_groups,
            main_window_geometry: None,
            is_builtin: false,
        }
    }

    /// Apply this preset to a DockManager
    /// Note: This only updates existing panels, doesn't create new ones
    pub fn apply_to_dock_manager(&self, dock_manager: &mut DockManager) {
        // First, dissolve all existing tab groups
        let group_ids: Vec<u32> = dock_manager.tab_groups().iter().map(|g| g.id).collect();
        for group_id in group_ids {
            dock_manager.dissolve_tab_group(group_id);
        }

        // Apply panel states
        for (panel_id, state) in &self.panels {
            if let Some(panel) = dock_manager.get_panel_mut(panel_id) {
                state.apply_to_panel(panel);
            }
        }

        // Recreate tab groups from the preset
        for tab_group_state in &self.tab_groups {
            if tab_group_state.panel_ids.len() >= 2 {
                // Create the tab group
                if let Some(group_id) = dock_manager.create_tab_group_at(
                    tab_group_state.panel_ids.clone(),
                    tab_group_state.geometry.position,
                    tab_group_state.geometry.size,
                ) {
                    // Set the active tab
                    dock_manager.set_active_tab(group_id, tab_group_state.active_tab);

                    // Set placement
                    if let Some(group) = dock_manager.get_tab_group_mut(group_id) {
                        group.placement = tab_group_state.placement;
                    }
                }
            }
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
    pub fn save_to_file(&self, path: &PathBuf) -> Result<(), LayoutPresetError> {
        let xml = to_string(self).map_err(LayoutPresetError::XmlWrite)?;
        let formatted = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}", xml);
        fs::write(path, formatted).map_err(LayoutPresetError::Io)?;
        Ok(())
    }

    /// Load from XML file
    pub fn load_from_file(path: &PathBuf) -> Result<Self, LayoutPresetError> {
        let contents = fs::read_to_string(path).map_err(LayoutPresetError::Io)?;
        from_str(&contents).map_err(LayoutPresetError::XmlParse)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// LAYOUT PRESET MANAGER — CRUD operations for presets
// ═══════════════════════════════════════════════════════════════════════════════

/// Manages layout presets including built-in and user-created presets
pub struct LayoutPresetManager {
    /// All available presets
    presets: Vec<LayoutPreset>,
    /// Index of currently active preset (if any)
    active_preset_index: Option<usize>,
}

impl Default for LayoutPresetManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutPresetManager {
    /// Create a new preset manager with built-in presets
    pub fn new() -> Self {
        let presets = vec![
            Self::create_default_preset(),
            Self::create_minimal_preset(),
            Self::create_multi_monitor_preset(),
        ];

        Self {
            presets,
            active_preset_index: Some(0), // Default preset is active
        }
    }

    /// Load user presets from the layouts directory
    pub fn load_user_presets(&mut self) {
        let layouts_dir = Self::get_layouts_dir();
        if let Some(dir) = layouts_dir {
            if dir.exists() {
                if let Ok(entries) = fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().map(|e| e == "xml").unwrap_or(false) {
                            if let Ok(preset) = LayoutPreset::load_from_file(&path) {
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

    /// Get the layouts directory path
    pub fn get_layouts_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|mut p| {
            p.push("ImmersiveServer");
            p.push("layouts");
            p
        })
    }

    /// Ensure the layouts directory exists
    pub fn ensure_layouts_dir() -> Result<PathBuf, LayoutPresetError> {
        let dir = Self::get_layouts_dir().ok_or(LayoutPresetError::NoConfigDir)?;
        fs::create_dir_all(&dir).map_err(LayoutPresetError::Io)?;
        Ok(dir)
    }

    /// Get all presets
    pub fn presets(&self) -> &[LayoutPreset] {
        &self.presets
    }

    /// Get preset by index
    pub fn get_preset(&self, index: usize) -> Option<&LayoutPreset> {
        self.presets.get(index)
    }

    /// Get preset by name
    pub fn get_preset_by_name(&self, name: &str) -> Option<&LayoutPreset> {
        self.presets.iter().find(|p| p.name == name)
    }

    /// Get the currently active preset
    pub fn active_preset(&self) -> Option<&LayoutPreset> {
        self.active_preset_index.and_then(|i| self.presets.get(i))
    }

    /// Get the index of the active preset
    pub fn active_preset_index(&self) -> Option<usize> {
        self.active_preset_index
    }

    /// Set the active preset by index
    pub fn set_active_preset(&mut self, index: usize) {
        if index < self.presets.len() {
            self.active_preset_index = Some(index);
        }
    }

    /// Add a new preset
    pub fn add_preset(&mut self, preset: LayoutPreset) {
        // Remove existing preset with same name (if not built-in)
        self.presets.retain(|p| p.is_builtin || p.name != preset.name);
        self.presets.push(preset);
    }

    /// Save a new preset from the current dock manager state
    pub fn save_current_as_preset(
        &mut self,
        name: impl Into<String>,
        dock_manager: &DockManager,
    ) -> Result<(), LayoutPresetError> {
        let name = name.into();
        let preset = LayoutPreset::from_dock_manager(&name, dock_manager);

        // Save to file
        let dir = Self::ensure_layouts_dir()?;
        let filename = Self::sanitize_filename(&name);
        let path = dir.join(format!("{}.xml", filename));
        preset.save_to_file(&path)?;

        // Add to manager
        self.add_preset(preset);
        Ok(())
    }

    /// Delete a preset by name (cannot delete built-in presets)
    pub fn delete_preset(&mut self, name: &str) -> Result<(), LayoutPresetError> {
        // Find the preset
        let index = self
            .presets
            .iter()
            .position(|p| p.name == name)
            .ok_or(LayoutPresetError::NotFound)?;

        // Check if it's built-in
        if self.presets[index].is_builtin {
            return Err(LayoutPresetError::CannotDeleteBuiltin);
        }

        // Remove from disk
        let dir = Self::get_layouts_dir().ok_or(LayoutPresetError::NoConfigDir)?;
        let filename = Self::sanitize_filename(name);
        let path = dir.join(format!("{}.xml", filename));
        if path.exists() {
            fs::remove_file(&path).map_err(LayoutPresetError::Io)?;
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

    /// Apply a preset by index
    pub fn apply_preset(&mut self, index: usize, dock_manager: &mut DockManager) -> bool {
        if let Some(preset) = self.presets.get(index) {
            preset.apply_to_dock_manager(dock_manager);
            self.active_preset_index = Some(index);
            true
        } else {
            false
        }
    }

    /// Apply a preset by name
    pub fn apply_preset_by_name(&mut self, name: &str, dock_manager: &mut DockManager) -> bool {
        if let Some(index) = self.presets.iter().position(|p| p.name == name) {
            self.apply_preset(index, dock_manager)
        } else {
            false
        }
    }

    /// Get built-in presets (first 3)
    pub fn builtin_presets(&self) -> impl Iterator<Item = (usize, &LayoutPreset)> {
        self.presets
            .iter()
            .enumerate()
            .filter(|(_, p)| p.is_builtin)
    }

    /// Get user presets (non-builtin)
    pub fn user_presets(&self) -> impl Iterator<Item = (usize, &LayoutPreset)> {
        self.presets
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.is_builtin)
    }

    /// Sanitize a name for use as a filename
    fn sanitize_filename(name: &str) -> String {
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

    /// Create the "Default" built-in preset
    fn create_default_preset() -> LayoutPreset {
        use super::dock::panel_ids::*;

        let mut panels = HashMap::new();

        // Properties panel - docked right
        panels.insert(
            PROPERTIES.to_string(),
            PanelLayoutState {
                open: true,
                placement: PanelPlacement::Docked(DockZone::Right),
                zone: DockZone::Right,
                dock_width: 320.0,
                ..Default::default()
            },
        );

        // Clip Grid - docked right
        panels.insert(
            CLIP_GRID.to_string(),
            PanelLayoutState {
                open: true,
                placement: PanelPlacement::Docked(DockZone::Right),
                zone: DockZone::Right,
                dock_width: 320.0,
                ..Default::default()
            },
        );

        // Sources panel - docked left
        panels.insert(
            SOURCES.to_string(),
            PanelLayoutState {
                open: true,
                placement: PanelPlacement::Docked(DockZone::Left),
                zone: DockZone::Left,
                dock_width: 280.0,
                ..Default::default()
            },
        );

        // Files panel - docked left
        panels.insert(
            FILES.to_string(),
            PanelLayoutState {
                open: true,
                placement: PanelPlacement::Docked(DockZone::Left),
                zone: DockZone::Left,
                dock_width: 280.0,
                ..Default::default()
            },
        );

        // Effects browser - docked left
        panels.insert(
            EFFECTS_BROWSER.to_string(),
            PanelLayoutState {
                open: true,
                placement: PanelPlacement::Docked(DockZone::Left),
                zone: DockZone::Left,
                dock_width: 280.0,
                ..Default::default()
            },
        );

        // Preview Monitor - floating
        panels.insert(
            PREVIEW_MONITOR.to_string(),
            PanelLayoutState {
                open: false,
                placement: PanelPlacement::Floating,
                zone: DockZone::Floating,
                floating_geometry: PanelGeometry::new((100.0, 100.0), (400.0, 300.0)),
                ..Default::default()
            },
        );

        // Performance panel - closed
        panels.insert(
            PERFORMANCE.to_string(),
            PanelLayoutState {
                open: false,
                placement: PanelPlacement::Floating,
                zone: DockZone::Floating,
                floating_geometry: PanelGeometry::new((100.0, 100.0), (300.0, 200.0)),
                ..Default::default()
            },
        );

        // Previs panel - closed
        panels.insert(
            PREVIS.to_string(),
            PanelLayoutState {
                open: false,
                placement: PanelPlacement::Floating,
                zone: DockZone::Floating,
                floating_geometry: PanelGeometry::new((100.0, 100.0), (400.0, 400.0)),
                ..Default::default()
            },
        );

        LayoutPreset {
            name: "Default".to_string(),
            description: Some("Standard layout with panels docked to sides".to_string()),
            panels,
            tab_groups: Vec::new(),
            main_window_geometry: None,
            is_builtin: true,
        }
    }

    /// Create the "Minimal" built-in preset
    fn create_minimal_preset() -> LayoutPreset {
        use super::dock::panel_ids::*;

        let mut panels = HashMap::new();

        // Only Properties panel open, docked right
        panels.insert(
            PROPERTIES.to_string(),
            PanelLayoutState {
                open: true,
                placement: PanelPlacement::Docked(DockZone::Right),
                zone: DockZone::Right,
                dock_width: 280.0,
                ..Default::default()
            },
        );

        // All other panels closed
        for panel_id in [
            CLIP_GRID,
            SOURCES,
            FILES,
            EFFECTS_BROWSER,
            PREVIEW_MONITOR,
            PERFORMANCE,
            PREVIS,
        ] {
            panels.insert(
                panel_id.to_string(),
                PanelLayoutState {
                    open: false,
                    ..Default::default()
                },
            );
        }

        LayoutPreset {
            name: "Minimal".to_string(),
            description: Some("Minimal interface with maximum viewport space".to_string()),
            panels,
            tab_groups: Vec::new(),
            main_window_geometry: None,
            is_builtin: true,
        }
    }

    /// Create the "Multi-Monitor" built-in preset
    fn create_multi_monitor_preset() -> LayoutPreset {
        use super::dock::panel_ids::*;

        let mut panels = HashMap::new();

        // Properties panel - docked right
        panels.insert(
            PROPERTIES.to_string(),
            PanelLayoutState {
                open: true,
                placement: PanelPlacement::Docked(DockZone::Right),
                zone: DockZone::Right,
                dock_width: 320.0,
                ..Default::default()
            },
        );

        // Clip Grid - docked right
        panels.insert(
            CLIP_GRID.to_string(),
            PanelLayoutState {
                open: true,
                placement: PanelPlacement::Docked(DockZone::Right),
                zone: DockZone::Right,
                dock_width: 320.0,
                ..Default::default()
            },
        );

        // Sources and Files - docked left
        panels.insert(
            SOURCES.to_string(),
            PanelLayoutState {
                open: true,
                placement: PanelPlacement::Docked(DockZone::Left),
                zone: DockZone::Left,
                dock_width: 280.0,
                ..Default::default()
            },
        );

        panels.insert(
            FILES.to_string(),
            PanelLayoutState {
                open: true,
                placement: PanelPlacement::Docked(DockZone::Left),
                zone: DockZone::Left,
                dock_width: 280.0,
                ..Default::default()
            },
        );

        // Effects browser - docked left
        panels.insert(
            EFFECTS_BROWSER.to_string(),
            PanelLayoutState {
                open: true,
                placement: PanelPlacement::Docked(DockZone::Left),
                zone: DockZone::Left,
                dock_width: 280.0,
                ..Default::default()
            },
        );

        // Preview Monitor - undocked (for second monitor)
        panels.insert(
            PREVIEW_MONITOR.to_string(),
            PanelLayoutState {
                open: true,
                placement: PanelPlacement::Undocked,
                zone: DockZone::Floating,
                undocked_geometry: PanelGeometry {
                    position: (1920.0, 0.0), // Positioned on second monitor
                    size: (1920.0, 1080.0),
                    min_size: (400.0, 300.0),
                    maximized: true,
                    monitor: Some(1),
                },
                ..Default::default()
            },
        );

        // Performance panel - open and floating
        panels.insert(
            PERFORMANCE.to_string(),
            PanelLayoutState {
                open: true,
                placement: PanelPlacement::Floating,
                zone: DockZone::Floating,
                floating_geometry: PanelGeometry::new((50.0, 50.0), (300.0, 200.0)),
                ..Default::default()
            },
        );

        // Previs panel - closed
        panels.insert(
            PREVIS.to_string(),
            PanelLayoutState {
                open: false,
                ..Default::default()
            },
        );

        LayoutPreset {
            name: "Multi-Monitor".to_string(),
            description: Some(
                "Optimized for multi-monitor setups with Preview on second display".to_string(),
            ),
            panels,
            tab_groups: Vec::new(),
            main_window_geometry: None,
            is_builtin: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ERROR TYPES
// ═══════════════════════════════════════════════════════════════════════════════

/// Errors that can occur during layout preset operations
#[derive(Debug)]
pub enum LayoutPresetError {
    Io(std::io::Error),
    XmlParse(quick_xml::DeError),
    XmlWrite(quick_xml::SeError),
    NoConfigDir,
    NotFound,
    CannotDeleteBuiltin,
}

impl std::fmt::Display for LayoutPresetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutPresetError::Io(e) => write!(f, "IO error: {}", e),
            LayoutPresetError::XmlParse(e) => write!(f, "XML parse error: {}", e),
            LayoutPresetError::XmlWrite(e) => write!(f, "XML write error: {}", e),
            LayoutPresetError::NoConfigDir => write!(f, "Could not find config directory"),
            LayoutPresetError::NotFound => write!(f, "Preset not found"),
            LayoutPresetError::CannotDeleteBuiltin => {
                write!(f, "Cannot delete built-in presets")
            }
        }
    }
}

impl std::error::Error for LayoutPresetError {}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_manager_creation() {
        let manager = LayoutPresetManager::new();
        assert_eq!(manager.presets().len(), 3);

        // Check built-in presets exist
        assert!(manager.get_preset_by_name("Default").is_some());
        assert!(manager.get_preset_by_name("Minimal").is_some());
        assert!(manager.get_preset_by_name("Multi-Monitor").is_some());

        // All should be builtin
        for preset in manager.presets() {
            assert!(preset.is_builtin);
        }
    }

    #[test]
    fn test_preset_default_active() {
        let manager = LayoutPresetManager::new();
        assert_eq!(manager.active_preset_index(), Some(0));
        assert_eq!(manager.active_preset().unwrap().name, "Default");
    }

    #[test]
    fn test_add_user_preset() {
        let mut manager = LayoutPresetManager::new();
        let preset = LayoutPreset::new("My Custom Layout");
        manager.add_preset(preset);

        assert_eq!(manager.presets().len(), 4);
        assert!(manager.get_preset_by_name("My Custom Layout").is_some());
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(
            LayoutPresetManager::sanitize_filename("My Layout"),
            "My_Layout"
        );
        assert_eq!(
            LayoutPresetManager::sanitize_filename("Test/Layout:v1"),
            "Test_Layout_v1"
        );
        assert_eq!(
            LayoutPresetManager::sanitize_filename("valid-name_123"),
            "valid-name_123"
        );
    }

    #[test]
    fn test_panel_layout_state_default() {
        let state = PanelLayoutState::default();
        assert!(state.open);
        assert!(matches!(state.placement, PanelPlacement::Docked(DockZone::Right)));
    }

    #[test]
    fn test_builtin_preset_properties() {
        let manager = LayoutPresetManager::new();
        let default_preset = manager.get_preset_by_name("Default").unwrap();

        assert!(default_preset.is_builtin);
        assert!(default_preset.description.is_some());
        assert!(!default_preset.panels.is_empty());
    }
}
