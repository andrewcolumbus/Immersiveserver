//! UI module for Immersive Server
//!
//! Contains egui-based menu bar and panels, plus native OS menu bar support.

pub mod advanced_output_window;
pub mod clip_grid_panel;
pub mod dock;
pub mod icons;
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
pub mod tiled_layout;
pub mod viewport_widget;
pub mod widgets;
pub mod window_registry;

pub use advanced_output_window::{AdvancedOutputAction, AdvancedOutputWindow};
pub use clip_grid_panel::{ClipGridAction, ClipGridPanel};
pub use dock::{DockAction, DockManager, DockZone, DockablePanel};
pub use effects_browser_panel::{DraggableEffect, EffectsBrowserAction, EffectsBrowserPanel, DRAG_EFFECT_PAYLOAD};
pub use tiled_layout::{
    CellDragState, ComputedLayout, DividerDragState, DividerInfo, DropTarget, PanelDragState,
    SplitDirection, TabbedCell, TileNode, TiledLayout,
};
pub use file_browser_panel::{FileBrowserAction, FileBrowserPanel};
pub use layout_preset::{LayoutPreset, LayoutPresetManager};
pub use menu_bar::MenuBar;
pub use native_menu::{activate_macos_app, focus_window_on_click, is_native_menu_supported, NativeMenu, NativeMenuEvent};
pub use performance_panel::PerformancePanel;
pub use preferences_window::PreferencesWindow;
pub use previs_panel::{PrevisAction, PrevisPanel, WallId};
pub use preview_monitor_panel::{PreviewClipInfo, PreviewLayerInfo, PreviewMode, PreviewMonitorAction, PreviewMonitorPanel, PreviewSourceInfo};
pub use properties_panel::{PropertiesPanel, PropertiesAction, PropertiesTab};
pub use sources_panel::{DraggableSource, SourcesAction, SourcesPanel};
pub use thumbnail_cache::ThumbnailCache;
pub use viewport_widget::{ViewportConfig, ViewportResponse, UvRenderInfo, handle_viewport_input, compute_uv_rect, compute_uv_and_dest_rect, draw_zoom_indicator};
pub use widgets::{
    // Resettable widget builder
    Resettable,
    // Slider/DragValue reset helpers
    slider_with_reset, slider_with_reset_suffix,
    drag_value_with_reset, drag_value_with_reset_speed, drag_value_with_reset_suffix,
    drag_value_i32_with_reset, drag_value_u32_with_reset, drag_value_with_reset_range_suffix,
    add_reset_on_right_click, add_reset_f32, add_reset_i32, add_reset_u32,
    // Texture registration helpers
    register_egui_texture, register_egui_texture_ptr, free_egui_texture,
    // Texture rendering helpers
    draw_texture, draw_texture_uv, draw_texture_aspect_fit,
    draw_texture_placeholder, draw_texture_or_placeholder, FULL_UV,
};

// ============================================================================
// Panel Boilerplate Macro
// ============================================================================

/// Generates boilerplate implementations for UI panels.
///
/// This macro generates:
/// - `impl Default` that delegates to `Self::new()`
/// - A basic `toggle()` method that flips the `open` field
///
/// # Usage
/// ```ignore
/// pub struct MyPanel {
///     pub open: bool,
///     // ... other fields
/// }
///
/// impl MyPanel {
///     pub fn new() -> Self { ... }
/// }
///
/// impl_panel_default!(MyPanel);
/// ```
///
/// If your panel needs custom toggle logic (e.g., refreshing state when opened),
/// implement `toggle()` manually instead of using this macro.
#[macro_export]
macro_rules! impl_panel_default {
    ($panel:ty) => {
        impl Default for $panel {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

pub use window_registry::{WindowEntry, WindowRegistry, WindowType};

// Re-export scrubber widget from external egui-widgets crate
pub use egui_widgets::{format_time, video_scrubber, ScrubberAction, ScrubberState};

// ============================================================================
// Cross-Window Drag State
// ============================================================================

/// State for tracking drag-and-drop across different OS windows.
///
/// Since each undocked panel has its own egui::Context, egui's built-in
/// DragAndDrop system can't transfer payloads between windows. This struct
/// provides app-level state that persists across all windows.
#[derive(Default)]
pub struct CrossWindowDragState {
    /// The currently dragged effect (from Effects Browser)
    pub dragged_effect: Option<DraggableEffect>,
}

impl CrossWindowDragState {
    /// Create a new empty drag state
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the currently dragged effect
    pub fn set_dragged_effect(&mut self, effect: DraggableEffect) {
        self.dragged_effect = Some(effect);
    }

    /// Clear the drag state (call when mouse button is released)
    pub fn clear(&mut self) {
        self.dragged_effect = None;
    }

    /// Check if an effect is being dragged
    pub fn is_dragging_effect(&self) -> bool {
        self.dragged_effect.is_some()
    }

    /// Get the dragged effect if any
    pub fn get_dragged_effect(&self) -> Option<&DraggableEffect> {
        self.dragged_effect.as_ref()
    }

    /// Take the dragged effect (consumes it)
    pub fn take_dragged_effect(&mut self) -> Option<DraggableEffect> {
        self.dragged_effect.take()
    }
}

