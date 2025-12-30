//! Network source discovery for OMT and NDI streams.
//!
//! Uses mDNS-SD (Bonjour-compatible) for automatic discovery of
//! video sources on the local network.

use aqueduct::{Discovery as AqueductDiscovery, AqueductError};
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
    /// Host address
    pub host: String,
    /// Port number
    pub port: u16,
    /// Additional properties
    pub properties: HashMap<String, String>,
}

impl DiscoveredSource {
    /// Get the full address (host:port) for connection.
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
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

/// Manages discovery of network video sources.
pub struct SourceDiscovery {
    /// Internal Aqueduct discovery service
    aqueduct_discovery: Option<AqueductDiscovery>,
    /// Currently discovered sources
    sources: Arc<RwLock<HashMap<String, DiscoveredSource>>>,
    /// Is discovery running?
    running: bool,
}

impl SourceDiscovery {
    /// Create a new source discovery manager.
    pub fn new() -> Result<Self, AqueductError> {
        let discovery = AqueductDiscovery::new()?;
        
        Ok(Self {
            aqueduct_discovery: Some(discovery),
            sources: Arc::new(RwLock::new(HashMap::new())),
            running: false,
        })
    }

    /// Start browsing for OMT sources on the network.
    pub fn start_browsing(&mut self) -> Result<(), AqueductError> {
        if self.running {
            return Ok(()); // Already running
        }

        let discovery = self.aqueduct_discovery.as_ref().ok_or_else(|| {
            AqueductError::Discovery("Discovery not initialized".to_string())
        })?;

        let sources = Arc::clone(&self.sources);

        log::info!("SourceDiscovery: Starting mDNS browse for OMT sources");

        discovery.browse_sources(move |event| {
            use mdns_sd::ServiceEvent;
            
            match event {
                ServiceEvent::ServiceResolved(info) => {
                    let source = DiscoveredSource {
                        id: info.get_fullname().to_string(),
                        name: info.get_fullname()
                            .split('.')
                            .next()
                            .unwrap_or("Unknown")
                            .to_string(),
                        source_type: SourceType::Omt,
                        host: info.get_addresses()
                            .iter()
                            .next()
                            .map(|a| a.to_string())
                            .unwrap_or_else(|| info.get_hostname().to_string()),
                        port: info.get_port(),
                        properties: info.get_properties()
                            .iter()
                            .map(|p| (p.key().to_string(), p.val_str().to_string()))
                            .collect(),
                    };

                    log::info!(
                        "SourceDiscovery: Found OMT source '{}' at {}:{}",
                        source.name,
                        source.host,
                        source.port
                    );

                    if let Ok(mut sources) = sources.write() {
                        sources.insert(source.id.clone(), source);
                    }
                }
                ServiceEvent::ServiceFound(service_type, fullname) => {
                    log::debug!(
                        "SourceDiscovery: Found service '{}' of type '{}'",
                        fullname,
                        service_type
                    );
                    // ServiceFound is fired before ServiceResolved
                    // We wait for ServiceResolved to get full details
                }
                ServiceEvent::ServiceRemoved(_, fullname) => {
                    log::info!("SourceDiscovery: OMT source removed: {}", fullname);
                    
                    if let Ok(mut sources) = sources.write() {
                        sources.remove(&fullname);
                    }
                }
                ServiceEvent::SearchStarted(_) => {
                    log::debug!("SourceDiscovery: mDNS search started");
                }
                ServiceEvent::SearchStopped(_) => {
                    log::debug!("SourceDiscovery: mDNS search stopped");
                }
            }
        })?;

        self.running = true;
        Ok(())
    }

    /// Stop browsing for sources.
    pub fn stop_browsing(&mut self) {
        self.running = false;
        // Note: Aqueduct discovery runs in a background thread
        // It will stop when the Discovery object is dropped
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
            aqueduct_discovery: None,
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
}

