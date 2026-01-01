//! Clip types for the compositor
//!
//! Each layer has a 1D array of clip slots that can be triggered.
//! The unified clip grid UI shows rows = layers, columns = clip slots.

use std::path::PathBuf;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::effects::EffectStack;

/// Default number of clip slots per layer
pub const DEFAULT_CLIP_SLOTS: usize = 8;

/// Default transition duration in milliseconds
pub const DEFAULT_TRANSITION_DURATION_MS: u32 = 500;

/// The source type for a clip
#[derive(Debug, Clone, PartialEq)]
pub enum ClipSource {
    /// Local video file
    File {
        /// Path to the video file
        path: PathBuf,
    },
    /// OMT (Open Media Transport) network source
    Omt {
        /// Source identifier (e.g., "192.168.1.100:9000" or mDNS name)
        address: String,
        /// Human-readable name of the source
        name: String,
    },
    // Future: NDI source
    // Ndi { source_name: String },
}

/// Helper struct for ClipSource serialization (quick-xml compatible)
#[derive(Serialize, Deserialize)]
struct ClipSourceHelper {
    #[serde(rename = "type")]
    source_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

impl Serialize for ClipSource {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let helper = match self {
            ClipSource::File { path } => ClipSourceHelper {
                source_type: "File".to_string(),
                path: Some(path.clone()),
                address: None,
                name: None,
            },
            ClipSource::Omt { address, name } => ClipSourceHelper {
                source_type: "Omt".to_string(),
                path: None,
                address: Some(address.clone()),
                name: Some(name.clone()),
            },
        };
        helper.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ClipSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helper = ClipSourceHelper::deserialize(deserializer)?;
        match helper.source_type.as_str() {
            "File" => Ok(ClipSource::File {
                path: helper.path.unwrap_or_default(),
            }),
            "Omt" => Ok(ClipSource::Omt {
                address: helper.address.unwrap_or_default(),
                name: helper.name.unwrap_or_default(),
            }),
            _ => Ok(ClipSource::File {
                path: helper.path.unwrap_or_default(),
            }),
        }
    }
}

impl Default for ClipSource {
    fn default() -> Self {
        ClipSource::File { path: PathBuf::new() }
    }
}

impl ClipSource {
    /// Create a file source
    pub fn file(path: impl Into<PathBuf>) -> Self {
        ClipSource::File { path: path.into() }
    }

    /// Create an OMT source
    pub fn omt(address: impl Into<String>, name: impl Into<String>) -> Self {
        ClipSource::Omt {
            address: address.into(),
            name: name.into(),
        }
    }

    /// Check if this is a file source
    pub fn is_file(&self) -> bool {
        matches!(self, ClipSource::File { .. })
    }

    /// Check if this is an OMT source
    pub fn is_omt(&self) -> bool {
        matches!(self, ClipSource::Omt { .. })
    }

    /// Get the file path if this is a file source
    pub fn as_file_path(&self) -> Option<&PathBuf> {
        match self {
            ClipSource::File { path } => Some(path),
            _ => None,
        }
    }

    /// Get display name for this source
    pub fn display_name(&self) -> String {
        match self {
            ClipSource::File { path } => path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string(),
            ClipSource::Omt { name, .. } => name.clone(),
        }
    }

    /// Get a type indicator string
    pub fn type_indicator(&self) -> &'static str {
        match self {
            ClipSource::File { .. } => "📁",
            ClipSource::Omt { .. } => "📡",
        }
    }
}

/// Transition mode when switching between clips
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClipTransition {
    /// Instant switch to new clip
    #[default]
    Cut,
    /// Fade transition - old content fades out while new fades in (duration in ms)
    Fade(u32),
}

// Custom serialization as simple strings: "Cut" or "Fade:500"
impl Serialize for ClipTransition {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            ClipTransition::Cut => serializer.serialize_str("Cut"),
            ClipTransition::Fade(ms) => serializer.serialize_str(&format!("Fade:{}", ms)),
        }
    }
}

impl<'de> Deserialize<'de> for ClipTransition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s == "Cut" || s.is_empty() {
            Ok(ClipTransition::Cut)
        } else if s == "Fade" {
            // Just "Fade" without duration - use default
            Ok(ClipTransition::Fade(DEFAULT_TRANSITION_DURATION_MS))
        } else if let Some(duration_str) = s.strip_prefix("Fade:") {
            let ms = duration_str.parse().unwrap_or(DEFAULT_TRANSITION_DURATION_MS);
            Ok(ClipTransition::Fade(ms))
        } else {
            // Unknown format - default to Cut
            Ok(ClipTransition::Cut)
        }
    }
}

impl ClipTransition {
    /// Get display name for the transition
    pub fn name(&self) -> &'static str {
        match self {
            ClipTransition::Cut => "Cut",
            ClipTransition::Fade(_) => "Fade",
        }
    }

    /// Get the duration of the transition in milliseconds (0 for Cut)
    pub fn duration_ms(&self) -> u32 {
        match self {
            ClipTransition::Cut => 0,
            ClipTransition::Fade(ms) => *ms,
        }
    }

    /// Check if this transition requires keeping the old content
    pub fn needs_old_content(&self) -> bool {
        matches!(self, ClipTransition::Fade(_))
    }

    /// Create a fade transition with default duration
    pub fn fade() -> Self {
        ClipTransition::Fade(DEFAULT_TRANSITION_DURATION_MS)
    }
}

/// A single clip cell containing a video source
#[derive(Debug, Clone, Serialize)]
pub struct ClipCell {
    /// The source for this clip (file, OMT, etc.)
    pub source: ClipSource,

    /// Legacy: Path to the video/source file (kept for backward compatibility)
    /// New code should use `source` field instead
    #[serde(skip_serializing_if = "PathBuf::as_os_str_is_empty")]
    pub source_path: PathBuf,

    /// Optional user-defined label for the cell
    pub label: Option<String>,

    /// Effect stack for this clip
    #[serde(default)]
    pub effects: EffectStack,
}

/// Helper struct for deserializing ClipCell with backwards compatibility
#[derive(Deserialize)]
struct ClipCellRaw {
    #[serde(default)]
    source: Option<ClipSource>,
    #[serde(default)]
    source_path: PathBuf,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    effects: EffectStack,
}

impl<'de> Deserialize<'de> for ClipCell {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = ClipCellRaw::deserialize(deserializer)?;

        // Determine the source: prefer explicit source field, fall back to source_path
        let source = match raw.source {
            Some(s) => {
                // Check if source has a valid path, otherwise fall back to source_path
                match &s {
                    ClipSource::File { path } if path.as_os_str().is_empty() => {
                        // Empty source path, use legacy source_path if available
                        if !raw.source_path.as_os_str().is_empty() {
                            ClipSource::File { path: raw.source_path.clone() }
                        } else {
                            s
                        }
                    }
                    _ => s,
                }
            }
            None => {
                // No source field - construct from legacy source_path
                ClipSource::File { path: raw.source_path.clone() }
            }
        };

        Ok(ClipCell {
            source,
            source_path: raw.source_path,
            label: raw.label,
            effects: raw.effects,
        })
    }
}

/// Helper trait for serde skip_serializing_if
trait PathBufExt {
    fn as_os_str_is_empty(&self) -> bool;
}

impl PathBufExt for PathBuf {
    fn as_os_str_is_empty(&self) -> bool {
        self.as_os_str().is_empty()
    }
}

impl ClipCell {
    /// Create a new clip cell with a video path
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        Self {
            source: ClipSource::File { path: path.clone() },
            source_path: path,
            label: None,
            effects: EffectStack::new(),
        }
    }

    /// Create a new clip cell with a path and label
    pub fn with_label(path: impl Into<PathBuf>, label: impl Into<String>) -> Self {
        let path = path.into();
        Self {
            source: ClipSource::File { path: path.clone() },
            source_path: path,
            label: Some(label.into()),
            effects: EffectStack::new(),
        }
    }

    /// Create a new clip cell with an OMT source
    pub fn from_omt(address: impl Into<String>, name: impl Into<String>) -> Self {
        let name_str = name.into();
        Self {
            source: ClipSource::Omt {
                address: address.into(),
                name: name_str.clone(),
            },
            source_path: PathBuf::new(), // Empty for OMT sources
            label: Some(name_str),
            effects: EffectStack::new(),
        }
    }

    /// Get the display name for this cell (label or source name)
    pub fn display_name(&self) -> String {
        if let Some(ref label) = self.label {
            label.clone()
        } else {
            self.source.display_name()
        }
    }

    /// Check if this clip cell is valid (has a valid source)
    pub fn is_valid(&self) -> bool {
        match &self.source {
            ClipSource::File { path } => !path.as_os_str().is_empty(),
            ClipSource::Omt { address, .. } => !address.is_empty(),
        }
    }

    /// Check if this is an OMT source
    pub fn is_omt(&self) -> bool {
        self.source.is_omt()
    }

    /// Check if this is a file source
    pub fn is_file(&self) -> bool {
        self.source.is_file()
    }

    /// Get the type indicator for display
    pub fn type_indicator(&self) -> &'static str {
        self.source.type_indicator()
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
        assert!(cell.is_file());
        assert!(!cell.is_omt());
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

    #[test]
    fn test_clip_cell_omt() {
        let cell = ClipCell::from_omt("192.168.1.100:9000", "Desktop Stream");
        assert!(cell.is_omt());
        assert!(!cell.is_file());
        assert_eq!(cell.display_name(), "Desktop Stream");
        assert_eq!(cell.type_indicator(), "📡");
    }

    #[test]
    fn test_clip_source_file() {
        let source = ClipSource::file("/path/to/video.mp4");
        assert!(source.is_file());
        assert!(!source.is_omt());
        assert_eq!(source.display_name(), "video");
        assert_eq!(source.type_indicator(), "📁");
    }

    #[test]
    fn test_clip_source_omt() {
        let source = ClipSource::omt("192.168.1.50:9000", "My Stream");
        assert!(source.is_omt());
        assert!(!source.is_file());
        assert_eq!(source.display_name(), "My Stream");
        assert_eq!(source.type_indicator(), "📡");
    }

}
