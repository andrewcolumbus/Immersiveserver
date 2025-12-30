//! UI module for Immersive Server
//!
//! Contains egui-based menu bar and panels.

pub mod clip_grid_panel;
pub mod dock;
pub mod menu_bar;
pub mod properties_panel;

pub use clip_grid_panel::{ClipGridAction, ClipGridPanel};
pub use dock::{DockManager, DockZone, DockablePanel};
pub use menu_bar::MenuBar;
pub use properties_panel::{PropertiesPanel, PropertiesTab};

