//! Screen definitions for multi-output display
//!
//! A Screen represents a single output destination (display, projector, NDI stream, etc.)
//! Each screen contains one or more slices that define what content is shown and where.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::color::OutputColorCorrection;
use super::slice::Slice;

/// Unique identifier for a screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ScreenId(pub u32);

impl std::fmt::Display for ScreenId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Screen {}", self.0)
    }
}

/// Output destination type
#[derive(Debug, Clone, PartialEq)]
pub enum OutputDevice {
    /// Internal preview only (no external output)
    Virtual,

    /// Physical display/monitor/projector
    Display {
        /// Display identifier from OS
        display_id: u32,
    },

    /// NDI network output
    Ndi {
        /// NDI source name
        name: String,
    },

    /// OMT (Open Media Transport) network output
    Omt {
        /// OMT source name
        name: String,
        /// OMT port
        port: u16,
    },

    /// Syphon texture sharing (macOS only)
    #[cfg(target_os = "macos")]
    Syphon {
        /// Syphon server name
        name: String,
    },

    /// Spout texture sharing (Windows only)
    #[cfg(target_os = "windows")]
    Spout {
        /// Spout sender name
        name: String,
    },
}

/// Helper struct for OutputDevice serialization (quick-xml compatible)
#[derive(Serialize, Deserialize)]
struct OutputDeviceHelper {
    #[serde(rename = "type")]
    device_type: String,
    #[serde(rename = "displayId", default, skip_serializing_if = "Option::is_none")]
    display_id: Option<u32>,
    #[serde(rename = "name", default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(rename = "port", default, skip_serializing_if = "Option::is_none")]
    port: Option<u16>,
}

impl Serialize for OutputDevice {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let helper = match self {
            OutputDevice::Virtual => OutputDeviceHelper {
                device_type: "Virtual".to_string(),
                display_id: None,
                name: None,
                port: None,
            },
            OutputDevice::Display { display_id } => OutputDeviceHelper {
                device_type: "Display".to_string(),
                display_id: Some(*display_id),
                name: None,
                port: None,
            },
            OutputDevice::Ndi { name } => OutputDeviceHelper {
                device_type: "Ndi".to_string(),
                display_id: None,
                name: Some(name.clone()),
                port: None,
            },
            OutputDevice::Omt { name, port } => OutputDeviceHelper {
                device_type: "Omt".to_string(),
                display_id: None,
                name: Some(name.clone()),
                port: Some(*port),
            },
            #[cfg(target_os = "macos")]
            OutputDevice::Syphon { name } => OutputDeviceHelper {
                device_type: "Syphon".to_string(),
                display_id: None,
                name: Some(name.clone()),
                port: None,
            },
            #[cfg(target_os = "windows")]
            OutputDevice::Spout { name } => OutputDeviceHelper {
                device_type: "Spout".to_string(),
                display_id: None,
                name: Some(name.clone()),
                port: None,
            },
        };
        helper.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for OutputDevice {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helper = OutputDeviceHelper::deserialize(deserializer)?;
        match helper.device_type.as_str() {
            "Virtual" => Ok(OutputDevice::Virtual),
            "Display" => Ok(OutputDevice::Display {
                display_id: helper.display_id.unwrap_or(0),
            }),
            "Ndi" => Ok(OutputDevice::Ndi {
                name: helper.name.unwrap_or_default(),
            }),
            "Omt" => Ok(OutputDevice::Omt {
                name: helper.name.unwrap_or_default(),
                port: helper.port.unwrap_or(5000),
            }),
            #[cfg(target_os = "macos")]
            "Syphon" => Ok(OutputDevice::Syphon {
                name: helper.name.unwrap_or_default(),
            }),
            #[cfg(target_os = "windows")]
            "Spout" => Ok(OutputDevice::Spout {
                name: helper.name.unwrap_or_default(),
            }),
            _ => Ok(OutputDevice::Virtual),
        }
    }
}

impl Default for OutputDevice {
    fn default() -> Self {
        Self::Virtual
    }
}

impl OutputDevice {
    /// Get a human-readable name for the output device type
    pub fn type_name(&self) -> &'static str {
        match self {
            OutputDevice::Virtual => "Virtual",
            OutputDevice::Display { .. } => "Display",
            OutputDevice::Ndi { .. } => "NDI",
            OutputDevice::Omt { .. } => "OMT",
            #[cfg(target_os = "macos")]
            OutputDevice::Syphon { .. } => "Syphon",
            #[cfg(target_os = "windows")]
            OutputDevice::Spout { .. } => "Spout",
        }
    }

    /// Get the display name for this output device
    pub fn display_name(&self) -> String {
        match self {
            OutputDevice::Virtual => "Virtual Preview".to_string(),
            OutputDevice::Display { display_id } => format!("Display {}", display_id),
            OutputDevice::Ndi { name } => format!("NDI: {}", name),
            OutputDevice::Omt { name, port } => format!("OMT: {} (port {})", name, port),
            #[cfg(target_os = "macos")]
            OutputDevice::Syphon { name } => format!("Syphon: {}", name),
            #[cfg(target_os = "windows")]
            OutputDevice::Spout { name } => format!("Spout: {}", name),
        }
    }
}

/// A screen output destination with slices
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Screen {
    /// Unique screen identifier
    #[serde(rename = "id")]
    pub id: ScreenId,

    /// Display name for this screen
    #[serde(rename = "name")]
    pub name: String,

    /// Output device/destination
    #[serde(rename = "device")]
    pub device: OutputDevice,

    /// Output resolution width
    #[serde(rename = "width")]
    pub width: u32,

    /// Output resolution height
    #[serde(rename = "height")]
    pub height: u32,

    /// Slices that compose this screen's output
    #[serde(rename = "slices", default)]
    pub slices: Vec<Slice>,

    /// Whether this screen is enabled for output
    #[serde(rename = "enabled")]
    pub enabled: bool,

    /// Per-screen color correction (applied after all slices)
    #[serde(rename = "colorCorrection", default)]
    pub color: OutputColorCorrection,

    /// Output timing delay in milliseconds (for projector sync)
    #[serde(rename = "delayMs", default)]
    pub delay_ms: u32,
}

impl Default for Screen {
    fn default() -> Self {
        Self {
            id: ScreenId::default(),
            name: "Screen 1".to_string(),
            device: OutputDevice::Virtual,
            width: 1920,
            height: 1080,
            slices: Vec::new(),
            enabled: true,
            color: OutputColorCorrection::default(),
            delay_ms: 0,
        }
    }
}

impl Screen {
    /// Create a new screen with default settings
    pub fn new(id: ScreenId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            ..Default::default()
        }
    }

    /// Create a new screen with a default full-composition slice
    pub fn new_with_default_slice(
        id: ScreenId,
        name: impl Into<String>,
        slice_id: super::slice::SliceId,
    ) -> Self {
        let mut screen = Self::new(id, name);
        screen.slices.push(Slice::new_full_composition(slice_id, "Slice 1"));
        screen
    }

    /// Get enabled slices
    pub fn enabled_slices(&self) -> impl Iterator<Item = &Slice> {
        self.slices.iter().filter(|s| s.enabled)
    }

    /// Get mutable enabled slices
    pub fn enabled_slices_mut(&mut self) -> impl Iterator<Item = &mut Slice> {
        self.slices.iter_mut().filter(|s| s.enabled)
    }

    /// Add a slice to this screen
    pub fn add_slice(&mut self, slice: Slice) {
        self.slices.push(slice);
    }

    /// Remove a slice by ID
    pub fn remove_slice(&mut self, slice_id: super::slice::SliceId) -> Option<Slice> {
        if let Some(pos) = self.slices.iter().position(|s| s.id == slice_id) {
            Some(self.slices.remove(pos))
        } else {
            None
        }
    }

    /// Find a slice by ID
    pub fn find_slice(&self, slice_id: super::slice::SliceId) -> Option<&Slice> {
        self.slices.iter().find(|s| s.id == slice_id)
    }

    /// Find a mutable slice by ID
    pub fn find_slice_mut(&mut self, slice_id: super::slice::SliceId) -> Option<&mut Slice> {
        self.slices.iter_mut().find(|s| s.id == slice_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_default() {
        let screen = Screen::default();
        assert_eq!(screen.name, "Screen 1");
        assert_eq!(screen.width, 1920);
        assert_eq!(screen.height, 1080);
        assert!(screen.enabled);
        assert!(screen.slices.is_empty());
    }

    #[test]
    fn test_screen_with_slice() {
        let screen =
            Screen::new_with_default_slice(ScreenId(1), "Test Screen", super::super::SliceId(1));
        assert_eq!(screen.name, "Test Screen");
        assert_eq!(screen.slices.len(), 1);
        assert_eq!(screen.slices[0].name, "Slice 1");
    }

    #[test]
    fn test_output_device_names() {
        assert_eq!(OutputDevice::Virtual.type_name(), "Virtual");
        assert_eq!(
            OutputDevice::Display { display_id: 1 }.type_name(),
            "Display"
        );
        assert_eq!(
            OutputDevice::Ndi {
                name: "test".to_string()
            }
            .type_name(),
            "NDI"
        );
    }
}
