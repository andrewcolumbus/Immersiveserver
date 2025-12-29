import Foundation
import Network
import os.log

// MARK: - Control Protocol Messages

/// Commands that can be received from immersive-player
public enum ControlCommand: Codable, Sendable {
    case switchFeed(sourceAddr: String)
    case getStatus
    case disconnect
    
    private enum CodingKeys: String, CodingKey {
        case type
        case sourceAddr = "source_addr"
    }
    
    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        
        switch type {
        case "switch_feed":
            let addr = try container.decode(String.self, forKey: .sourceAddr)
            self = .switchFeed(sourceAddr: addr)
        case "get_status":
            self = .getStatus
        case "disconnect":
            self = .disconnect
        default:
            throw DecodingError.dataCorrupted(.init(codingPath: [CodingKeys.type], debugDescription: "Unknown command type: \(type)"))
        }
    }
    
    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .switchFeed(let addr):
            try container.encode("switch_feed", forKey: .type)
            try container.encode(addr, forKey: .sourceAddr)
        case .getStatus:
            try container.encode("get_status", forKey: .type)
        case .disconnect:
            try container.encode("disconnect", forKey: .type)
        }
    }
}

/// Responses sent back to immersive-player
public enum ControlResponse: Codable, Sendable {
    case status(name: String, connectedSource: String?, resolution: String?, fps: Int?)
    case ack(success: Bool)
    case error(message: String)
    
    private enum CodingKeys: String, CodingKey {
        case type
        case name
        case connectedSource = "connected_source"
        case resolution
        case fps
        case success
        case message
    }
    
    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .status(let name, let source, let resolution, let fps):
            try container.encode("status", forKey: .type)
            try container.encode(name, forKey: .name)
            try container.encodeIfPresent(source, forKey: .connectedSource)
            try container.encodeIfPresent(resolution, forKey: .resolution)
            try container.encodeIfPresent(fps, forKey: .fps)
        case .ack(let success):
            try container.encode("ack", forKey: .type)
            try container.encode(success, forKey: .success)
        case .error(let message):
            try container.encode("error", forKey: .type)
            try container.encode(message, forKey: .message)
        }
    }
    
    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        
        switch type {
        case "status":
            let name = try container.decode(String.self, forKey: .name)
            let source = try container.decodeIfPresent(String.self, forKey: .connectedSource)
            let resolution = try container.decodeIfPresent(String.self, forKey: .resolution)
            let fps = try container.decodeIfPresent(Int.self, forKey: .fps)
            self = .status(name: name, connectedSource: source, resolution: resolution, fps: fps)
        case "ack":
            let success = try container.decode(Bool.self, forKey: .success)
            self = .ack(success: success)
        case "error":
            let message = try container.decode(String.self, forKey: .message)
            self = .error(message: message)
        default:
            throw DecodingError.dataCorrupted(.init(codingPath: [CodingKeys.type], debugDescription: "Unknown response type"))
        }
    }
}

// MARK: - Control Server Delegate

/// Delegate for handling control commands
public protocol ControlServerDelegate: AnyObject {
    /// Handle a switch feed command
    func controlServer(_ server: ControlServer, didReceiveSwitchFeed sourceAddr: String) async -> Bool
    
    /// Get current status information
    func controlServerRequestStatus(_ server: ControlServer) async -> (connectedSource: String?, resolution: String?, fps: Int?)
    
    /// Handle disconnect command
    func controlServerDidReceiveDisconnect(_ server: ControlServer) async
}

// MARK: - Control Server

/// TCP server for receiving control commands from immersive-player
@MainActor
public final class ControlServer: ObservableObject {
    /// Whether the server is running
    @Published public private(set) var isRunning: Bool = false
    
    /// Number of connected controllers
    @Published public private(set) var connectedClients: Int = 0
    
    /// Last error encountered
    @Published public private(set) var lastError: String?
    
    /// The receiver's display name
    public var receiverName: String
    
    /// Control port
    public let port: UInt16
    
    /// Delegate for handling commands
    public weak var delegate: ControlServerDelegate?
    
    private var listener: NWListener?
    private var connections: [ObjectIdentifier: NWConnection] = [:]
    private let logger = Logger(subsystem: "com.immersive.receiver", category: "ControlServer")
    private let processingQueue = DispatchQueue(label: "com.immersive.receiver.control", qos: .userInitiated)
    
    public init(port: UInt16 = 9001, receiverName: String = "Immersive Receiver") {
        self.port = port
        self.receiverName = receiverName
    }
    
    /// Start the control server
    public func start() {
        guard listener == nil else {
            logger.info("Control server already running")
            return
        }
        
        logger.info("Starting control server on port \(self.port)")
        
        do {
            let parameters = NWParameters.tcp
            parameters.allowLocalEndpointReuse = true
            
            let listener = try NWListener(using: parameters, on: NWEndpoint.Port(rawValue: port)!)
            
            listener.stateUpdateHandler = { [weak self] state in
                Task { @MainActor in
                    self?.handleListenerState(state)
                }
            }
            
            listener.newConnectionHandler = { [weak self] connection in
                Task { @MainActor in
                    self?.handleNewConnection(connection)
                }
            }
            
            listener.start(queue: processingQueue)
            self.listener = listener
            
        } catch {
            logger.error("Failed to start control server: \(error.localizedDescription)")
            lastError = error.localizedDescription
        }
    }
    
    /// Stop the control server
    public func stop() {
        logger.info("Stopping control server")
        
        // Close all connections
        for (_, connection) in connections {
            connection.cancel()
        }
        connections.removeAll()
        connectedClients = 0
        
        // Stop listener
        listener?.cancel()
        listener = nil
        isRunning = false
    }
    
    // MARK: - Private Methods
    
    private func handleListenerState(_ state: NWListener.State) {
        switch state {
        case .ready:
            logger.info("Control server ready on port \(self.port)")
            isRunning = true
            lastError = nil
        case .failed(let error):
            logger.error("Control server failed: \(error.localizedDescription)")
            lastError = error.localizedDescription
            isRunning = false
        case .cancelled:
            logger.info("Control server cancelled")
            isRunning = false
        default:
            break
        }
    }
    
    private func handleNewConnection(_ connection: NWConnection) {
        let id = ObjectIdentifier(connection)
        connections[id] = connection
        connectedClients = connections.count
        
        logger.info("New control client connected (total: \(self.connectedClients))")
        
        connection.stateUpdateHandler = { [weak self] state in
            Task { @MainActor in
                self?.handleConnectionState(state, id: id, connection: connection)
            }
        }
        
        connection.start(queue: processingQueue)
    }
    
    private func handleConnectionState(_ state: NWConnection.State, id: ObjectIdentifier, connection: NWConnection) {
        switch state {
        case .ready:
            logger.debug("Control connection ready")
            startReceiving(on: connection, id: id)
        case .failed(let error):
            logger.error("Control connection failed: \(error.localizedDescription)")
            removeConnection(id: id)
        case .cancelled:
            logger.debug("Control connection cancelled")
            removeConnection(id: id)
        default:
            break
        }
    }
    
    private func removeConnection(id: ObjectIdentifier) {
        connections.removeValue(forKey: id)
        connectedClients = connections.count
    }
    
    private func startReceiving(on connection: NWConnection, id: ObjectIdentifier) {
        connection.receive(minimumIncompleteLength: 1, maximumLength: 65536) { [weak self] data, _, isComplete, error in
            guard let self = self else { return }
            
            if let error = error {
                self.logger.error("Control receive error: \(error.localizedDescription)")
                connection.cancel()
                return
            }
            
            if let data = data, !data.isEmpty {
                Task { @MainActor in
                    await self.processCommand(data: data, connection: connection)
                }
            }
            
            if isComplete {
                connection.cancel()
                return
            }
            
            // Continue receiving on main actor
            Task { @MainActor in
                self.startReceiving(on: connection, id: id)
            }
        }
    }
    
    private func processCommand(data: Data, connection: NWConnection) async {
        // Try to parse JSON command (may have multiple commands separated by newlines)
        let jsonString = String(data: data, encoding: .utf8) ?? ""
        let lines = jsonString.split(separator: "\n")
        
        for line in lines {
            guard let lineData = String(line).data(using: .utf8) else { continue }
            
            do {
                let command = try JSONDecoder().decode(ControlCommand.self, from: lineData)
                let response = await handleCommand(command)
                sendResponse(response, to: connection)
            } catch {
                logger.error("Failed to parse command: \(error.localizedDescription)")
                sendResponse(.error(message: "Invalid command: \(error.localizedDescription)"), to: connection)
            }
        }
    }
    
    private func handleCommand(_ command: ControlCommand) async -> ControlResponse {
        switch command {
        case .switchFeed(let sourceAddr):
            logger.info("Received switch_feed command: \(sourceAddr)")
            if let delegate = delegate {
                let success = await delegate.controlServer(self, didReceiveSwitchFeed: sourceAddr)
                return .ack(success: success)
            }
            return .error(message: "No handler available")
            
        case .getStatus:
            logger.debug("Received get_status command")
            if let delegate = delegate {
                let status = await delegate.controlServerRequestStatus(self)
                return .status(
                    name: receiverName,
                    connectedSource: status.connectedSource,
                    resolution: status.resolution,
                    fps: status.fps
                )
            }
            return .status(name: receiverName, connectedSource: nil, resolution: nil, fps: nil)
            
        case .disconnect:
            logger.info("Received disconnect command")
            if let delegate = delegate {
                await delegate.controlServerDidReceiveDisconnect(self)
            }
            return .ack(success: true)
        }
    }
    
    private func sendResponse(_ response: ControlResponse, to connection: NWConnection) {
        do {
            var data = try JSONEncoder().encode(response)
            data.append(contentsOf: "\n".utf8)  // Add newline delimiter
            
            connection.send(content: data, completion: .contentProcessed { [weak self] error in
                if let error = error {
                    self?.logger.error("Failed to send response: \(error.localizedDescription)")
                }
            })
        } catch {
            logger.error("Failed to encode response: \(error.localizedDescription)")
        }
    }
}

