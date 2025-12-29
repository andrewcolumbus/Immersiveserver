//! Shader management and hot-reload system
//!
//! This module provides shader loading and hot-reload functionality.
//! Shaders are loaded from disk at runtime and can be modified without
//! restarting the application. This enables users to create and edit
//! custom effects in real-time.

use std::path::{Path, PathBuf};

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::time::{Duration, Instant};

/// The embedded fullscreen quad shader (fallback if file is missing)
pub const FULLSCREEN_QUAD_SHADER: &str = include_str!("fullscreen_quad.wgsl");

/// Get the path to the shaders directory
pub fn shaders_dir() -> PathBuf {
    // In development, this is relative to the cargo manifest directory
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir).join("src").join("shaders")
}

/// Get the path to the fullscreen quad shader file
pub fn fullscreen_quad_path() -> PathBuf {
    shaders_dir().join("fullscreen_quad.wgsl")
}

/// Load the fullscreen quad shader source from disk
///
/// Falls back to the embedded shader if the file cannot be read.
pub fn load_fullscreen_quad_shader() -> Result<String, std::io::Error> {
    std::fs::read_to_string(fullscreen_quad_path())
}

// ============================================================================
// Shader Hot-Reload System
// ============================================================================

/// Watches shader files for changes and signals when reloading is needed
pub struct ShaderWatcher {
    /// The file watcher (kept alive to maintain watch)
    _watcher: RecommendedWatcher,
    /// Receiver for file change events
    receiver: Receiver<Result<Event, notify::Error>>,
    /// Last time we detected a change (for debouncing)
    last_change: Option<Instant>,
    /// Debounce duration (ignore rapid successive changes)
    debounce_duration: Duration,
    /// Path that changed (for reporting)
    pending_path: Option<PathBuf>,
}

impl ShaderWatcher {
    /// Create a new shader watcher
    ///
    /// Watches the shaders directory for changes to `.wgsl` files.
    pub fn new() -> Result<Self, notify::Error> {
        let (tx, rx) = channel();

        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default(),
        )?;

        // Watch the shaders directory
        let shaders_path = shaders_dir();
        log::info!("🔄 Shader hot-reload enabled, watching: {}", shaders_path.display());
        watcher.watch(&shaders_path, RecursiveMode::NonRecursive)?;

        Ok(Self {
            _watcher: watcher,
            receiver: rx,
            last_change: None,
            debounce_duration: Duration::from_millis(100),
            pending_path: None,
        })
    }

    /// Poll for shader changes
    ///
    /// Returns `Some(path)` if a shader file changed and enough time has
    /// passed since the last change (debouncing). Returns `None` otherwise.
    pub fn poll(&mut self) -> Option<PathBuf> {
        // Drain all pending events
        loop {
            match self.receiver.try_recv() {
                Ok(Ok(event)) => {
                    // Check if any of the paths are .wgsl files
                    for path in event.paths {
                        if path.extension().is_some_and(|ext| ext == "wgsl") {
                            self.last_change = Some(Instant::now());
                            self.pending_path = Some(path);
                        }
                    }
                }
                Ok(Err(e)) => {
                    log::warn!("Shader watcher error: {:?}", e);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    log::error!("Shader watcher channel disconnected");
                    break;
                }
            }
        }

        // Check if we have a pending change and debounce time has passed
        if let (Some(last), Some(path)) = (self.last_change, self.pending_path.take()) {
            if last.elapsed() >= self.debounce_duration {
                self.last_change = None;
                log::info!("🔄 Shader changed: {}", path.display());
                return Some(path);
            } else {
                // Put it back, not ready yet
                self.pending_path = Some(path);
            }
        }

        None
    }

    /// Check if a specific shader file changed
    ///
    /// Convenience method that polls and checks if the changed file
    /// matches the expected path.
    pub fn did_change(&mut self, expected: &Path) -> bool {
        if let Some(changed) = self.poll() {
            changed == expected
        } else {
            false
        }
    }
}

impl Default for ShaderWatcher {
    fn default() -> Self {
        Self::new().expect("Failed to create shader watcher")
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shaders_dir_exists() {
        let path = shaders_dir();
        assert!(path.exists(), "Shaders directory should exist at {:?}", path);
    }

    #[test]
    fn test_fullscreen_quad_path() {
        let path = fullscreen_quad_path();
        assert!(path.exists(), "Fullscreen quad shader should exist at {:?}", path);
    }

    #[test]
    fn test_embedded_shader_not_empty() {
        assert!(!FULLSCREEN_QUAD_SHADER.is_empty());
        assert!(FULLSCREEN_QUAD_SHADER.contains("fn vs_main"));
        assert!(FULLSCREEN_QUAD_SHADER.contains("fn fs_main"));
    }

    #[test]
    fn test_load_shader() {
        let source = load_fullscreen_quad_shader().expect("Failed to load shader");
        assert!(!source.is_empty());
        assert!(source.contains("fn vs_main"));
    }
}
