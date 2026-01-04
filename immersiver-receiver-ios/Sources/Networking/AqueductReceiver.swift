import Foundation
import Network
import os.log

/// Delegate for receiving video frames and connection events
public protocol AqueductReceiverDelegate: AnyObject, Sendable {
    /// Called when a video frame is received
    func receiver(_ receiver: AqueductReceiver, didReceiveVideoFrame frame: VideoFrame)
    
    /// Called when an audio frame is received
    func receiver(_ receiver: AqueductReceiver, didReceiveAudioFrame frame: AudioFrame)
    
    /// Called when metadata is received
    func receiver(_ receiver: AqueductReceiver, didReceiveMetadata frame: MetadataFrame)
    
    /// Called when connection state changes
    func receiver(_ receiver: AqueductReceiver, didChangeState state: ConnectionState)
    
    /// Called when an error occurs
    func receiver(_ receiver: AqueductReceiver, didEncounterError error: Error)
}

/// Receives video streams from Aqueduct senders
public final class AqueductReceiver: @unchecked Sendable {
    /// Current connection state
    public var state: ConnectionState {
        get { stateStorage.value }
    }
    
    /// Delegate for callbacks
    public weak var delegate: AqueductReceiverDelegate? {
        get { delegateStorage.value }
        set { delegateStorage.value = newValue }
    }
    
    /// Currently connected source
    public var connectedSource: SourceInfo? {
        if case .connected(let source) = state {
            return source
        }
        return nil
    }
    
    /// Frame statistics
    public var frameCount: Int { frameCountStorage.value }
    public var lastFrameTime: Date? { lastFrameTimeStorage.value }
    
    private let stateStorage = MutableBox<ConnectionState>(.disconnected)
    private let delegateStorage = MutableBox<AqueductReceiverDelegate?>(nil)
    private let frameCountStorage = MutableBox<Int>(0)
    private let lastFrameTimeStorage = MutableBox<Date?>(nil)
    private let connectionStorage = MutableBox<NWConnection?>(nil)
    private let parserStorage = MutableBox<PacketParser?>(nil)
    
    private let logger = Logger(subsystem: "com.immersive.receiver.ios", category: "AqueductReceiver")
    private let receiveQueue = DispatchQueue(label: "com.immersive.receiver.ios.receive", qos: .userInteractive)
    
    public init() {}
    
    /// Connect to an Aqueduct source
    public func connect(to source: SourceInfo) {
        disconnect()
        
        logger.info("Connecting to source: \(source.address)")
        updateState(.connecting)
        
        let host = NWEndpoint.Host(source.host)
        let port = NWEndpoint.Port(rawValue: source.port)!
        
        let parameters = NWParameters.tcp
        parameters.allowLocalEndpointReuse = true
        
        // Optimize TCP for throughput
        if let tcpOptions = parameters.defaultProtocolStack.transportProtocol as? NWProtocolTCP.Options {
            tcpOptions.noDelay = true  // Disable Nagle's algorithm
        }
        
        let connection = NWConnection(host: host, port: port, using: parameters)
        connectionStorage.value = connection
        
        // Create new parser
        let parser = PacketParser()
        parserStorage.value = parser
        
        connection.stateUpdateHandler = { [weak self] state in
            self?.handleConnectionState(state, source: source)
        }
        
        connection.start(queue: receiveQueue)
    }
    
    /// Connect to an address string (host:port)
    public func connect(to address: String) {
        let components = address.split(separator: ":")
        guard components.count == 2,
              let port = UInt16(components[1]) else {
            updateState(.error("Invalid address format"))
            return
        }
        
        let source = SourceInfo(
            id: address,
            name: address,
            host: String(components[0]),
            port: port
        )
        
        connect(to: source)
    }
    
    /// Disconnect from current source
    public func disconnect() {
        logger.info("Disconnecting")
        connectionStorage.value?.cancel()
        connectionStorage.value = nil
        parserStorage.value = nil
        updateState(.disconnected)
    }
    
    // MARK: - Private Methods
    
    private func handleConnectionState(_ state: NWConnection.State, source: SourceInfo) {
        switch state {
        case .ready:
            logger.info("Connected to \(source.address)")
            updateState(.connected(source: source))
            startReceiving()
            
        case .failed(let error):
            logger.error("Connection failed: \(error.localizedDescription)")
            updateState(.error(error.localizedDescription))
            delegate?.receiver(self, didEncounterError: error)
            
        case .cancelled:
            logger.info("Connection cancelled")
            updateState(.disconnected)
            
        case .waiting(let error):
            logger.warning("Connection waiting: \(error.localizedDescription)")
            
        case .preparing:
            logger.debug("Connection preparing...")
            
        default:
            break
        }
    }
    
    private func startReceiving() {
        guard let connection = connectionStorage.value else { return }
        receiveData(on: connection)
    }
    
    private func receiveData(on connection: NWConnection) {
        // Request larger chunks for better throughput
        connection.receive(minimumIncompleteLength: 1, maximumLength: 1024 * 1024) { [weak self] data, _, isComplete, error in
            guard let self = self else { return }
            
            if let error = error {
                self.logger.error("Receive error: \(error.localizedDescription)")
                self.updateState(.error(error.localizedDescription))
                self.delegate?.receiver(self, didEncounterError: error)
                return
            }
            
            if let data = data, !data.isEmpty {
                // Process synchronously on receive queue - no Task overhead
                self.processReceivedData(data)
            }
            
            if isComplete {
                self.logger.info("Connection completed")
                self.updateState(.disconnected)
                return
            }
            
            // Continue receiving
            if case .connected = self.state {
                self.receiveData(on: connection)
            }
        }
    }
    
    private func processReceivedData(_ data: Data) {
        guard let parser = parserStorage.value else { return }
        
        parser.appendData(data)
        
        let packets = parser.parseAllPackets()
        
        for packet in packets {
            switch packet {
            case .video(let frame):
                frameCountStorage.value += 1
                lastFrameTimeStorage.value = Date()
                delegate?.receiver(self, didReceiveVideoFrame: frame)
                
            case .audio(let frame):
                delegate?.receiver(self, didReceiveAudioFrame: frame)
                
            case .metadata(let frame):
                delegate?.receiver(self, didReceiveMetadata: frame)
            }
        }
    }
    
    private func updateState(_ newState: ConnectionState) {
        stateStorage.value = newState
        delegate?.receiver(self, didChangeState: newState)
    }
}

// MARK: - Thread-Safe Box

/// A simple thread-safe mutable box for Sendable compliance
private final class MutableBox<T>: @unchecked Sendable {
    private let lock = NSLock()
    private var _value: T
    
    var value: T {
        get {
            lock.lock()
            defer { lock.unlock() }
            return _value
        }
        set {
            lock.lock()
            defer { lock.unlock() }
            _value = newValue
        }
    }
    
    init(_ value: T) {
        self._value = value
    }
}


