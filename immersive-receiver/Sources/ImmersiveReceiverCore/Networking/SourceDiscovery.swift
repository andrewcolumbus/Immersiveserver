import Foundation
import Network
import os.log

/// Service type for Aqueduct/OMT video sources
private let omtServiceType = "_omt._tcp"

/// Discovers Aqueduct video sources on the local network using Bonjour/mDNS
@MainActor
public final class SourceDiscovery: ObservableObject {
    /// Currently discovered sources
    @Published public private(set) var sources: [SourceInfo] = []
    
    /// Whether discovery is active
    @Published public private(set) var isSearching: Bool = false
    
    /// Last error encountered
    @Published public private(set) var lastError: String?
    
    private var browser: NWBrowser?
    private var resolvers: [String: NWConnection] = [:]
    private let logger = Logger(subsystem: "com.immersive.receiver", category: "SourceDiscovery")
    
    public init() {
        // Add localhost as a default test source
        addLocalhost()
    }
    
    /// Add localhost sender as a default source for testing
    private func addLocalhost() {
        let localhost = SourceInfo(
            id: "localhost_9030",
            name: "Local Sender (localhost:9030)",
            host: "127.0.0.1",
            port: 9030,
            properties: ["type": "manual"]
        )
        sources.append(localhost)
    }
    
    /// Manually add a source by address
    public func addManualSource(host: String, port: UInt16, name: String? = nil) {
        let displayName = name ?? "\(host):\(port)"
        let source = SourceInfo(
            id: "\(host)_\(port)",
            name: displayName,
            host: host,
            port: port,
            properties: ["type": "manual"]
        )
        
        // Check if already exists
        if !sources.contains(where: { $0.id == source.id }) {
            sources.append(source)
            logger.info("Added manual source: \(displayName)")
        }
    }
    
    /// Start browsing for Aqueduct sources
    public func startBrowsing() {
        guard browser == nil else {
            logger.info("Already browsing for sources")
            return
        }
        
        logger.info("Starting source discovery for \(omtServiceType)")
        
        let parameters = NWParameters()
        parameters.includePeerToPeer = true
        
        let browser = NWBrowser(for: .bonjour(type: omtServiceType, domain: nil), using: parameters)
        
        browser.stateUpdateHandler = { [weak self] state in
            Task { @MainActor in
                self?.handleBrowserState(state)
            }
        }
        
        browser.browseResultsChangedHandler = { [weak self] results, changes in
            Task { @MainActor in
                self?.handleBrowseResults(results, changes: changes)
            }
        }
        
        browser.start(queue: .main)
        self.browser = browser
        isSearching = true
    }
    
    /// Stop browsing for sources
    public func stopBrowsing() {
        logger.info("Stopping source discovery")
        browser?.cancel()
        browser = nil
        isSearching = false
        
        // Cancel all resolvers
        for (_, connection) in resolvers {
            connection.cancel()
        }
        resolvers.removeAll()
    }
    
    /// Refresh the source list
    public func refresh() {
        stopBrowsing()
        sources.removeAll()
        startBrowsing()
    }
    
    // MARK: - Private Methods
    
    private func handleBrowserState(_ state: NWBrowser.State) {
        switch state {
        case .ready:
            logger.info("Browser ready")
            lastError = nil
        case .failed(let error):
            logger.error("Browser failed: \(error.localizedDescription)")
            lastError = error.localizedDescription
            isSearching = false
        case .cancelled:
            logger.info("Browser cancelled")
            isSearching = false
        case .waiting(let error):
            logger.warning("Browser waiting: \(error.localizedDescription)")
        default:
            break
        }
    }
    
    private func handleBrowseResults(_ results: Set<NWBrowser.Result>, changes: Set<NWBrowser.Result.Change>) {
        for change in changes {
            switch change {
            case .added(let result):
                handleServiceAdded(result)
            case .removed(let result):
                handleServiceRemoved(result)
            case .changed(old: _, new: let new, flags: _):
                handleServiceChanged(new)
            case .identical:
                break
            @unknown default:
                break
            }
        }
    }
    
    private func handleServiceAdded(_ result: NWBrowser.Result) {
        guard case .service(let name, let type, let domain, _) = result.endpoint else {
            return
        }
        
        logger.info("Found source: \(name) (\(type).\(domain))")
        
        // Resolve the service to get IP and port
        resolveService(result)
    }
    
    private func handleServiceRemoved(_ result: NWBrowser.Result) {
        guard case .service(let name, _, _, _) = result.endpoint else {
            return
        }
        
        logger.info("Source removed: \(name)")
        sources.removeAll { $0.name == name }
        resolvers[name]?.cancel()
        resolvers.removeValue(forKey: name)
    }
    
    private func handleServiceChanged(_ result: NWBrowser.Result) {
        // Re-resolve to update info
        resolveService(result)
    }
    
    private func resolveService(_ result: NWBrowser.Result) {
        guard case .service(let name, _, _, _) = result.endpoint else {
            return
        }
        
        // Cancel any existing resolver for this service
        resolvers[name]?.cancel()
        
        let parameters = NWParameters.tcp
        let connection = NWConnection(to: result.endpoint, using: parameters)
        resolvers[name] = connection
        
        connection.stateUpdateHandler = { [weak self] state in
            Task { @MainActor in
                self?.handleResolverState(state, name: name, result: result, connection: connection)
            }
        }
        
        connection.start(queue: .main)
    }
    
    private func handleResolverState(_ state: NWConnection.State, name: String, result: NWBrowser.Result, connection: NWConnection) {
        switch state {
        case .ready:
            // Get the resolved endpoint
            if let endpoint = connection.currentPath?.remoteEndpoint {
                let (host, port) = extractHostPort(from: endpoint)
                
                // Extract metadata if available
                var properties: [String: String] = [:]
                if case .service(_, _, _, let interface) = result.endpoint {
                    properties["interface"] = interface?.name ?? "unknown"
                }
                
                // Extract TXT record if available
                if case let .bonjour(txtRecord) = result.metadata {
                    for key in txtRecord.dictionary.keys {
                        if let value = txtRecord.dictionary[key] {
                            properties[key] = value
                        }
                    }
                }
                
                let source = SourceInfo(
                    id: "\(name)_\(host)_\(port)",
                    name: name,
                    host: host,
                    port: port,
                    properties: properties
                )
                
                // Update or add source
                if let index = sources.firstIndex(where: { $0.name == name }) {
                    sources[index] = source
                } else {
                    sources.append(source)
                }
                
                logger.info("Resolved source: \(name) at \(host):\(port)")
            }
            
            // Close the connection (we only needed to resolve)
            connection.cancel()
            resolvers.removeValue(forKey: name)
            
        case .failed(let error):
            logger.error("Failed to resolve \(name): \(error.localizedDescription)")
            connection.cancel()
            resolvers.removeValue(forKey: name)
            
        case .cancelled:
            resolvers.removeValue(forKey: name)
            
        default:
            break
        }
    }
    
    private func extractHostPort(from endpoint: NWEndpoint) -> (String, UInt16) {
        switch endpoint {
        case .hostPort(let host, let port):
            let hostString: String
            switch host {
            case .ipv4(let addr):
                hostString = addr.debugDescription
            case .ipv6(let addr):
                hostString = addr.debugDescription
            case .name(let name, _):
                hostString = name
            @unknown default:
                hostString = host.debugDescription
            }
            return (hostString, port.rawValue)
        default:
            return ("unknown", 0)
        }
    }
}


