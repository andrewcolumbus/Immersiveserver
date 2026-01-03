import Foundation
import Network
import Compression
import os.log

/// OMT sender that broadcasts video frames to connected receivers
public final class AqueductSender: @unchecked Sendable {
    
    /// Whether the sender is currently running
    public var isRunning: Bool { isRunningStorage.value }
    
    /// Number of connected receivers
    public var connectedClientCount: Int { clientsStorage.value.count }
    
    /// Port the sender is listening on
    public let port: UInt16
    
    /// Service name for Bonjour advertising
    public let serviceName: String
    
    private let isRunningStorage = MutableBox<Bool>(false)
    private let clientsStorage = MutableBox<[ObjectIdentifier: NWConnection]>([:])
    private var listener: NWListener?
    private var advertiser: NWListener?
    
    private let logger = Logger(subsystem: "com.immersive.receiver.ios", category: "AqueductSender")
    private let sendQueue = DispatchQueue(label: "com.immersive.receiver.ios.sender", qos: .userInteractive)
    
    // Compression buffer
    private var compressBuffer: UnsafeMutablePointer<UInt8>?
    private var compressBufferSize: Int = 0
    
    public init(port: UInt16 = 9030, serviceName: String = "iOS Camera") {
        self.port = port
        self.serviceName = serviceName
    }
    
    /// Start the sender and begin accepting connections
    public func start() {
        guard !isRunning else { return }
        
        logger.info("Starting sender on port \(self.port)")
        
        do {
            let parameters = NWParameters.tcp
            parameters.allowLocalEndpointReuse = true
            
            // Optimize for throughput
            if let tcpOptions = parameters.defaultProtocolStack.transportProtocol as? NWProtocolTCP.Options {
                tcpOptions.noDelay = true
            }
            
            let listener = try NWListener(using: parameters, on: NWEndpoint.Port(rawValue: port)!)
            
            // Set up Bonjour advertising
            let txtRecord = NWTXTRecord([
                "version": "1.0",
                "type": "camera"
            ])
            
            listener.service = NWListener.Service(
                name: serviceName,
                type: "_omt._tcp",
                txtRecord: txtRecord
            )
            
            listener.stateUpdateHandler = { [weak self] state in
                self?.handleListenerState(state)
            }
            
            listener.newConnectionHandler = { [weak self] connection in
                self?.handleNewConnection(connection)
            }
            
            listener.start(queue: sendQueue)
            self.listener = listener
            
            // Allocate compression buffer
            compressBufferSize = 10_000_000 // 10MB should be enough for compressed frames
            compressBuffer = UnsafeMutablePointer<UInt8>.allocate(capacity: compressBufferSize)
            
            isRunningStorage.value = true
            
        } catch {
            logger.error("Failed to start sender: \(error.localizedDescription)")
        }
    }
    
    /// Stop the sender and disconnect all clients
    public func stop() {
        logger.info("Stopping sender")
        
        listener?.cancel()
        listener = nil
        
        // Disconnect all clients
        let clients = clientsStorage.value
        for (_, connection) in clients {
            connection.cancel()
        }
        clientsStorage.value = [:]
        
        // Free compression buffer
        compressBuffer?.deallocate()
        compressBuffer = nil
        compressBufferSize = 0
        
        isRunningStorage.value = false
    }
    
    /// Broadcast a video frame to all connected receivers
    public func broadcast(frame: VideoFrame) {
        guard isRunning else { return }
        
        let clients = clientsStorage.value
        guard !clients.isEmpty else { return }
        
        // Encode the frame
        guard let packetData = encodeVideoFrame(frame) else { return }
        
        // Send to all clients
        for (id, connection) in clients {
            connection.send(content: packetData, completion: .contentProcessed { [weak self] error in
                if let error = error {
                    self?.logger.error("Send error to client: \(error.localizedDescription)")
                    self?.removeClient(id: id)
                }
            })
        }
    }
    
    // MARK: - Private Methods
    
    private func handleListenerState(_ state: NWListener.State) {
        switch state {
        case .ready:
            logger.info("Sender ready on port \(self.port)")
        case .failed(let error):
            logger.error("Sender failed: \(error.localizedDescription)")
            isRunningStorage.value = false
        case .cancelled:
            logger.info("Sender cancelled")
            isRunningStorage.value = false
        default:
            break
        }
    }
    
    private func handleNewConnection(_ connection: NWConnection) {
        let id = ObjectIdentifier(connection)
        
        connection.stateUpdateHandler = { [weak self] state in
            self?.handleConnectionState(state, id: id, connection: connection)
        }
        
        connection.start(queue: sendQueue)
        
        logger.info("New receiver connecting...")
    }
    
    private func handleConnectionState(_ state: NWConnection.State, id: ObjectIdentifier, connection: NWConnection) {
        switch state {
        case .ready:
            logger.info("Receiver connected (total: \(self.clientsStorage.value.count + 1))")
            clientsStorage.value[id] = connection
            
        case .failed(let error):
            logger.error("Receiver connection failed: \(error.localizedDescription)")
            removeClient(id: id)
            
        case .cancelled:
            logger.info("Receiver disconnected")
            removeClient(id: id)
            
        default:
            break
        }
    }
    
    private func removeClient(id: ObjectIdentifier) {
        clientsStorage.value.removeValue(forKey: id)
        logger.info("Receiver removed (remaining: \(self.clientsStorage.value.count))")
    }
    
    private func encodeVideoFrame(_ frame: VideoFrame) -> Data? {
        guard let compressBuffer = compressBuffer else { return nil }
        
        // Compress the frame data using LZ4
        let srcData = frame.data
        let srcCount = srcData.count
        
        let compressedSize = srcData.withUnsafeBytes { srcPtr -> Int in
            guard let srcBase = srcPtr.baseAddress else { return 0 }
            
            return compression_encode_buffer(
                compressBuffer,
                compressBufferSize,
                srcBase.assumingMemoryBound(to: UInt8.self),
                srcCount,
                nil,
                COMPRESSION_LZ4_RAW
            )
        }
        
        guard compressedSize > 0 else { return nil }
        
        // Build packet:
        // Header: [Type: u8][Length: u32]
        // Payload: [Width: u32][Height: u32][Format: u8][Timestamp: u64][UncompressedSize: u32][CompressedData...]
        
        let payloadLength = 4 + 4 + 1 + 8 + 4 + compressedSize
        var packet = Data(capacity: 5 + payloadLength)
        
        // Type (video = 0x01)
        packet.append(0x01)
        
        // Length (big endian)
        var length = UInt32(payloadLength).bigEndian
        packet.append(contentsOf: withUnsafeBytes(of: &length) { Array($0) })
        
        // Width (big endian)
        var width = frame.width.bigEndian
        packet.append(contentsOf: withUnsafeBytes(of: &width) { Array($0) })
        
        // Height (big endian)
        var height = frame.height.bigEndian
        packet.append(contentsOf: withUnsafeBytes(of: &height) { Array($0) })
        
        // Format
        packet.append(frame.format.rawValue)
        
        // Timestamp (big endian, microseconds)
        var timestampMicros = UInt64(frame.timestamp * 1_000_000).bigEndian
        packet.append(contentsOf: withUnsafeBytes(of: &timestampMicros) { Array($0) })
        
        // Uncompressed size (little endian for LZ4 compatibility)
        var uncompressedSize = UInt32(srcCount).littleEndian
        packet.append(contentsOf: withUnsafeBytes(of: &uncompressedSize) { Array($0) })
        
        // Compressed data
        packet.append(UnsafeBufferPointer(start: compressBuffer, count: compressedSize))
        
        return packet
    }
    
    deinit {
        stop()
    }
}

// MARK: - Thread-Safe Box

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

