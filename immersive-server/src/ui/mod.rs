//! UI module for Immersive Server
//!
//! Contains egui-based menu bar and panels, plus native OS menu bar support.

pub mod advanced_output_window;
pub mod clip_grid_panel;
pub mod dock;
pub mod effects_browser_panel;
pub mod file_browser_panel;
pub mod layout_preset;
pub mod menu_bar;
pub mod menu_definition;
pub mod native_menu;
pub mod performance_panel;
pub mod preferences_window;
pub mod previs_panel;
pub mod preview_monitor_panel;
pub mod properties_panel;
pub mod sources_panel;
pub mod thumbnail_cache;
pub mod window_registry;

pub use advanced_output_window::{AdvancedOutputAction, AdvancedOutputWindow};
pub use clip_grid_panel::{ClipGridAction, ClipGridPanel};
pub use dock::{DockAction, DockManager, DockZone, DockablePanel};
pub use effects_browser_panel::{DraggableEffect, EffectsBrowserAction, EffectsBrowserPanel, DRAG_EFFECT_PAYLOAD};
pub use file_browser_panel::{FileBrowserAction, FileBrowserPanel};
pub use layout_preset::{LayoutPreset, LayoutPresetManager};
pub use menu_bar::MenuBar;
pub use native_menu::{activate_macos_app, focus_window_on_click, is_native_menu_supported, NativeMenu, NativeMenuEvent};
pub use performance_panel::PerformancePanel;
pub use preferences_window::PreferencesWindow;
pub use previs_panel::{PrevisAction, PrevisPanel, WallId};
pub use preview_monitor_panel::{PreviewClipInfo, PreviewLayerInfo, PreviewMode, PreviewMonitorAction, PreviewMonitorPanel};
pub use properties_panel::{PropertiesPanel, PropertiesAction, PropertiesTab};
pub use sources_panel::{DraggableSource, SourcesAction, SourcesPanel};
pub use thumbnail_cache::ThumbnailCache;
pub use window_registry::{WindowEntry, WindowRegistry, WindowType};

// Re-export scrubber widget from external egui-widgets crate
pub use egui_widgets::{format_time, video_scrubber, ScrubberAction, ScrubberState};

