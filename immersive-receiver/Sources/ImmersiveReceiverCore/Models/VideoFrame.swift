import Foundation

/// Pixel format matching Aqueduct protocol
public enum PixelFormat: UInt8, Sendable {
    case uyvy = 0   // 4:2:2
    case uyva = 1   // 4:2:2:4
    case bgra = 2   // 4:4:4
    case nv12 = 3   // 4:2:0 Planar
    case yv12 = 4   // 4:2:0 Planar
    case p216 = 5   // Planar 4:2:2 16-bit
    case pa16 = 6   // Planar 4:2:2:4 16-bit
}

/// Frame flags matching Aqueduct protocol
public struct FrameFlags: Sendable {
    public var alpha: Bool
    public var premultiplied: Bool
    public var highBitDepth: Bool
    
    public init(alpha: Bool = false, premultiplied: Bool = false, highBitDepth: Bool = false) {
        self.alpha = alpha
        self.premultiplied = premultiplied
        self.highBitDepth = highBitDepth
    }
}

/// Video frame received from Aqueduct stream
public struct VideoFrame: Sendable {
    public let width: UInt32
    public let height: UInt32
    public let format: PixelFormat
    public let flags: FrameFlags
    public let timestamp: TimeInterval
    public let data: Data
    
    public init(width: UInt32, height: UInt32, format: PixelFormat, flags: FrameFlags, timestamp: TimeInterval, data: Data) {
        self.width = width
        self.height = height
        self.format = format
        self.flags = flags
        self.timestamp = timestamp
        self.data = data
    }
    
    /// Expected data size for BGRA format
    public var expectedDataSize: Int {
        Int(width) * Int(height) * 4
    }
}

/// Audio frame received from Aqueduct stream
public struct AudioFrame: Sendable {
    public let sampleRate: UInt32
    public let channels: UInt32
    public let timestamp: TimeInterval
    public let data: Data
    
    public init(sampleRate: UInt32, channels: UInt32, timestamp: TimeInterval, data: Data) {
        self.sampleRate = sampleRate
        self.channels = channels
        self.timestamp = timestamp
        self.data = data
    }
}

/// Metadata frame received from Aqueduct stream
public struct MetadataFrame: Sendable {
    public let timestamp: TimeInterval
    public let content: String
    
    public init(timestamp: TimeInterval, content: String) {
        self.timestamp = timestamp
        self.content = content
    }
}

/// Packet types in the Aqueduct protocol
public enum Packet: Sendable {
    case video(VideoFrame)
    case audio(AudioFrame)
    case metadata(MetadataFrame)
}




