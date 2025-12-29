//! UI module for immersive player
//!
//! Provides all UI components for the application.

pub mod advanced_output;
pub mod blend_panel;
pub mod clip_matrix;
pub mod layer_controls;
pub mod main_window;
pub mod output_editor;
pub mod preview_monitor;
pub mod screen_manager;
pub mod screen_tree;
pub mod warp_panel;
pub mod widgets;

pub use advanced_output::AdvancedOutputWindow;
pub use blend_panel::BlendPanel;
pub use clip_matrix::ClipMatrix;
pub use layer_controls::LayerControls;
pub use main_window::MainWindow;
pub use output_editor::OutputEditor;
pub use preview_monitor::PreviewMonitor;
pub use screen_tree::{ScreenTreePanel, TreeSelection};
pub use warp_panel::WarpPanel;
