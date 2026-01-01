//! UI module for Immersive Server
//!
//! Contains egui-based menu bar and panels.

pub mod clip_grid_panel;
pub mod dock;
pub mod effects_browser_panel;
pub mod file_browser_panel;
pub mod menu_bar;
pub mod preview_monitor_panel;
pub mod properties_panel;
pub mod sources_panel;
pub mod thumbnail_cache;

pub use clip_grid_panel::{ClipGridAction, ClipGridPanel};
pub use dock::{DockManager, DockZone, DockablePanel};
pub use effects_browser_panel::{DraggableEffect, EffectsBrowserAction, EffectsBrowserPanel, DRAG_EFFECT_PAYLOAD};
pub use file_browser_panel::{FileBrowserAction, FileBrowserPanel};
pub use menu_bar::MenuBar;
pub use preview_monitor_panel::{PreviewClipInfo, PreviewMonitorAction, PreviewMonitorPanel};
pub use properties_panel::{PropertiesPanel, PropertiesTab};
pub use sources_panel::{DraggableSource, SourcesAction, SourcesPanel};
pub use thumbnail_cache::ThumbnailCache;

