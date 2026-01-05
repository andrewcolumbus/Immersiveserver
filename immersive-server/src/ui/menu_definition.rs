//! Declarative menu definition shared between native and egui menus
//!
//! This module defines the menu structure once, and both native_menu.rs and menu_bar.rs
//! use it to generate their respective menu implementations.
//!
//! ## Native vs egui Boundaries
//!
//! **Native Menus (LEFT side):**
//! - File menu: Open, Save, Save As, Exit (Windows)
//! - Edit menu: Preferences (Windows only)
//! - View menu: Panel toggles, Show FPS/BPM, Layout submenu
//! - Tools menu: HAP Converter, MIDI Mapping
//!
//! **egui Only (RIGHT side):**
//! - FPS display with frame time
//! - BPM display with beat indicator dots
//! - Tap tempo button, Resync button
//! - Status messages with fade animation
//!
//! Native menus can only contain text, checkboxes, and submenus. Interactive widgets
//! like DragValue, custom drawing, and animations must remain in egui.

use muda::accelerator::{Accelerator, Code, Modifiers};

use super::menu_bar::{FileAction, MenuAction};

/// Unique identifier for menu items that need state tracking or action mapping
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MenuItemId {
    // File menu
    Open,
    Save,
    SaveAs,
    Exit,

    // Edit/App menu
    Preferences,

    // View menu - panels
    PanelClipGrid,
    PanelProperties,
    PanelSources,
    PanelEffectsBrowser,
    PanelPreviewMonitor,
    PanelPerformance,
    PanelPrevis,

    // View menu - settings
    ShowFps,
    ShowBpm,

    // View menu - Layout submenu
    LayoutPreset(usize),
    LayoutSave,
    LayoutReset,

    // Tools menu
    HapConverter,

    // Windows
    AdvancedOutput,
}

impl MenuItemId {
    /// Convert to string ID for muda menu item identification
    pub fn to_string_id(&self) -> String {
        match self {
            Self::Open => "file_open".into(),
            Self::Save => "file_save".into(),
            Self::SaveAs => "file_save_as".into(),
            Self::Exit => "file_exit".into(),
            Self::Preferences => "app_preferences".into(),
            Self::PanelClipGrid => "view_clip_grid".into(),
            Self::PanelProperties => "view_properties".into(),
            Self::PanelSources => "view_sources".into(),
            Self::PanelEffectsBrowser => "view_effects_browser".into(),
            Self::PanelPreviewMonitor => "view_preview_monitor".into(),
            Self::PanelPerformance => "view_performance".into(),
            Self::PanelPrevis => "view_previs".into(),
            Self::ShowFps => "view_show_fps".into(),
            Self::ShowBpm => "view_show_bpm".into(),
            Self::LayoutPreset(i) => format!("layout_preset_{}", i),
            Self::LayoutSave => "layout_save".into(),
            Self::LayoutReset => "layout_reset".into(),
            Self::HapConverter => "tools_hap_converter".into(),
            Self::AdvancedOutput => "view_advanced_output".into(),
        }
    }

    /// Parse from string ID (for muda event handling)
    pub fn from_string_id(s: &str) -> Option<Self> {
        match s {
            "file_open" => Some(Self::Open),
            "file_save" => Some(Self::Save),
            "file_save_as" => Some(Self::SaveAs),
            "file_exit" => Some(Self::Exit),
            "app_preferences" => Some(Self::Preferences),
            "view_clip_grid" => Some(Self::PanelClipGrid),
            "view_properties" => Some(Self::PanelProperties),
            "view_sources" => Some(Self::PanelSources),
            "view_effects_browser" => Some(Self::PanelEffectsBrowser),
            "view_preview_monitor" => Some(Self::PanelPreviewMonitor),
            "view_performance" => Some(Self::PanelPerformance),
            "view_previs" => Some(Self::PanelPrevis),
            "view_show_fps" => Some(Self::ShowFps),
            "view_show_bpm" => Some(Self::ShowBpm),
            "layout_save" => Some(Self::LayoutSave),
            "layout_reset" => Some(Self::LayoutReset),
            "tools_hap_converter" => Some(Self::HapConverter),
            "view_advanced_output" => Some(Self::AdvancedOutput),
            _ if s.starts_with("layout_preset_") => s
                .strip_prefix("layout_preset_")
                .and_then(|n| n.parse().ok())
                .map(Self::LayoutPreset),
            _ => None,
        }
    }

    /// Map to panel ID string for panel toggle actions
    pub fn to_panel_id(&self) -> Option<&'static str> {
        match self {
            Self::PanelClipGrid => Some("clip_grid"),
            Self::PanelProperties => Some("properties"),
            Self::PanelSources => Some("sources"),
            Self::PanelEffectsBrowser => Some("effects_browser"),
            Self::PanelPreviewMonitor => Some("preview_monitor"),
            Self::PanelPerformance => Some("performance"),
            Self::PanelPrevis => Some("previs"),
            _ => None,
        }
    }

    /// Map to app action
    pub fn to_action(&self) -> MenuItemAction {
        match self {
            Self::Open => MenuItemAction::File(FileAction::Open),
            Self::Save => MenuItemAction::File(FileAction::Save),
            Self::SaveAs => MenuItemAction::File(FileAction::SaveAs),
            Self::Exit => MenuItemAction::Exit,
            Self::Preferences => MenuItemAction::Menu(MenuAction::OpenPreferences),
            Self::PanelClipGrid => MenuItemAction::Menu(MenuAction::TogglePanel {
                panel_id: "clip_grid".into(),
            }),
            Self::PanelProperties => MenuItemAction::Menu(MenuAction::TogglePanel {
                panel_id: "properties".into(),
            }),
            Self::PanelSources => MenuItemAction::Menu(MenuAction::TogglePanel {
                panel_id: "sources".into(),
            }),
            Self::PanelEffectsBrowser => MenuItemAction::Menu(MenuAction::TogglePanel {
                panel_id: "effects_browser".into(),
            }),
            Self::PanelPreviewMonitor => MenuItemAction::Menu(MenuAction::TogglePanel {
                panel_id: "preview_monitor".into(),
            }),
            Self::PanelPerformance => MenuItemAction::Menu(MenuAction::TogglePanel {
                panel_id: "performance".into(),
            }),
            Self::PanelPrevis => MenuItemAction::Menu(MenuAction::TogglePanel {
                panel_id: "previs".into(),
            }),
            Self::ShowFps => MenuItemAction::ToggleSetting(SettingId::ShowFps),
            Self::ShowBpm => MenuItemAction::ToggleSetting(SettingId::ShowBpm),
            Self::LayoutPreset(i) => MenuItemAction::Menu(MenuAction::ApplyLayoutPreset { index: *i }),
            Self::LayoutSave => MenuItemAction::Menu(MenuAction::SaveLayout),
            Self::LayoutReset => MenuItemAction::Menu(MenuAction::ResetLayout),
            Self::HapConverter => MenuItemAction::Menu(MenuAction::OpenHAPConverter),
            Self::AdvancedOutput => MenuItemAction::Menu(MenuAction::OpenAdvancedOutput),
        }
    }
}

/// Settings that can be toggled from the menu
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingId {
    ShowFps,
    ShowBpm,
}

/// Action resulting from a menu click
#[derive(Debug, Clone)]
pub enum MenuItemAction {
    File(FileAction),
    Menu(MenuAction),
    ToggleSetting(SettingId),
    Exit,
    None,
}

/// Keyboard shortcut definition
#[derive(Debug, Clone)]
pub struct Shortcut {
    /// Modifier keys (Ctrl/Cmd, Shift, Alt)
    pub modifiers: Modifiers,
    /// The key code
    pub code: Code,
}

impl Shortcut {
    pub const fn new(modifiers: Modifiers, code: Code) -> Self {
        Self { modifiers, code }
    }

    /// Convert to muda Accelerator
    pub fn to_accelerator(&self) -> Accelerator {
        Accelerator::new(Some(self.modifiers), self.code)
    }

    /// Format for egui display (e.g., "Cmd+S" or "Ctrl+S")
    pub fn to_display_string(&self) -> String {
        let mut parts = Vec::new();

        #[cfg(target_os = "macos")]
        {
            if self.modifiers.contains(Modifiers::META) {
                parts.push("\u{2318}"); // Command symbol
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            if self.modifiers.contains(Modifiers::CONTROL) || self.modifiers.contains(Modifiers::META)
            {
                parts.push("Ctrl");
            }
        }

        if self.modifiers.contains(Modifiers::SHIFT) {
            #[cfg(target_os = "macos")]
            parts.push("\u{21E7}"); // Shift symbol
            #[cfg(not(target_os = "macos"))]
            parts.push("Shift");
        }
        if self.modifiers.contains(Modifiers::ALT) {
            #[cfg(target_os = "macos")]
            parts.push("\u{2325}"); // Option symbol
            #[cfg(not(target_os = "macos"))]
            parts.push("Alt");
        }

        parts.push(self.key_name());

        #[cfg(target_os = "macos")]
        {
            parts.join("")
        }
        #[cfg(not(target_os = "macos"))]
        {
            parts.join("+")
        }
    }

    fn key_name(&self) -> &'static str {
        match self.code {
            Code::KeyO => "O",
            Code::KeyS => "S",
            Code::Comma => ",",
            Code::F4 => "F4",
            _ => "?",
        }
    }
}

/// A single menu item
#[derive(Debug, Clone)]
pub enum MenuItem {
    /// Regular clickable item
    Action {
        id: MenuItemId,
        label: String,
        shortcut: Option<Shortcut>,
        enabled: bool,
    },

    /// Checkbox item with state
    Check {
        id: MenuItemId,
        label: String,
        shortcut: Option<Shortcut>,
        /// Initial checked state (updated via sync_state)
        default_checked: bool,
    },

    /// Separator line
    Separator,

    /// Submenu containing nested items
    Submenu {
        label: String,
        items: Vec<MenuItem>,
    },

    /// Label-only item (non-clickable, for section headers)
    Label { text: String },

    /// Platform predefined items (About, Quit, etc.)
    Predefined(PredefinedItem),
}

/// Platform-provided menu items
#[derive(Debug, Clone)]
pub enum PredefinedItem {
    About,
    Quit,
}

/// A complete menu (e.g., "File", "View")
#[derive(Debug, Clone)]
pub struct MenuDefinition {
    pub label: String,
    pub items: Vec<MenuItem>,
}

/// The entire menu bar structure
#[derive(Debug, Clone)]
pub struct MenuBarDefinition {
    /// Platform-specific app menu (macOS only)
    pub app_menu: Option<MenuDefinition>,
    /// Standard menus (File, Edit, View, Tools)
    pub menus: Vec<MenuDefinition>,
    /// Windows-only Help menu
    pub help_menu: Option<MenuDefinition>,
}

impl MenuBarDefinition {
    /// Build the complete menu bar definition
    pub fn build() -> Self {
        Self {
            app_menu: Self::build_app_menu(),
            menus: vec![
                Self::build_file_menu(),
                Self::build_edit_menu(),
                Self::build_view_menu(),
                Self::build_tools_menu(),
            ],
            help_menu: Self::build_help_menu(),
        }
    }

    #[cfg(target_os = "macos")]
    fn build_app_menu() -> Option<MenuDefinition> {
        Some(MenuDefinition {
            label: "Immersive Server".into(),
            items: vec![
                MenuItem::Predefined(PredefinedItem::About),
                MenuItem::Separator,
                MenuItem::Action {
                    id: MenuItemId::Preferences,
                    label: "Preferences...".into(),
                    shortcut: Some(Shortcut::new(Modifiers::META, Code::Comma)),
                    enabled: true,
                },
                MenuItem::Separator,
                MenuItem::Predefined(PredefinedItem::Quit),
            ],
        })
    }

    #[cfg(not(target_os = "macos"))]
    fn build_app_menu() -> Option<MenuDefinition> {
        None
    }

    #[allow(unused_mut)] // mut needed for Windows build
    fn build_file_menu() -> MenuDefinition {
        let mut items = vec![
            MenuItem::Action {
                id: MenuItemId::Open,
                label: "Open Environment...".into(),
                shortcut: Some(Shortcut::new(Modifiers::META, Code::KeyO)),
                enabled: true,
            },
            MenuItem::Separator,
            MenuItem::Action {
                id: MenuItemId::Save,
                label: "Save".into(),
                shortcut: Some(Shortcut::new(Modifiers::META, Code::KeyS)),
                enabled: true,
            },
            MenuItem::Action {
                id: MenuItemId::SaveAs,
                label: "Save As...".into(),
                shortcut: Some(Shortcut::new(Modifiers::META | Modifiers::SHIFT, Code::KeyS)),
                enabled: true,
            },
        ];

        // Windows-only Exit item
        #[cfg(target_os = "windows")]
        {
            items.push(MenuItem::Separator);
            items.push(MenuItem::Action {
                id: MenuItemId::Exit,
                label: "Exit".into(),
                shortcut: Some(Shortcut::new(Modifiers::ALT, Code::F4)),
                enabled: true,
            });
        }

        MenuDefinition {
            label: "File".into(),
            items,
        }
    }

    fn build_edit_menu() -> MenuDefinition {
        // Windows: Preferences in Edit menu
        // macOS: Preferences in app menu (handled separately)
        #[cfg(target_os = "windows")]
        let items = vec![MenuItem::Action {
            id: MenuItemId::Preferences,
            label: "Preferences...".into(),
            shortcut: None,
            enabled: true,
        }];

        #[cfg(not(target_os = "windows"))]
        let items = vec![];

        MenuDefinition {
            label: "Edit".into(),
            items,
        }
    }

    fn build_view_menu() -> MenuDefinition {
        MenuDefinition {
            label: "View".into(),
            items: vec![
                // Panels section
                MenuItem::Label {
                    text: "Panels".into(),
                },
                MenuItem::Check {
                    id: MenuItemId::PanelClipGrid,
                    label: "Clip Grid Panel".into(),
                    shortcut: None,
                    default_checked: true,
                },
                MenuItem::Check {
                    id: MenuItemId::PanelProperties,
                    label: "Properties Panel".into(),
                    shortcut: None,
                    default_checked: true,
                },
                MenuItem::Check {
                    id: MenuItemId::PanelSources,
                    label: "Sources Panel".into(),
                    shortcut: None,
                    default_checked: true,
                },
                MenuItem::Check {
                    id: MenuItemId::PanelEffectsBrowser,
                    label: "Effects Browser Panel".into(),
                    shortcut: None,
                    default_checked: true,
                },
                MenuItem::Check {
                    id: MenuItemId::PanelPreviewMonitor,
                    label: "Preview Monitor Panel".into(),
                    shortcut: None,
                    default_checked: true,
                },
                MenuItem::Check {
                    id: MenuItemId::PanelPerformance,
                    label: "Performance Panel".into(),
                    shortcut: None,
                    default_checked: false,
                },
                MenuItem::Check {
                    id: MenuItemId::PanelPrevis,
                    label: "3D Preview Panel".into(),
                    shortcut: None,
                    default_checked: false,
                },
                MenuItem::Separator,
                // Windows section
                MenuItem::Action {
                    id: MenuItemId::AdvancedOutput,
                    label: "Advanced Output...".into(),
                    shortcut: None,
                    enabled: true,
                },
                MenuItem::Separator,
                // Settings section
                MenuItem::Check {
                    id: MenuItemId::ShowFps,
                    label: "Show FPS".into(),
                    shortcut: None,
                    default_checked: false,
                },
                MenuItem::Check {
                    id: MenuItemId::ShowBpm,
                    label: "Show BPM".into(),
                    shortcut: None,
                    default_checked: true,
                },
                MenuItem::Separator,
                // Layout submenu - dynamic content added at runtime
                MenuItem::Submenu {
                    label: "Layout".into(),
                    items: vec![
                        // Built-in presets will be injected dynamically
                        MenuItem::Separator,
                        MenuItem::Action {
                            id: MenuItemId::LayoutSave,
                            label: "Save Layout...".into(),
                            shortcut: None,
                            enabled: true,
                        },
                        MenuItem::Action {
                            id: MenuItemId::LayoutReset,
                            label: "Reset Layout".into(),
                            shortcut: None,
                            enabled: true,
                        },
                    ],
                },
            ],
        }
    }

    fn build_tools_menu() -> MenuDefinition {
        MenuDefinition {
            label: "Tools".into(),
            items: vec![
                MenuItem::Action {
                    id: MenuItemId::HapConverter,
                    label: "HAP Converter...".into(),
                    shortcut: None,
                    enabled: true,
                },
            ],
        }
    }

    #[cfg(target_os = "windows")]
    fn build_help_menu() -> Option<MenuDefinition> {
        Some(MenuDefinition {
            label: "Help".into(),
            items: vec![MenuItem::Predefined(PredefinedItem::About)],
        })
    }

    #[cfg(not(target_os = "windows"))]
    fn build_help_menu() -> Option<MenuDefinition> {
        None
    }
}
