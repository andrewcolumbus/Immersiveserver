//! Cross-platform GPU texture sharing abstraction.
//!
//! This module provides a platform-agnostic interface for sharing GPU textures
//! with other applications. On macOS, this uses Syphon; on Windows, Spout.

/// Platform-agnostic texture sharing interface.
///
/// Implementations share the environment texture directly on the GPU without
/// CPU readback, providing zero-latency output to compatible receivers.
pub trait TextureSharer: Send {
    /// Start sharing with the given server name.
    ///
    /// The name will be visible to receivers browsing for available sources.
    fn start(&mut self, name: &str) -> Result<(), String>;

    /// Publish the current frame.
    ///
    /// This should be called once per frame after rendering to the environment
    /// texture but before presenting to the window.
    fn publish_frame(&mut self) -> Result<(), String>;

    /// Check if any receivers are connected.
    fn has_receivers(&self) -> bool;

    /// Stop sharing and release resources.
    fn stop(&mut self);

    /// Get the technology name for UI display ("Syphon" or "Spout").
    fn technology_name(&self) -> &'static str;

    /// Check if sharing is currently active.
    fn is_active(&self) -> bool;
}

/// Get the platform-specific technology name.
#[cfg(target_os = "macos")]
pub fn platform_technology_name() -> &'static str {
    "Syphon"
}

#[cfg(target_os = "windows")]
pub fn platform_technology_name() -> &'static str {
    "Spout"
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn platform_technology_name() -> &'static str {
    "Texture Share"
}

/// Check if texture sharing is available on this platform.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn is_available() -> bool {
    true
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn is_available() -> bool {
    false
}
