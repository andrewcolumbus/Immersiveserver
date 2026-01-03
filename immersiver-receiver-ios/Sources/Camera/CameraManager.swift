import AVFoundation
import UIKit

/// Manages camera capture for broadcasting as an OMT source
public final class CameraManager: NSObject, @unchecked Sendable {
    
    /// Callback when a frame is captured
    public var onFrameCaptured: ((VideoFrame) -> Void)?
    
    /// Whether the camera is currently capturing
    public private(set) var isCapturing: Bool = false
    
    /// Current camera position
    public private(set) var cameraPosition: AVCaptureDevice.Position = .back
    
    /// Current capture resolution
    public private(set) var captureSize: CGSize = .zero
    
    // AVFoundation components
    private let captureSession = AVCaptureSession()
    private var videoOutput: AVCaptureVideoDataOutput?
    private var currentInput: AVCaptureDeviceInput?
    
    private let captureQueue = DispatchQueue(label: "com.immersive.receiver.ios.camera", qos: .userInteractive)
    
    // Frame counter for timestamp generation
    private var frameCount: UInt64 = 0
    private var startTime: Date?
    
    public override init() {
        super.init()
        setupCaptureSession()
    }
    
    /// Start capturing from the camera
    public func startCapture() {
        guard !isCapturing else { return }
        
        captureQueue.async { [weak self] in
            self?.captureSession.startRunning()
            self?.startTime = Date()
            self?.frameCount = 0
            
            DispatchQueue.main.async {
                self?.isCapturing = true
            }
        }
    }
    
    /// Stop capturing
    public func stopCapture() {
        guard isCapturing else { return }
        
        captureQueue.async { [weak self] in
            self?.captureSession.stopRunning()
            
            DispatchQueue.main.async {
                self?.isCapturing = false
            }
        }
    }
    
    /// Switch between front and back camera
    public func switchCamera() {
        let newPosition: AVCaptureDevice.Position = cameraPosition == .back ? .front : .back
        
        captureQueue.async { [weak self] in
            self?.configureCamera(position: newPosition)
        }
    }
    
    // MARK: - Private Methods
    
    private func setupCaptureSession() {
        captureSession.beginConfiguration()
        
        // Set session preset for high quality
        if captureSession.canSetSessionPreset(.hd1920x1080) {
            captureSession.sessionPreset = .hd1920x1080
        } else if captureSession.canSetSessionPreset(.hd1280x720) {
            captureSession.sessionPreset = .hd1280x720
        } else {
            captureSession.sessionPreset = .high
        }
        
        // Configure video output
        let output = AVCaptureVideoDataOutput()
        output.videoSettings = [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA
        ]
        output.alwaysDiscardsLateVideoFrames = true
        output.setSampleBufferDelegate(self, queue: captureQueue)
        
        if captureSession.canAddOutput(output) {
            captureSession.addOutput(output)
            videoOutput = output
        }
        
        captureSession.commitConfiguration()
        
        // Configure initial camera
        configureCamera(position: .back)
    }
    
    private func configureCamera(position: AVCaptureDevice.Position) {
        captureSession.beginConfiguration()
        
        // Remove existing input
        if let currentInput = currentInput {
            captureSession.removeInput(currentInput)
        }
        
        // Find camera
        guard let camera = findCamera(position: position) else {
            print("[CameraManager] ERROR: Camera not found for position: \(position)")
            captureSession.commitConfiguration()
            return
        }
        
        // Create input
        do {
            let input = try AVCaptureDeviceInput(device: camera)
            
            if captureSession.canAddInput(input) {
                captureSession.addInput(input)
                currentInput = input
                cameraPosition = position
                
                // Get actual capture dimensions
                let formatDescription = camera.activeFormat.formatDescription
                let dimensions = CMVideoFormatDescriptionGetDimensions(formatDescription)
                captureSize = CGSize(width: CGFloat(dimensions.width), height: CGFloat(dimensions.height))
                
                print("[CameraManager] Configured camera: \(camera.localizedName) at \(captureSize)")
            }
        } catch {
            print("[CameraManager] ERROR: Failed to create camera input: \(error)")
        }
        
        captureSession.commitConfiguration()
    }
    
    private func findCamera(position: AVCaptureDevice.Position) -> AVCaptureDevice? {
        // Try to find the best camera for the position
        let discoverySession = AVCaptureDevice.DiscoverySession(
            deviceTypes: [.builtInWideAngleCamera, .builtInDualCamera, .builtInTripleCamera],
            mediaType: .video,
            position: position
        )
        
        return discoverySession.devices.first
    }
}

// MARK: - AVCaptureVideoDataOutputSampleBufferDelegate

extension CameraManager: AVCaptureVideoDataOutputSampleBufferDelegate {
    
    public func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer, from connection: AVCaptureConnection) {
        guard let pixelBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        
        // Lock the pixel buffer
        CVPixelBufferLockBaseAddress(pixelBuffer, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(pixelBuffer, .readOnly) }
        
        // Get dimensions
        let width = CVPixelBufferGetWidth(pixelBuffer)
        let height = CVPixelBufferGetHeight(pixelBuffer)
        let bytesPerRow = CVPixelBufferGetBytesPerRow(pixelBuffer)
        
        // Get pixel data
        guard let baseAddress = CVPixelBufferGetBaseAddress(pixelBuffer) else { return }
        
        // Calculate timestamp
        let timestamp: TimeInterval
        if let start = startTime {
            timestamp = Date().timeIntervalSince(start)
        } else {
            timestamp = 0
        }
        
        // Create data - need to copy row by row if bytesPerRow != width * 4
        let data: Data
        let expectedBytesPerRow = width * 4
        
        if bytesPerRow == expectedBytesPerRow {
            // Direct copy
            data = Data(bytes: baseAddress, count: height * bytesPerRow)
        } else {
            // Copy row by row (handle stride)
            var packedData = Data(capacity: width * height * 4)
            let srcPtr = baseAddress.assumingMemoryBound(to: UInt8.self)
            
            for row in 0..<height {
                let rowStart = srcPtr + (row * bytesPerRow)
                packedData.append(rowStart, count: expectedBytesPerRow)
            }
            data = packedData
        }
        
        // Create video frame
        let frame = VideoFrame(
            width: UInt32(width),
            height: UInt32(height),
            format: .bgra,
            flags: FrameFlags(),
            timestamp: timestamp,
            data: data
        )
        
        frameCount += 1
        
        // Notify callback
        onFrameCaptured?(frame)
    }
    
    public func captureOutput(_ output: AVCaptureOutput, didDrop sampleBuffer: CMSampleBuffer, from connection: AVCaptureConnection) {
        // Frame was dropped - this is normal under high load
    }
}

