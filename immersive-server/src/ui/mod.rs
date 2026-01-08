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
pub mod viewport_widget;
pub mod widgets;
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
pub use preview_monitor_panel::{PreviewClipInfo, PreviewLayerInfo, PreviewMode, PreviewMonitorAction, PreviewMonitorPanel, PreviewSourceInfo};
pub use properties_panel::{PropertiesPanel, PropertiesAction, PropertiesTab};
pub use sources_panel::{DraggableSource, SourcesAction, SourcesPanel};
pub use thumbnail_cache::ThumbnailCache;
pub use viewport_widget::{ViewportConfig, ViewportResponse, UvRenderInfo, handle_viewport_input, compute_uv_rect, compute_uv_and_dest_rect, draw_zoom_indicator};
pub use widgets::{
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
pub use window_registry::{WindowEntry, WindowRegistry, WindowType};

// Re-export scrubber widget from external egui-widgets crate
pub use egui_widgets::{format_time, video_scrubber, ScrubberAction, ScrubberState};

