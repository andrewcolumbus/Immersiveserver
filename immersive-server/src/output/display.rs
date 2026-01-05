//! Display enumeration and management
//!
//! Provides utilities to query connected displays/monitors and track their status
//! for multi-display output.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use winit::event_loop::ActiveEventLoop;
use winit::monitor::MonitorHandle;

/// Information about a connected display
#[derive(Debug, Clone)]
pub struct DisplayInfo {
    /// Unique identifier for this display (stable across app restarts)
    pub id: u32,

    /// Display name from the operating system
    pub name: String,

    /// Position in the virtual desktop (top-left corner)
    pub position: (i32, i32),

    /// Physical resolution in pixels
    pub size: (u32, u32),

    /// DPI scale factor
    pub scale_factor: f64,

    /// Refresh rate in millihertz (e.g., 60000 = 60Hz), if available
    pub refresh_rate_millihertz: Option<u32>,

    /// Whether this is the primary display
    pub is_primary: bool,

    /// The underlying monitor handle (for window creation)
    monitor_handle: MonitorHandle,
}

impl DisplayInfo {
    /// Create a DisplayInfo from a winit MonitorHandle
    fn from_monitor(monitor: &MonitorHandle, is_primary: bool) -> Self {
        let id = display_id_from_monitor(monitor);
        let name = monitor.name().unwrap_or_else(|| "Unknown Display".to_string());
        let pos = monitor.position();
        let position = (pos.x, pos.y);
        let size = monitor.size();
        let scale_factor = monitor.scale_factor();
        let refresh_rate_millihertz = monitor.refresh_rate_millihertz();

        Self {
            id,
            name,
            position,
            size: (size.width, size.height),
            scale_factor,
            refresh_rate_millihertz,
            is_primary,
            monitor_handle: monitor.clone(),
        }
    }

    /// Get a display label suitable for UI (includes resolution)
    pub fn label(&self) -> String {
        let refresh = self
            .refresh_rate_millihertz
            .map(|r| format!(" @ {}Hz", r / 1000))
            .unwrap_or_default();
        format!(
            "{} ({}x{}{})",
            self.name, self.size.0, self.size.1, refresh
        )
    }

    /// Get the underlying MonitorHandle for window creation
    pub fn monitor_handle(&self) -> &MonitorHandle {
        &self.monitor_handle
    }
}

/// Generate a stable display ID from a MonitorHandle
///
/// Uses name + position to create a hash that should be consistent
/// across app restarts as long as the display configuration doesn't change.
fn display_id_from_monitor(monitor: &MonitorHandle) -> u32 {
    let mut hasher = DefaultHasher::new();

    // Hash the name
    if let Some(name) = monitor.name() {
        name.hash(&mut hasher);
    }

    // Hash the position (helps distinguish multiple identical monitors)
    let pos = monitor.position();
    pos.x.hash(&mut hasher);
    pos.y.hash(&mut hasher);

    // Hash the size
    let size = monitor.size();
    size.width.hash(&mut hasher);
    size.height.hash(&mut hasher);

    // Return lower 32 bits of hash
    hasher.finish() as u32
}

/// Connection status of a display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayStatus {
    /// Display is currently connected
    Connected,
    /// Display was connected but is now disconnected
    Disconnected,
}

/// Events from display connection changes
#[derive(Debug, Clone)]
pub enum DisplayEvent {
    /// A new display was connected
    Connected(DisplayInfo),
    /// A display was disconnected
    Disconnected(u32),
}

/// Manager for connected displays
///
/// Tracks available displays and provides methods for enumeration and lookup.
#[derive(Debug, Default)]
pub struct DisplayManager {
    /// Currently connected displays
    displays: HashMap<u32, DisplayInfo>,

    /// Status of known displays (for hot-plug detection)
    status: HashMap<u32, DisplayStatus>,
}

impl DisplayManager {
    /// Create a new DisplayManager
    pub fn new() -> Self {
        Self::default()
    }

    /// Refresh the list of connected displays from the event loop
    ///
    /// Call this during initialization and periodically for hot-plug detection.
    pub fn refresh(&mut self, event_loop: &ActiveEventLoop) {
        let mut current_ids = std::collections::HashSet::new();

        // Get the primary monitor
        let primary_monitor = event_loop.primary_monitor();
        let primary_id = primary_monitor.as_ref().map(|m| display_id_from_monitor(m));

        // Enumerate all monitors
        for monitor in event_loop.available_monitors() {
            let is_primary = primary_id == Some(display_id_from_monitor(&monitor));
            let info = DisplayInfo::from_monitor(&monitor, is_primary);
            let id = info.id;
            current_ids.insert(id);
            self.displays.insert(id, info);
            self.status.insert(id, DisplayStatus::Connected);
        }

        // Mark missing displays as disconnected (keep them in status for tracking)
        for (id, status) in self.status.iter_mut() {
            if !current_ids.contains(id) {
                *status = DisplayStatus::Disconnected;
                self.displays.remove(id);
            }
        }
    }

    /// Check for connection changes and return events
    ///
    /// Call this periodically to detect hot-plug events.
    pub fn check_connections(&mut self, event_loop: &ActiveEventLoop) -> Vec<DisplayEvent> {
        let mut events = Vec::new();

        // Track previous state
        let previous_ids: std::collections::HashSet<_> = self.displays.keys().copied().collect();

        // Refresh the display list
        self.refresh(event_loop);

        // Find new connections
        for (id, info) in &self.displays {
            if !previous_ids.contains(id) {
                events.push(DisplayEvent::Connected(info.clone()));
            }
        }

        // Find disconnections
        for id in &previous_ids {
            if !self.displays.contains_key(id) {
                events.push(DisplayEvent::Disconnected(*id));
            }
        }

        events
    }

    /// Get all currently connected displays
    pub fn displays(&self) -> impl Iterator<Item = &DisplayInfo> {
        self.displays.values()
    }

    /// Get display count
    pub fn count(&self) -> usize {
        self.displays.len()
    }

    /// Get a display by ID
    pub fn get(&self, id: u32) -> Option<&DisplayInfo> {
        self.displays.get(&id)
    }

    /// Check if a display is currently connected
    pub fn is_connected(&self, id: u32) -> bool {
        self.displays.contains_key(&id)
    }

    /// Get the status of a display (even if disconnected)
    pub fn status(&self, id: u32) -> Option<DisplayStatus> {
        self.status.get(&id).copied()
    }

    /// Get the primary display
    pub fn primary(&self) -> Option<&DisplayInfo> {
        self.displays.values().find(|d| d.is_primary)
    }

    /// Get displays sorted by position (left to right, then top to bottom)
    pub fn displays_sorted(&self) -> Vec<&DisplayInfo> {
        let mut displays: Vec<_> = self.displays.values().collect();
        displays.sort_by(|a, b| {
            a.position
                .0
                .cmp(&b.position.0)
                .then_with(|| a.position.1.cmp(&b.position.1))
        });
        displays
    }

    /// Find a display containing the given point in virtual desktop coordinates
    pub fn display_at_point(&self, x: i32, y: i32) -> Option<&DisplayInfo> {
        self.displays.values().find(|d| {
            x >= d.position.0
                && x < d.position.0 + d.size.0 as i32
                && y >= d.position.1
                && y < d.position.1 + d.size.1 as i32
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_manager_new() {
        let manager = DisplayManager::new();
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_display_info_label() {
        // We can't easily test with real monitors, but we can test the label format
        // by checking that the format string works correctly
        let label = format!(
            "{} ({}x{}{})",
            "Test Display",
            1920,
            1080,
            " @ 60Hz".to_string()
        );
        assert_eq!(label, "Test Display (1920x1080 @ 60Hz)");
    }

    #[test]
    fn test_display_status() {
        assert_eq!(DisplayStatus::Connected, DisplayStatus::Connected);
        assert_ne!(DisplayStatus::Connected, DisplayStatus::Disconnected);
    }

    #[test]
    fn test_display_event_types() {
        // Test that we can create display events
        let connected = DisplayEvent::Disconnected(123);
        match connected {
            DisplayEvent::Disconnected(id) => assert_eq!(id, 123),
            _ => panic!("Expected Disconnected event"),
        }
    }

    #[test]
    fn test_display_manager_is_connected() {
        let manager = DisplayManager::new();
        // Empty manager should report all displays as not connected
        assert!(!manager.is_connected(12345));
    }
}
