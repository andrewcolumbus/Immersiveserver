import UIKit
import MetalKit

/// Manages external display (HDMI/AirPlay) output
/// Creates a separate window on the external screen for fullscreen video output
public final class ExternalDisplayManager: @unchecked Sendable {
    
    /// Callback when external display is connected
    public var onExternalDisplayConnected: (() -> Void)?
    
    /// Callback when external display is disconnected
    public var onExternalDisplayDisconnected: (() -> Void)?
    
    /// Whether an external display is currently connected
    public private(set) var isExternalDisplayConnected: Bool = false
    
    /// External display resolution
    public private(set) var externalDisplaySize: CGSize = .zero
    
    // Internal state
    private var externalWindow: UIWindow?
    private var externalRenderer: VideoRenderer?
    private var metalView: MTKView?
    
    public init() {}
    
    /// Start monitoring for external display connections
    public func startMonitoring() {
        // Register for screen connection notifications
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleScreenDidConnect),
            name: UIScreen.didConnectNotification,
            object: nil
        )
        
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleScreenDidDisconnect),
            name: UIScreen.didDisconnectNotification,
            object: nil
        )
        
        // Check if already connected
        checkForExternalDisplay()
    }
    
    /// Stop monitoring for external display connections
    public func stopMonitoring() {
        NotificationCenter.default.removeObserver(self, name: UIScreen.didConnectNotification, object: nil)
        NotificationCenter.default.removeObserver(self, name: UIScreen.didDisconnectNotification, object: nil)
        
        tearDownExternalDisplay()
    }
    
    /// Update the frame on the external display
    public func updateFrame(_ frame: VideoFrame) {
        externalRenderer?.updateTexture(with: frame)
    }
    
    /// Set zoom level on external display
    public func setZoom(_ scale: Float) {
        externalRenderer?.setZoom(scale)
    }
    
    /// Set pan offset on external display
    public func setPan(x: Float, y: Float) {
        externalRenderer?.setPan(x: x, y: y)
    }
    
    /// Reset transform on external display
    public func resetTransform() {
        externalRenderer?.resetTransform()
    }
    
    // MARK: - Private Methods
    
    private func checkForExternalDisplay() {
        // Check all connected screens
        if UIScreen.screens.count > 1 {
            // External screen is at index 1 or higher
            if let externalScreen = UIScreen.screens.dropFirst().first {
                setupExternalDisplay(on: externalScreen)
            }
        }
    }
    
    @objc private func handleScreenDidConnect(_ notification: Notification) {
        guard let screen = notification.object as? UIScreen else { return }
        
        print("[ExternalDisplay] Screen connected: \(screen.bounds.size)")
        setupExternalDisplay(on: screen)
    }
    
    @objc private func handleScreenDidDisconnect(_ notification: Notification) {
        print("[ExternalDisplay] Screen disconnected")
        tearDownExternalDisplay()
    }
    
    private func setupExternalDisplay(on screen: UIScreen) {
        // Create window for external display
        let window = UIWindow(frame: screen.bounds)
        window.windowScene = UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .first { $0.screen == screen }
        
        // Create renderer for external display
        guard let renderer = VideoRenderer.create() else {
            print("[ExternalDisplay] ERROR: Failed to create renderer for external display")
            return
        }
        
        // Create Metal view
        let view = renderer.createView()
        view.frame = screen.bounds
        view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        
        // Create view controller
        let viewController = ExternalDisplayViewController()
        viewController.view = view
        
        window.rootViewController = viewController
        window.isHidden = false
        
        // Store references
        self.externalWindow = window
        self.externalRenderer = renderer
        self.metalView = view
        self.externalDisplaySize = screen.bounds.size
        self.isExternalDisplayConnected = true
        
        print("[ExternalDisplay] External display setup complete: \(screen.bounds.size)")
        
        // Notify
        DispatchQueue.main.async { [weak self] in
            self?.onExternalDisplayConnected?()
        }
    }
    
    private func tearDownExternalDisplay() {
        externalWindow?.isHidden = true
        externalWindow = nil
        externalRenderer = nil
        metalView = nil
        isExternalDisplayConnected = false
        externalDisplaySize = .zero
        
        // Notify
        DispatchQueue.main.async { [weak self] in
            self?.onExternalDisplayDisconnected?()
        }
    }
    
    deinit {
        stopMonitoring()
    }
}

// MARK: - External Display View Controller

/// Simple view controller for the external display
private class ExternalDisplayViewController: UIViewController {
    
    override var prefersStatusBarHidden: Bool {
        return true
    }
    
    override var prefersHomeIndicatorAutoHidden: Bool {
        return true
    }
    
    override var preferredScreenEdgesDeferringSystemGestures: UIRectEdge {
        return .all
    }
    
    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
    }
}

