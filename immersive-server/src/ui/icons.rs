//! Icon constants for panel window controls
//!
//! Uses text-based labels instead of Unicode symbols for cross-platform compatibility.
//! Unicode symbols like ⊟ ⧉ ⊞ may render as squares on some fonts/platforms.

/// Panel window control icons
pub mod panel {
    /// Undock button - opens panel in separate OS window
    pub const UNDOCK: &str = "Undock";
    /// Float button - floats panel within main window
    pub const FLOAT: &str = "Float";
    /// Dock button - docks panel to edge
    pub const DOCK: &str = "Dock";
    /// Close button
    pub const CLOSE: &str = "X";
}
