import SwiftUI
import ImmersiveReceiverCore

@main
struct ImmersiveReceiverApp: App {
    @StateObject private var appState = AppState()
    
    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(appState)
        }
    }
}

/// Main application state for iOS
@MainActor
class AppState: ObservableObject {
    /// Source discovery
    @Published var sourceDiscovery = SourceDiscovery()
    
    /// Receiver broadcast
    @Published var receiverBroadcast: ReceiverBroadcast
    
    /// Control server
    @Published var controlServer: ControlServer
    
    /// Video receiver
    let videoReceiver = AqueductReceiver()
    
    /// Video renderer
    let videoRenderer: VideoRenderer?
    
    /// Receiver name
    @Published var receiverName: String {
        didSet {
            UserDefaults.standard.set(receiverName, forKey: "receiverName")
            controlServer.receiverName = receiverName
            if receiverBroadcast.isAdvertising {
                receiverBroadcast.updateName(receiverName)
            }
        }
    }
    
    /// Current connection state
    @Published var connectionState: ConnectionState = .disconnected
    
    /// Latest video frame info
    @Published var currentResolution: String = "No video"
    @Published var currentFps: Int = 0
    
    private var frameCount = 0
    private var lastFpsUpdate = Date()
    
    init() {
        // Load saved name or use default
        let savedName = UserDefaults.standard.string(forKey: "receiverName") ?? UIDevice.current.name
        self.receiverName = savedName
        
        // Initialize components
        self.receiverBroadcast = ReceiverBroadcast(controlPort: 9001)
        self.controlServer = ControlServer(port: 9001, receiverName: savedName)
        self.videoRenderer = VideoRenderer.create()
        
        // Set up receiver delegate
        videoReceiver.delegate = self
        
        // Set up control server delegate
        controlServer.delegate = self
        
        // Start services
        Task {
            await startServices()
        }
    }
    
    func startServices() async {
        sourceDiscovery.startBrowsing()
        receiverBroadcast.startAdvertising(name: receiverName)
        controlServer.start()
    }
    
    func stopServices() {
        sourceDiscovery.stopBrowsing()
        receiverBroadcast.stopAdvertising()
        controlServer.stop()
        videoReceiver.disconnect()
    }
    
    func connectToSource(_ source: SourceInfo) {
        videoReceiver.connect(to: source)
    }
    
    func disconnect() {
        videoReceiver.disconnect()
    }
}

// MARK: - AqueductReceiverDelegate

extension AppState: AqueductReceiverDelegate {
    nonisolated func receiver(_ receiver: AqueductReceiver, didReceiveVideoFrame frame: VideoFrame) {
        Task { @MainActor in
            // Update renderer
            videoRenderer?.updateTexture(with: frame)
            
            // Update stats
            frameCount += 1
            let now = Date()
            if now.timeIntervalSince(lastFpsUpdate) >= 1.0 {
                currentFps = frameCount
                frameCount = 0
                lastFpsUpdate = now
            }
            
            currentResolution = "\(frame.width)x\(frame.height)"
        }
    }
    
    nonisolated func receiver(_ receiver: AqueductReceiver, didReceiveAudioFrame frame: AudioFrame) {
        // Audio handling not implemented yet
    }
    
    nonisolated func receiver(_ receiver: AqueductReceiver, didReceiveMetadata frame: MetadataFrame) {
        // Metadata handling
    }
    
    nonisolated func receiver(_ receiver: AqueductReceiver, didChangeState state: ConnectionState) {
        Task { @MainActor in
            connectionState = state
        }
    }
    
    nonisolated func receiver(_ receiver: AqueductReceiver, didEncounterError error: Error) {
        Task { @MainActor in
            connectionState = .error(error.localizedDescription)
        }
    }
}

// MARK: - ControlServerDelegate

extension AppState: ControlServerDelegate {
    nonisolated func controlServer(_ server: ControlServer, didReceiveSwitchFeed sourceAddr: String) async -> Bool {
        await MainActor.run {
            videoReceiver.connect(to: sourceAddr)
            return true
        }
    }
    
    nonisolated func controlServerRequestStatus(_ server: ControlServer) async -> (connectedSource: String?, resolution: String?, fps: Int?) {
        await MainActor.run {
            let source = videoReceiver.connectedSource?.address
            return (source, currentResolution, currentFps)
        }
    }
    
    nonisolated func controlServerDidReceiveDisconnect(_ server: ControlServer) async {
        await MainActor.run {
            videoReceiver.disconnect()
        }
    }
}

