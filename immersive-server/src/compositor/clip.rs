//! Clip types for the compositor
//!
//! Each layer has a 1D array of clip slots that can be triggered.
//! The unified clip grid UI shows rows = layers, columns = clip slots.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Default number of clip slots per layer
pub const DEFAULT_CLIP_SLOTS: usize = 8;

/// Default transition duration in milliseconds
pub const DEFAULT_TRANSITION_DURATION_MS: u32 = 500;

/// Transition mode when switching between clips
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ClipTransition {
    /// Instant switch to new clip
    #[default]
    Cut,
    /// Fade in from black/transparent (duration in ms)
    Fade(u32),
    /// Crossfade from previous clip (duration in ms)
    Crossfade(u32),
}

impl ClipTransition {
    /// Get display name for the transition
    pub fn name(&self) -> &'static str {
        match self {
            ClipTransition::Cut => "Cut",
            ClipTransition::Fade(_) => "Fade",
            ClipTransition::Crossfade(_) => "Crossfade",
        }
    }

    /// Get the duration of the transition in milliseconds (0 for Cut)
    pub fn duration_ms(&self) -> u32 {
        match self {
            ClipTransition::Cut => 0,
            ClipTransition::Fade(ms) => *ms,
            ClipTransition::Crossfade(ms) => *ms,
        }
    }

    /// Check if this transition requires keeping the old content
    pub fn needs_old_content(&self) -> bool {
        matches!(self, ClipTransition::Crossfade(_))
    }

    /// Create a fade transition with default duration
    pub fn fade() -> Self {
        ClipTransition::Fade(DEFAULT_TRANSITION_DURATION_MS)
    }

    /// Create a crossfade transition with default duration
    pub fn crossfade() -> Self {
        ClipTransition::Crossfade(DEFAULT_TRANSITION_DURATION_MS)
    }
}

/// A single clip cell containing a video source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipCell {
    /// Path to the video/source file
    #[serde(default)]
    pub source_path: PathBuf,
    /// Optional user-defined label for the cell
    #[serde(default)]
    pub label: Option<String>,
}

impl ClipCell {
    /// Create a new clip cell with a video path
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            source_path: path.into(),
            label: None,
        }
    }

    /// Create a new clip cell with a path and label
    pub fn with_label(path: impl Into<PathBuf>, label: impl Into<String>) -> Self {
        Self {
            source_path: path.into(),
            label: Some(label.into()),
        }
    }

    /// Get the display name for this cell (label or filename)
    pub fn display_name(&self) -> String {
        if let Some(ref label) = self.label {
            label.clone()
        } else {
            self.source_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string()
        }
    }

    /// Check if this clip cell is valid (has a non-empty source path)
    pub fn is_valid(&self) -> bool {
        !self.source_path.as_os_str().is_empty()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clip_cell_new() {
        let cell = ClipCell::new("/path/to/video.mp4");
        assert_eq!(cell.source_path, PathBuf::from("/path/to/video.mp4"));
        assert!(cell.label.is_none());
        assert_eq!(cell.transition, ClipTransition::Cut);
    }

    #[test]
    fn test_clip_cell_with_label() {
        let cell = ClipCell::with_label("/path/to/video.mp4", "Intro");
        assert_eq!(cell.label, Some("Intro".to_string()));
    }

    #[test]
    fn test_clip_cell_display_name() {
        let cell = ClipCell::new("/path/to/my_video.mp4");
        assert_eq!(cell.display_name(), "my_video");

        let cell_with_label = ClipCell::with_label("/path/to/video.mp4", "Custom Name");
        assert_eq!(cell_with_label.display_name(), "Custom Name");
    }
}
