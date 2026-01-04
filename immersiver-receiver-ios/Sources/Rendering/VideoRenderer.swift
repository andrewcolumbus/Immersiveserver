import Foundation
import Metal
import MetalKit
import UIKit
import simd

/// Metal-based video renderer for displaying BGRA frames with zoom/pan support
public final class VideoRenderer: NSObject, MTKViewDelegate, @unchecked Sendable {
    /// The Metal device
    private let device: MTLDevice
    
    /// Command queue for GPU commands
    private let commandQueue: MTLCommandQueue
    
    /// Render pipeline for drawing textures
    private let pipelineState: MTLRenderPipelineState
    
    /// Sampler for texture sampling
    private let sampler: MTLSamplerState
    
    /// Current video texture (thread-safe)
    private let textureStorage = TextureStorage()
    
    /// Video dimensions
    private let dimensionsStorage = DimensionsStorage()
    
    /// Zoom and pan parameters (thread-safe)
    private let transformStorage = TransformStorage()
    
    /// Statistics
    public var renderCount: Int { renderCountStorage.value }
    private let renderCountStorage = AtomicCounter()
    
    /// Private initializer - use create() factory method
    private init(device: MTLDevice, commandQueue: MTLCommandQueue, pipelineState: MTLRenderPipelineState, sampler: MTLSamplerState) {
        self.device = device
        self.commandQueue = commandQueue
        self.pipelineState = pipelineState
        self.sampler = sampler
        super.init()
    }
    
    /// Factory method to create a VideoRenderer
    /// Returns nil if Metal is not available
    public static func create() -> VideoRenderer? {
        print("[VideoRenderer] Initializing Metal...")
        
        guard let device = MTLCreateSystemDefaultDevice() else {
            print("[VideoRenderer] ERROR: Failed to create Metal device")
            return nil
        }
        print("[VideoRenderer] Metal device: \(device.name)")
        
        guard let commandQueue = device.makeCommandQueue() else {
            print("[VideoRenderer] ERROR: Failed to create command queue")
            return nil
        }
        
        // Create pipeline from source
        guard let (pipeline, samp) = createPipelineFromSource(device: device) else {
            print("[VideoRenderer] ERROR: Failed to create render pipeline")
            return nil
        }
        
        print("[VideoRenderer] Successfully initialized Metal renderer")
        return VideoRenderer(device: device, commandQueue: commandQueue, pipelineState: pipeline, sampler: samp)
    }
    
    /// Create an MTKView configured for video display
    public func createView() -> MTKView {
        let view = MTKView()
        view.device = device
        view.delegate = self
        view.colorPixelFormat = .bgra8Unorm
        view.framebufferOnly = true
        view.isPaused = false
        view.enableSetNeedsDisplay = false
        view.preferredFramesPerSecond = 60
        view.layer.isOpaque = true
        
        return view
    }
    
    /// Update the video texture with a new frame
    public func updateTexture(with frame: VideoFrame) {
        guard frame.format == .bgra else {
            // Only BGRA is currently supported
            return
        }
        
        let width = Int(frame.width)
        let height = Int(frame.height)
        
        // Create or reuse texture
        let texture = getOrCreateTexture(width: width, height: height)
        
        // Upload frame data to texture
        let region = MTLRegion(origin: MTLOrigin(x: 0, y: 0, z: 0),
                               size: MTLSize(width: width, height: height, depth: 1))
        
        frame.data.withUnsafeBytes { ptr in
            texture.replace(region: region,
                           mipmapLevel: 0,
                           withBytes: ptr.baseAddress!,
                           bytesPerRow: width * 4)
        }
        
        textureStorage.texture = texture
        dimensionsStorage.width = width
        dimensionsStorage.height = height
    }
    
    /// Current video dimensions
    public var videoDimensions: (width: Int, height: Int) {
        (dimensionsStorage.width, dimensionsStorage.height)
    }
    
    /// Update zoom scale (1.0 = normal, > 1.0 = zoomed in)
    public func setZoom(_ scale: Float) {
        transformStorage.zoom = max(0.5, min(scale, 5.0))
    }
    
    /// Update pan offset (normalized -1 to 1)
    public func setPan(x: Float, y: Float) {
        transformStorage.panX = x
        transformStorage.panY = y
    }
    
    /// Reset zoom and pan to defaults
    public func resetTransform() {
        transformStorage.zoom = 1.0
        transformStorage.panX = 0.0
        transformStorage.panY = 0.0
    }
    
    // MARK: - MTKViewDelegate
    
    public func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {
        // Handle resize if needed
    }
    
    public func draw(in view: MTKView) {
        guard let texture = textureStorage.texture,
              let drawable = view.currentDrawable,
              let renderPassDesc = view.currentRenderPassDescriptor,
              let commandBuffer = commandQueue.makeCommandBuffer(),
              let encoder = commandBuffer.makeRenderCommandEncoder(descriptor: renderPassDesc) else {
            return
        }
        
        // Set up render state
        encoder.setRenderPipelineState(pipelineState)
        encoder.setFragmentTexture(texture, index: 0)
        encoder.setFragmentSamplerState(sampler, index: 0)
        
        // Get transform parameters
        let zoom = transformStorage.zoom
        let panX = transformStorage.panX
        let panY = transformStorage.panY
        
        // Calculate aspect-fit vertices with zoom/pan
        let viewAspect = Float(view.drawableSize.width / view.drawableSize.height)
        let textureAspect = Float(texture.width) / Float(texture.height)
        
        var scaleX: Float = 1.0
        var scaleY: Float = 1.0
        
        if textureAspect > viewAspect {
            // Texture is wider - fit to width
            scaleY = viewAspect / textureAspect
        } else {
            // Texture is taller - fit to height
            scaleX = textureAspect / viewAspect
        }
        
        // Apply zoom
        scaleX *= zoom
        scaleY *= zoom
        
        // Full-screen quad vertices with aspect correction and pan
        let vertices: [Float] = [
            -scaleX + panX, -scaleY + panY, 0.0, 1.0,  // bottom-left
             scaleX + panX, -scaleY + panY, 1.0, 1.0,  // bottom-right
            -scaleX + panX,  scaleY + panY, 0.0, 0.0,  // top-left
             scaleX + panX,  scaleY + panY, 1.0, 0.0   // top-right
        ]
        
        encoder.setVertexBytes(vertices, length: vertices.count * MemoryLayout<Float>.stride, index: 0)
        encoder.drawPrimitives(type: .triangleStrip, vertexStart: 0, vertexCount: 4)
        encoder.endEncoding()
        
        commandBuffer.present(drawable)
        commandBuffer.commit()
        
        renderCountStorage.increment()
    }
    
    // MARK: - Private Methods
    
    private func getOrCreateTexture(width: Int, height: Int) -> MTLTexture {
        // Check if existing texture can be reused
        if let existing = textureStorage.texture,
           existing.width == width && existing.height == height {
            return existing
        }
        
        // Create new texture
        let descriptor = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .bgra8Unorm,
            width: width,
            height: height,
            mipmapped: false
        )
        descriptor.usage = [.shaderRead]
        descriptor.storageMode = .shared
        
        return device.makeTexture(descriptor: descriptor)!
    }
    
    private static func createSampler(device: MTLDevice) -> MTLSamplerState? {
        let descriptor = MTLSamplerDescriptor()
        descriptor.minFilter = .linear
        descriptor.magFilter = .linear
        descriptor.mipFilter = .notMipmapped
        descriptor.sAddressMode = .clampToEdge
        descriptor.tAddressMode = .clampToEdge
        return device.makeSamplerState(descriptor: descriptor)
    }
    
    private static func createPipelineFromSource(device: MTLDevice) -> (MTLRenderPipelineState, MTLSamplerState)? {
        // Metal Shading Language source
        let shaderSource = """
        #include <metal_stdlib>
        using namespace metal;
        
        struct VertexOut {
            float4 position [[position]];
            float2 texCoord;
        };
        
        vertex VertexOut videoVertexShader(uint vertexID [[vertex_id]],
                                           constant float4* vertices [[buffer(0)]]) {
            VertexOut out;
            float4 vtx = vertices[vertexID];
            out.position = float4(vtx.xy, 0.0, 1.0);
            out.texCoord = vtx.zw;
            return out;
        }
        
        fragment float4 videoFragmentShader(VertexOut in [[stage_in]],
                                            texture2d<float> videoTexture [[texture(0)]],
                                            sampler videoSampler [[sampler(0)]]) {
            return videoTexture.sample(videoSampler, in.texCoord);
        }
        """
        
        do {
            print("[VideoRenderer] Compiling Metal shaders...")
            let compileOptions = MTLCompileOptions()
            compileOptions.fastMathEnabled = true
            
            let library = try device.makeLibrary(source: shaderSource, options: compileOptions)
            print("[VideoRenderer] Shader library created")
            
            guard let vertexFunc = library.makeFunction(name: "videoVertexShader") else {
                print("[VideoRenderer] ERROR: Could not find videoVertexShader function")
                return nil
            }
            
            guard let fragmentFunc = library.makeFunction(name: "videoFragmentShader") else {
                print("[VideoRenderer] ERROR: Could not find videoFragmentShader function")
                return nil
            }
            
            let pipelineDesc = MTLRenderPipelineDescriptor()
            pipelineDesc.vertexFunction = vertexFunc
            pipelineDesc.fragmentFunction = fragmentFunc
            pipelineDesc.colorAttachments[0].pixelFormat = .bgra8Unorm
            
            print("[VideoRenderer] Creating render pipeline state...")
            let pipeline = try device.makeRenderPipelineState(descriptor: pipelineDesc)
            
            guard let sampler = createSampler(device: device) else {
                print("[VideoRenderer] ERROR: Failed to create sampler")
                return nil
            }
            
            print("[VideoRenderer] Pipeline created successfully")
            return (pipeline, sampler)
        } catch {
            print("[VideoRenderer] ERROR: Failed to create shader: \(error)")
            return nil
        }
    }
}

// MARK: - Thread-Safe Storage

private final class TextureStorage: @unchecked Sendable {
    private let lock = NSLock()
    private var _texture: MTLTexture?
    
    var texture: MTLTexture? {
        get {
            lock.lock()
            defer { lock.unlock() }
            return _texture
        }
        set {
            lock.lock()
            defer { lock.unlock() }
            _texture = newValue
        }
    }
}

private final class DimensionsStorage: @unchecked Sendable {
    private let lock = NSLock()
    private var _width: Int = 0
    private var _height: Int = 0
    
    var width: Int {
        get {
            lock.lock()
            defer { lock.unlock() }
            return _width
        }
        set {
            lock.lock()
            defer { lock.unlock() }
            _width = newValue
        }
    }
    
    var height: Int {
        get {
            lock.lock()
            defer { lock.unlock() }
            return _height
        }
        set {
            lock.lock()
            defer { lock.unlock() }
            _height = newValue
        }
    }
}

private final class TransformStorage: @unchecked Sendable {
    private let lock = NSLock()
    private var _zoom: Float = 1.0
    private var _panX: Float = 0.0
    private var _panY: Float = 0.0
    
    var zoom: Float {
        get {
            lock.lock()
            defer { lock.unlock() }
            return _zoom
        }
        set {
            lock.lock()
            defer { lock.unlock() }
            _zoom = newValue
        }
    }
    
    var panX: Float {
        get {
            lock.lock()
            defer { lock.unlock() }
            return _panX
        }
        set {
            lock.lock()
            defer { lock.unlock() }
            _panX = newValue
        }
    }
    
    var panY: Float {
        get {
            lock.lock()
            defer { lock.unlock() }
            return _panY
        }
        set {
            lock.lock()
            defer { lock.unlock() }
            _panY = newValue
        }
    }
}

private final class AtomicCounter: @unchecked Sendable {
    private let lock = NSLock()
    private var _value: Int = 0
    
    var value: Int {
        lock.lock()
        defer { lock.unlock() }
        return _value
    }
    
    func increment() {
        lock.lock()
        defer { lock.unlock() }
        _value += 1
    }
}


