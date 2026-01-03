import SwiftUI
import Combine

/// Main application state that coordinates all services
@MainActor
final class AppState: ObservableObject {
    // MARK: - Published Properties
    
    /// Current mode: receiving video or sending camera
    @Published var mode: AppMode = .receiver
    
    /// Whether the stream is playing (receiving) or broadcasting (sending)
    @Published var isPlaying: Bool = false
    
    /// Whether the overlay menu is visible
    @Published var isMenuVisible: Bool = false
    
    /// Currently selected source (persisted)
    @Published var selectedSource: SourceInfo? {
        didSet {
            if let source = selectedSource {
                persistSelectedSource(source)
            }
        }
    }
    
    /// Connection state for receiver mode
    @Published var connectionState: ConnectionState = .disconnected
    
    /// Current device IP address
    @Published var deviceIPAddress: String = "Unknown"
    
    /// Video dimensions for display
    @Published var videoDimensions: (width: Int, height: Int) = (0, 0)
    
    /// Zoom and pan state
    @Published var zoomScale: CGFloat = 1.0
    @Published var panOffset: CGSize = .zero
    
    // MARK: - Services
    
    let sourceDiscovery = SourceDiscovery()
    let receiver = AqueductReceiver()
    let videoRenderer: VideoRenderer?
    let externalDisplayManager = ExternalDisplayManager()
    var cameraManager: CameraManager?
    var sender: AqueductSender?
    
    // MARK: - Private
    
    private var cancellables = Set<AnyCancellable>()
    private let userDefaultsKey = "selectedSourceAddress"
    
    // MARK: - Initialization
    
    init() {
        // Create video renderer
        self.videoRenderer = VideoRenderer.create()
        
        // Setup receiver delegate
        receiver.delegate = ReceiverDelegateAdapter(appState: self)
        
        // Load persisted source
        loadPersistedSource()
        
        // Get device IP
        updateDeviceIP()
        
        // Start source discovery
        sourceDiscovery.startBrowsing()
        
        // Setup external display monitoring
        externalDisplayManager.onExternalDisplayConnected = { [weak self] in
            self?.handleExternalDisplayConnected()
        }
        externalDisplayManager.onExternalDisplayDisconnected = { [weak self] in
            self?.handleExternalDisplayDisconnected()
        }
        externalDisplayManager.startMonitoring()
        
        // Auto-connect if we have a persisted source
        if let source = selectedSource, isPlaying == false {
            // We'll let the user manually start
        }
    }
    
    // MARK: - Public Methods
    
    func toggleMenu() {
        withAnimation(.easeInOut(duration: 0.25)) {
            isMenuVisible.toggle()
        }
    }
    
    func togglePlayback() {
        isPlaying.toggle()
        
        if mode == .receiver {
            if isPlaying {
                if let source = selectedSource {
                    receiver.connect(to: source)
                }
            } else {
                receiver.disconnect()
            }
        } else {
            // Camera mode
            if isPlaying {
                startCameraBroadcast()
            } else {
                stopCameraBroadcast()
            }
        }
    }
    
    func selectSource(_ source: SourceInfo) {
        selectedSource = source
        
        // If playing, reconnect to new source
        if isPlaying && mode == .receiver {
            receiver.disconnect()
            receiver.connect(to: source)
        }
    }
    
    func switchMode(_ newMode: AppMode) {
        guard mode != newMode else { return }
        
        // Stop current mode
        if isPlaying {
            if mode == .receiver {
                receiver.disconnect()
            } else {
                stopCameraBroadcast()
            }
            isPlaying = false
        }
        
        mode = newMode
        
        if newMode == .camera {
            setupCameraMode()
        }
    }
    
    func resetZoomPan() {
        withAnimation(.easeOut(duration: 0.3)) {
            zoomScale = 1.0
            panOffset = .zero
        }
    }
    
    func refreshSources() {
        sourceDiscovery.refresh()
        updateDeviceIP()
    }
    
    // MARK: - Private Methods
    
    private func updateDeviceIP() {
        deviceIPAddress = NetworkUtility.getWiFiIPAddress() ?? "No WiFi"
    }
    
    private func persistSelectedSource(_ source: SourceInfo) {
        UserDefaults.standard.set(source.address, forKey: userDefaultsKey)
    }
    
    private func loadPersistedSource() {
        guard let address = UserDefaults.standard.string(forKey: userDefaultsKey) else { return }
        
        // Create a source from the persisted address
        let components = address.split(separator: ":")
        guard components.count == 2, let port = UInt16(components[1]) else { return }
        
        selectedSource = SourceInfo(
            id: address,
            name: "Saved Source",
            host: String(components[0]),
            port: port
        )
    }
    
    private func setupCameraMode() {
        if cameraManager == nil {
            cameraManager = CameraManager()
        }
        if sender == nil {
            sender = AqueductSender()
        }
    }
    
    private func startCameraBroadcast() {
        guard let camera = cameraManager, let sender = sender else {
            setupCameraMode()
            startCameraBroadcast()
            return
        }
        
        camera.onFrameCaptured = { [weak sender] frame in
            sender?.broadcast(frame: frame)
        }
        
        sender.start()
        camera.startCapture()
    }
    
    private func stopCameraBroadcast() {
        cameraManager?.stopCapture()
        sender?.stop()
    }
    
    private func handleExternalDisplayConnected() {
        // External display is now available - the renderer will automatically
        // be attached to it via ExternalDisplayManager
    }
    
    private func handleExternalDisplayDisconnected() {
        // External display was disconnected
    }
    
    func updateVideoFrame(_ frame: VideoFrame) {
        videoRenderer?.updateTexture(with: frame)
        videoDimensions = (Int(frame.width), Int(frame.height))
        
        // Also update external display if connected
        externalDisplayManager.updateFrame(frame)
    }
}

// MARK: - App Mode

enum AppMode: String, CaseIterable {
    case receiver = "Receiver"
    case camera = "Camera Output"
}

// MARK: - Receiver Delegate Adapter

/// Bridges the delegate pattern to the ObservableObject pattern
final class ReceiverDelegateAdapter: AqueductReceiverDelegate, @unchecked Sendable {
    private weak var appState: AppState?
    
    init(appState: AppState) {
        self.appState = appState
    }
    
    func receiver(_ receiver: AqueductReceiver, didReceiveVideoFrame frame: VideoFrame) {
        Task { @MainActor in
            appState?.updateVideoFrame(frame)
        }
    }
    
    func receiver(_ receiver: AqueductReceiver, didReceiveAudioFrame frame: AudioFrame) {
        // Audio handling not implemented for this version
    }
    
    func receiver(_ receiver: AqueductReceiver, didReceiveMetadata frame: MetadataFrame) {
        // Metadata handling not implemented for this version
    }
    
    func receiver(_ receiver: AqueductReceiver, didChangeState state: ConnectionState) {
        Task { @MainActor in
            appState?.connectionState = state
        }
    }
    
    func receiver(_ receiver: AqueductReceiver, didEncounterError error: Error) {
        Task { @MainActor in
            appState?.connectionState = .error(error.localizedDescription)
        }
    }
}

