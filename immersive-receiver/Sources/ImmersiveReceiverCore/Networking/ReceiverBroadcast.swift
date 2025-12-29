import Foundation
import Network
import os.log

/// Service type for Aqueduct/OMT receivers (for control discovery)
private let receiverServiceType = "_omt-receiver._tcp"

/// Broadcasts this receiver's presence on the network for discovery by immersive-player
@MainActor
public final class ReceiverBroadcast: ObservableObject {
    /// Whether the receiver is currently being advertised
    @Published public private(set) var isAdvertising: Bool = false
    
    /// The name being advertised
    @Published public private(set) var advertisedName: String = ""
    
    /// Last error encountered
    @Published public private(set) var lastError: String?
    
    private var listener: NWListener?
    private let logger = Logger(subsystem: "com.immersive.receiver", category: "ReceiverBroadcast")
    
    /// Control port for incoming connections
    public let controlPort: UInt16
    
    public init(controlPort: UInt16 = 9001) {
        self.controlPort = controlPort
    }
    
    /// Start advertising this receiver on the network
    /// - Parameter name: The display name for this receiver
    public func startAdvertising(name: String) {
        guard listener == nil else {
            logger.info("Already advertising")
            return
        }
        
        logger.info("Starting receiver broadcast as '\(name)' on port \(self.controlPort)")
        
        do {
            let parameters = NWParameters.tcp
            parameters.includePeerToPeer = true
            
            let listener = try NWListener(using: parameters, on: NWEndpoint.Port(rawValue: controlPort)!)
            
            // Set up Bonjour advertising
            let txtRecord = NWTXTRecord([
                "version": "1.0",
                "name": name
            ])
            
            listener.service = NWListener.Service(
                name: name,
                type: receiverServiceType,
                txtRecord: txtRecord
            )
            
            listener.stateUpdateHandler = { [weak self] state in
                Task { @MainActor in
                    self?.handleListenerState(state)
                }
            }
            
            listener.newConnectionHandler = { [weak self] connection in
                // We don't handle connections here - the ControlServer does that
                // Just cancel incoming connections on this listener
                connection.cancel()
                self?.logger.debug("Ignored connection on broadcast listener (handled by ControlServer)")
            }
            
            listener.start(queue: .main)
            self.listener = listener
            self.advertisedName = name
            
        } catch {
            logger.error("Failed to create listener: \(error.localizedDescription)")
            lastError = error.localizedDescription
        }
    }
    
    /// Stop advertising this receiver
    public func stopAdvertising() {
        logger.info("Stopping receiver broadcast")
        listener?.cancel()
        listener = nil
        isAdvertising = false
        advertisedName = ""
    }
    
    /// Update the advertised name
    public func updateName(_ newName: String) {
        if isAdvertising {
            stopAdvertising()
            startAdvertising(name: newName)
        }
    }
    
    // MARK: - Private Methods
    
    private func handleListenerState(_ state: NWListener.State) {
        switch state {
        case .ready:
            logger.info("Receiver broadcast ready")
            isAdvertising = true
            lastError = nil
        case .failed(let error):
            logger.error("Broadcast failed: \(error.localizedDescription)")
            lastError = error.localizedDescription
            isAdvertising = false
        case .cancelled:
            logger.info("Broadcast cancelled")
            isAdvertising = false
        case .waiting(let error):
            logger.warning("Broadcast waiting: \(error.localizedDescription)")
        default:
            break
        }
    }
}






