//! Network source discovery for OMT and NDI streams.
//!
//! Uses libOMT's built-in discovery for finding OMT sources on the network.

use super::omt_ffi::get_discovered_sources;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Type of discovered source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceType {
    /// Open Media Transport source
    Omt,
    /// NDI source (future)
    Ndi,
}

impl std::fmt::Display for SourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceType::Omt => write!(f, "OMT"),
            SourceType::Ndi => write!(f, "NDI"),
        }
    }
}

/// A discovered network video source.
#[derive(Debug, Clone)]
pub struct DiscoveredSource {
    /// Unique identifier for the source
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Source type (OMT, NDI, etc.)
    pub source_type: SourceType,
    /// Host address (from discovery string)
    pub host: String,
    /// Port number (0 if not resolved yet)
    pub port: u16,
    /// Additional properties
    pub properties: HashMap<String, String>,
}

impl DiscoveredSource {
    /// Get the full address (host:port) for connection.
    pub fn address(&self) -> String {
        if self.port > 0 {
            format!("{}:{}", self.host, self.port)
        } else {
            self.host.clone()
        }
    }
}

/// Callback type for source discovery events.
pub type DiscoveryCallback = Box<dyn Fn(DiscoveryEvent) + Send + Sync>;

/// Events emitted during source discovery.
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    /// A new source was found
    SourceFound(DiscoveredSource),
    /// A source was removed/went offline
    SourceLost(String), // source id
    /// Discovery error occurred
    Error(String),
}

/// Error type for discovery operations.
#[derive(Debug, Clone)]
pub struct DiscoveryError(pub String);

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Discovery error: {}", self.0)
    }
}

impl std::error::Error for DiscoveryError {}

/// Manages discovery of network video sources.
///
/// Uses libOMT's built-in discovery which returns sources in format:
/// "HOSTNAME (NAME)" e.g. "MACBOOKPRO.LOCAL (OBS Output)"
pub struct SourceDiscovery {
    /// Currently discovered sources (cached)
    sources: Arc<RwLock<HashMap<String, DiscoveredSource>>>,
    /// Is discovery active?
    running: bool,
}

impl SourceDiscovery {
    /// Create a new source discovery manager.
    pub fn new() -> Result<Self, DiscoveryError> {
        Ok(Self {
            sources: Arc::new(RwLock::new(HashMap::new())),
            running: false,
        })
    }

    /// Start browsing for OMT sources on the network.
    ///
    /// Note: libOMT discovery is polled (not callback-based), so this just
    /// marks discovery as active. Call `refresh()` periodically to update sources.
    pub fn start_browsing(&mut self) -> Result<(), DiscoveryError> {
        if self.running {
            return Ok(());
        }

        log::info!("SourceDiscovery: Starting libOMT discovery");
        self.running = true;

        // Do an initial refresh
        self.refresh();

        Ok(())
    }

    /// Refresh the list of discovered sources from libOMT.
    ///
    /// Call this periodically (e.g., every 1-2 seconds) to update the source list.
    pub fn refresh(&mut self) {
        if !self.running {
            return;
        }

        let addresses = get_discovered_sources();

        let mut new_sources = HashMap::new();

        for addr in addresses {
            // libOMT returns format: "HOSTNAME (NAME)"
            // Parse into host and name
            let (host, name) = Self::parse_discovery_address(&addr);

            let source = DiscoveredSource {
                id: addr.clone(),
                name,
                source_type: SourceType::Omt,
                host,
                port: 0, // libOMT discovery doesn't include port
                properties: HashMap::new(),
            };

            // Check if this is a new source
            let is_new = self.sources
                .read()
                .map(|s| !s.contains_key(&addr))
                .unwrap_or(true);

            if is_new {
                log::info!("SourceDiscovery: Found OMT source '{}'", source.name);
            }

            new_sources.insert(addr, source);
        }

        // Log removed sources
        if let Ok(old_sources) = self.sources.read() {
            for id in old_sources.keys() {
                if !new_sources.contains_key(id) {
                    log::info!("SourceDiscovery: OMT source removed: {}", id);
                }
            }
        }

        // Update the source list
        if let Ok(mut sources) = self.sources.write() {
            *sources = new_sources;
        }
    }

    /// Parse a libOMT discovery address string.
    ///
    /// Format: "HOSTNAME (NAME)" -> (hostname, name)
    fn parse_discovery_address(addr: &str) -> (String, String) {
        // Try to split on " (" to extract name
        if let Some(paren_start) = addr.find(" (") {
            let host = addr[..paren_start].trim().to_string();
            let name = addr[paren_start + 2..]
                .trim_end_matches(')')
                .to_string();
            (host, name)
        } else {
            // No parenthetical name, use the whole thing
            (addr.to_string(), addr.to_string())
        }
    }

    /// Stop browsing for sources.
    pub fn stop_browsing(&mut self) {
        self.running = false;
        log::info!("SourceDiscovery: Stopped browsing");
    }

    /// Get all currently discovered sources.
    pub fn get_sources(&self) -> Vec<DiscoveredSource> {
        self.sources
            .read()
            .map(|s| s.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Get sources of a specific type.
    pub fn get_sources_by_type(&self, source_type: SourceType) -> Vec<DiscoveredSource> {
        self.get_sources()
            .into_iter()
            .filter(|s| s.source_type == source_type)
            .collect()
    }

    /// Get a specific source by ID.
    pub fn get_source(&self, id: &str) -> Option<DiscoveredSource> {
        self.sources
            .read()
            .ok()
            .and_then(|s| s.get(id).cloned())
    }

    /// Check if discovery is running.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Get the count of discovered sources.
    pub fn source_count(&self) -> usize {
        self.sources
            .read()
            .map(|s| s.len())
            .unwrap_or(0)
    }
}

impl Default for SourceDiscovery {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            sources: Arc::new(RwLock::new(HashMap::new())),
            running: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovered_source_address() {
        let source = DiscoveredSource {
            id: "test".to_string(),
            name: "Test Source".to_string(),
            source_type: SourceType::Omt,
            host: "192.168.1.100".to_string(),
            port: 9000,
            properties: HashMap::new(),
        };

        assert_eq!(source.address(), "192.168.1.100:9000");
    }

    #[test]
    fn test_source_type_display() {
        assert_eq!(SourceType::Omt.to_string(), "OMT");
        assert_eq!(SourceType::Ndi.to_string(), "NDI");
    }

    #[test]
    fn test_parse_discovery_address() {
        let (host, name) = SourceDiscovery::parse_discovery_address("MACBOOKPRO.LOCAL (OBS Output)");
        assert_eq!(host, "MACBOOKPRO.LOCAL");
        assert_eq!(name, "OBS Output");

        let (host, name) = SourceDiscovery::parse_discovery_address("just-a-host");
        assert_eq!(host, "just-a-host");
        assert_eq!(name, "just-a-host");
    }
}
