//! Window registry for multi-window support
//!
//! Tracks and manages multiple native OS windows for undocked panels.
//! Each window has its own GPU context (surface, egui renderer) and
//! can render panel content independently.

use std::collections::HashMap;
use std::sync::Arc;
use winit::window::{Window, WindowId};

use crate::gpu_context::{GpuContext, WindowGpuContext};

// ═══════════════════════════════════════════════════════════════════════════════
// WINDOW TYPE — What kind of window is this
// ═══════════════════════════════════════════════════════════════════════════════

/// Type of window in the registry
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowType {
    /// The main application window
    Main,
    /// An undocked panel window
    Panel {
        /// The panel ID this window is hosting
        panel_id: String,
    },
    /// A fullscreen output monitor (for projection mapping)
    Monitor {
        /// The output/display index
        output_id: u32,
    },
}

// ═══════════════════════════════════════════════════════════════════════════════
// WINDOW ENTRY — Information about a registered window
// ═══════════════════════════════════════════════════════════════════════════════

/// Entry for a window in the registry
pub struct WindowEntry {
    /// The winit window handle
    pub window: Arc<Window>,
    /// What type of window this is
    pub window_type: WindowType,
    /// GPU context for this window (surface, egui renderer)
    /// Only present for windows that need rendering
    pub gpu_context: Option<WindowGpuContext>,
    /// egui context for this window (persistent across frames)
    pub egui_ctx: egui::Context,
    /// egui state for this window
    pub egui_state: egui_winit::State,
    /// Whether this window needs a redraw
    pub needs_redraw: bool,
    /// Whether the window has been closed (pending cleanup)
    pub closed: bool,
}

impl WindowEntry {
    /// Create a new window entry for the main window (without GPU context yet)
    pub fn new_main(window: Arc<Window>, egui_ctx: egui::Context) -> Self {
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        Self {
            window,
            window_type: WindowType::Main,
            gpu_context: None, // Main window GPU context is managed by App
            egui_ctx,
            egui_state,
            needs_redraw: true,
            closed: false,
        }
    }

    /// Create a new window entry for a panel window
    pub fn new_panel(
        window: Arc<Window>,
        panel_id: String,
        gpu: &GpuContext,
    ) -> Self {
        let gpu_context = WindowGpuContext::new(gpu, window.clone());

        // Create a new egui context for this panel window
        let egui_ctx = egui::Context::default();
        egui_ctx.set_pixels_per_point(window.scale_factor() as f32);

        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::from_hash_of(&panel_id),
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        Self {
            window,
            window_type: WindowType::Panel { panel_id },
            gpu_context: Some(gpu_context),
            egui_ctx,
            egui_state,
            needs_redraw: true,
            closed: false,
        }
    }

    /// Create a new window entry for a monitor output window (fullscreen on a display)
    ///
    /// Monitor windows are used for outputting to physical displays/projectors.
    /// They have a GPU context but minimal egui (for potential debug overlays).
    pub fn new_monitor(window: Arc<Window>, output_id: u32, gpu: &GpuContext) -> Self {
        let gpu_context = WindowGpuContext::new(gpu, window.clone());

        // Create egui context (minimal, for potential overlays)
        let egui_ctx = egui::Context::default();
        egui_ctx.set_pixels_per_point(window.scale_factor() as f32);

        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::from_hash_of(&format!("monitor_{}", output_id)),
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        Self {
            window,
            window_type: WindowType::Monitor { output_id },
            gpu_context: Some(gpu_context),
            egui_ctx,
            egui_state,
            needs_redraw: true,
            closed: false,
        }
    }

    /// Get the panel ID if this is a panel window
    pub fn panel_id(&self) -> Option<&str> {
        match &self.window_type {
            WindowType::Panel { panel_id } => Some(panel_id),
            _ => None,
        }
    }

    /// Get the output ID if this is a monitor window
    pub fn output_id(&self) -> Option<u32> {
        match &self.window_type {
            WindowType::Monitor { output_id } => Some(*output_id),
            _ => None,
        }
    }

    /// Check if this is the main window
    pub fn is_main(&self) -> bool {
        matches!(self.window_type, WindowType::Main)
    }

    /// Check if this is a monitor window
    pub fn is_monitor(&self) -> bool {
        matches!(self.window_type, WindowType::Monitor { .. })
    }

    /// Mark the window as needing a redraw
    pub fn request_redraw(&mut self) {
        self.needs_redraw = true;
        self.window.request_redraw();
    }

    /// Clear the redraw flag
    pub fn clear_redraw(&mut self) {
        self.needs_redraw = false;
    }

    /// Resize the window's GPU context
    pub fn resize(&mut self, gpu: &GpuContext, new_size: winit::dpi::PhysicalSize<u32>) {
        if let Some(gpu_ctx) = &mut self.gpu_context {
            gpu_ctx.resize(gpu, new_size);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// WINDOW REGISTRY — Tracks all windows
// ═══════════════════════════════════════════════════════════════════════════════

/// Registry for tracking all application windows
pub struct WindowRegistry {
    /// All registered windows by their winit WindowId
    windows: HashMap<WindowId, WindowEntry>,
    /// Map from panel ID to window ID (for quick lookup)
    panel_to_window: HashMap<String, WindowId>,
    /// Map from output/screen ID to window ID (for monitor windows)
    monitor_to_window: HashMap<u32, WindowId>,
    /// The main window's ID
    main_window_id: Option<WindowId>,
    /// Next viewport ID for egui (incremented for each new panel window)
    next_viewport_id: u64,
}

impl Default for WindowRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowRegistry {
    /// Create a new empty window registry
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            panel_to_window: HashMap::new(),
            monitor_to_window: HashMap::new(),
            main_window_id: None,
            next_viewport_id: 1,
        }
    }

    /// Register the main application window
    pub fn register_main_window(&mut self, window: Arc<Window>, egui_ctx: egui::Context) {
        let id = window.id();
        self.main_window_id = Some(id);
        self.windows.insert(id, WindowEntry::new_main(window, egui_ctx));
    }

    /// Register a panel window
    pub fn register_panel_window(
        &mut self,
        window: Arc<Window>,
        panel_id: String,
        gpu: &GpuContext,
    ) {
        let id = window.id();
        self.panel_to_window.insert(panel_id.clone(), id);
        self.windows.insert(id, WindowEntry::new_panel(window, panel_id, gpu));
        self.next_viewport_id += 1;
    }

    /// Register a monitor output window
    pub fn register_monitor_window(
        &mut self,
        window: Arc<Window>,
        output_id: u32,
        gpu: &GpuContext,
    ) {
        let id = window.id();
        self.monitor_to_window.insert(output_id, id);
        self.windows
            .insert(id, WindowEntry::new_monitor(window, output_id, gpu));
    }

    /// Unregister a window by its ID
    pub fn unregister_window(&mut self, window_id: WindowId) -> Option<WindowEntry> {
        if let Some(entry) = self.windows.remove(&window_id) {
            // Remove from panel_to_window if it's a panel
            if let WindowType::Panel { ref panel_id } = entry.window_type {
                self.panel_to_window.remove(panel_id);
            }
            // Remove from monitor_to_window if it's a monitor
            if let WindowType::Monitor { output_id } = entry.window_type {
                self.monitor_to_window.remove(&output_id);
            }
            // Clear main window ID if this was the main window
            if self.main_window_id == Some(window_id) {
                self.main_window_id = None;
            }
            Some(entry)
        } else {
            None
        }
    }

    /// Get a window entry by ID
    pub fn get(&self, window_id: WindowId) -> Option<&WindowEntry> {
        self.windows.get(&window_id)
    }

    /// Get a mutable window entry by ID
    pub fn get_mut(&mut self, window_id: WindowId) -> Option<&mut WindowEntry> {
        self.windows.get_mut(&window_id)
    }

    /// Get the main window entry
    pub fn main_window(&self) -> Option<&WindowEntry> {
        self.main_window_id.and_then(|id| self.windows.get(&id))
    }

    /// Get the main window entry mutably
    pub fn main_window_mut(&mut self) -> Option<&mut WindowEntry> {
        self.main_window_id.and_then(|id| self.windows.get_mut(&id))
    }

    /// Get the main window ID
    pub fn main_window_id(&self) -> Option<WindowId> {
        self.main_window_id
    }

    /// Check if a window ID is the main window
    pub fn is_main_window(&self, window_id: WindowId) -> bool {
        self.main_window_id == Some(window_id)
    }

    /// Get the window hosting a specific panel
    pub fn get_panel_window(&self, panel_id: &str) -> Option<&WindowEntry> {
        self.panel_to_window
            .get(panel_id)
            .and_then(|id| self.windows.get(id))
    }

    /// Get the window hosting a specific panel mutably
    pub fn get_panel_window_mut(&mut self, panel_id: &str) -> Option<&mut WindowEntry> {
        if let Some(&id) = self.panel_to_window.get(panel_id) {
            self.windows.get_mut(&id)
        } else {
            None
        }
    }

    /// Get the window ID for a panel
    pub fn get_panel_window_id(&self, panel_id: &str) -> Option<WindowId> {
        self.panel_to_window.get(panel_id).copied()
    }

    /// Get the window for a specific monitor output
    pub fn get_monitor_window(&self, output_id: u32) -> Option<&WindowEntry> {
        self.monitor_to_window
            .get(&output_id)
            .and_then(|id| self.windows.get(id))
    }

    /// Get the window for a specific monitor output mutably
    pub fn get_monitor_window_mut(&mut self, output_id: u32) -> Option<&mut WindowEntry> {
        if let Some(&id) = self.monitor_to_window.get(&output_id) {
            self.windows.get_mut(&id)
        } else {
            None
        }
    }

    /// Get the window ID for a monitor output
    pub fn get_monitor_window_id(&self, output_id: u32) -> Option<WindowId> {
        self.monitor_to_window.get(&output_id).copied()
    }

    /// Check if a monitor window exists for the given output ID
    pub fn has_monitor_window(&self, output_id: u32) -> bool {
        self.monitor_to_window.contains_key(&output_id)
    }

    /// Iterate over all windows
    pub fn iter(&self) -> impl Iterator<Item = (&WindowId, &WindowEntry)> {
        self.windows.iter()
    }

    /// Iterate over all windows mutably
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&WindowId, &mut WindowEntry)> {
        self.windows.iter_mut()
    }

    /// Iterate over all panel windows
    pub fn panel_windows(&self) -> impl Iterator<Item = (&WindowId, &WindowEntry)> {
        self.windows
            .iter()
            .filter(|(_, entry)| matches!(entry.window_type, WindowType::Panel { .. }))
    }

    /// Iterate over all monitor windows
    pub fn monitor_windows(&self) -> impl Iterator<Item = (&WindowId, &WindowEntry)> {
        self.windows
            .iter()
            .filter(|(_, entry)| matches!(entry.window_type, WindowType::Monitor { .. }))
    }

    /// Get the number of registered windows
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Get the number of panel windows
    pub fn panel_window_count(&self) -> usize {
        self.panel_to_window.len()
    }

    /// Get the number of monitor windows
    pub fn monitor_window_count(&self) -> usize {
        self.monitor_to_window.len()
    }

    /// Request redraw for all windows
    pub fn request_redraw_all(&mut self) {
        for entry in self.windows.values_mut() {
            entry.request_redraw();
        }
    }

    /// Mark a window as closed (for deferred cleanup)
    pub fn mark_closed(&mut self, window_id: WindowId) {
        if let Some(entry) = self.windows.get_mut(&window_id) {
            entry.closed = true;
        }
    }

    /// Remove all windows marked as closed
    pub fn cleanup_closed_windows(&mut self) -> Vec<WindowEntry> {
        let closed_ids: Vec<WindowId> = self
            .windows
            .iter()
            .filter(|(_, entry)| entry.closed)
            .map(|(id, _)| *id)
            .collect();

        let mut closed_entries = Vec::new();
        for id in closed_ids {
            if let Some(entry) = self.unregister_window(id) {
                closed_entries.push(entry);
            }
        }
        closed_entries
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_registry_new() {
        let registry = WindowRegistry::new();
        assert_eq!(registry.window_count(), 0);
        assert!(registry.main_window_id().is_none());
    }

    #[test]
    fn test_window_type_panel() {
        let wt = WindowType::Panel {
            panel_id: "properties".to_string(),
        };
        assert!(matches!(wt, WindowType::Panel { .. }));
    }

    #[test]
    fn test_window_type_main() {
        let wt = WindowType::Main;
        assert!(matches!(wt, WindowType::Main));
    }

    #[test]
    fn test_window_type_monitor() {
        let wt = WindowType::Monitor { output_id: 1 };
        assert!(matches!(wt, WindowType::Monitor { output_id: 1 }));
    }
}
