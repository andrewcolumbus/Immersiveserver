import Foundation

/// Information about a discovered Aqueduct video source
public struct SourceInfo: Identifiable, Hashable, Sendable, Codable {
    public let id: String
    public let name: String
    public let host: String
    public let port: UInt16
    public let properties: [String: String]
    
    public init(id: String, name: String, host: String, port: UInt16, properties: [String: String] = [:]) {
        self.id = id
        self.name = name
        self.host = host
        self.port = port
        self.properties = properties
    }
    
    /// Full address string for connection
    public var address: String {
        "\(host):\(port)"
    }
    
    /// Display name for UI
    public var displayName: String {
        name.isEmpty ? address : name
    }
}

/// Information about a discovered receiver (for immersive-player to control)
public struct ReceiverInfo: Identifiable, Hashable, Sendable {
    public let id: String
    public let name: String
    public let host: String
    public let controlPort: UInt16
    public let properties: [String: String]
    
    public init(id: String, name: String, host: String, controlPort: UInt16, properties: [String: String] = [:]) {
        self.id = id
        self.name = name
        self.host = host
        self.controlPort = controlPort
        self.properties = properties
    }
    
    /// Control address string
    public var controlAddress: String {
        "\(host):\(controlPort)"
    }
}

/// Connection state for the receiver
public enum ConnectionState: Sendable {
    case disconnected
    case connecting
    case connected(source: SourceInfo)
    case error(String)
    
    public var isConnected: Bool {
        if case .connected = self { return true }
        return false
    }
    
    public var displayText: String {
        switch self {
        case .disconnected:
            return "Disconnected"
        case .connecting:
            return "Connecting..."
        case .connected(let source):
            return "Connected to \(source.displayName)"
        case .error(let message):
            return "Error: \(message)"
        }
    }
}


