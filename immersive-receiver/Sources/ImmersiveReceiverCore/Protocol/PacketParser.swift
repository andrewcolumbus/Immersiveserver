import Foundation
import Compression

/// Packet type identifiers matching Aqueduct protocol
private enum PacketTypeId: UInt8 {
    case video = 0x01
    case audio = 0x02
    case metadata = 0x03
}

/// Errors that can occur during packet parsing
public enum PacketParserError: Error, LocalizedError {
    case insufficientData
    case invalidPacketType(UInt8)
    case invalidPixelFormat(UInt8)
    case packetTooLarge(Int)
    case decompressFailed(String)
    case invalidUTF8
    
    public var errorDescription: String? {
        switch self {
        case .insufficientData:
            return "Insufficient data to parse packet"
        case .invalidPacketType(let type):
            return "Invalid packet type: \(type)"
        case .invalidPixelFormat(let format):
            return "Invalid pixel format: \(format)"
        case .packetTooLarge(let size):
            return "Packet too large: \(size) bytes"
        case .decompressFailed(let reason):
            return "Decompression failed: \(reason)"
        case .invalidUTF8:
            return "Invalid UTF-8 in metadata"
        }
    }
}

/// Fast synchronous packet parser - no actor overhead
/// MUST be used from a single thread (the receive queue)
public final class PacketParser: @unchecked Sendable {
    private var buffer: [UInt8]
    private let maxPacketSize: Int
    
    // Reusable decompression buffer
    private var decompressBuffer: UnsafeMutablePointer<UInt8>
    private var decompressBufferSize: Int
    
    public init(maxPacketSize: Int = 100_000_000) {
        self.buffer = []
        self.buffer.reserveCapacity(1024 * 1024) // 1MB initial
        self.maxPacketSize = maxPacketSize
        
        // Allocate reusable buffer (50MB should handle most frames)
        self.decompressBufferSize = 50_000_000
        self.decompressBuffer = UnsafeMutablePointer<UInt8>.allocate(capacity: decompressBufferSize)
    }
    
    deinit {
        decompressBuffer.deallocate()
    }
    
    /// Append received data to the buffer
    @inline(__always)
    public func appendData(_ data: Data) {
        data.withUnsafeBytes { ptr in
            if let base = ptr.baseAddress {
                buffer.append(contentsOf: UnsafeBufferPointer(start: base.assumingMemoryBound(to: UInt8.self), count: data.count))
            }
        }
    }
    
    /// Parse next packet synchronously - returns nil if not enough data
    public func parseNextPacket() -> Packet? {
        // Header is 5 bytes: Type (1) + Length (4)
        guard buffer.count >= 5 else { return nil }
        
        let typeId = buffer[0]
        let length = UInt32(buffer[1]) << 24 | UInt32(buffer[2]) << 16 | UInt32(buffer[3]) << 8 | UInt32(buffer[4])
        let payloadLength = Int(length)
        
        guard payloadLength >= 0 && payloadLength <= maxPacketSize else { return nil }
        
        let totalLength = 5 + payloadLength
        guard buffer.count >= totalLength else { return nil }
        
        // Parse based on type
        let packet: Packet?
        switch PacketTypeId(rawValue: typeId) {
        case .video:
            packet = parseVideoPacket(payloadStart: 5, payloadLength: payloadLength)
        case .audio:
            packet = parseAudioPacket(payloadStart: 5, payloadLength: payloadLength)
        case .metadata:
            packet = parseMetadataPacket(payloadStart: 5, payloadLength: payloadLength)
        case .none:
            packet = nil
        }
        
        // Remove parsed data from buffer efficiently
        if totalLength == buffer.count {
            buffer.removeAll(keepingCapacity: true)
        } else {
            buffer.removeFirst(totalLength)
        }
        
        return packet
    }
    
    /// Parse all available complete packets
    @inline(__always)
    public func parseAllPackets() -> [Packet] {
        var packets: [Packet] = []
        while let packet = parseNextPacket() {
            packets.append(packet)
        }
        return packets
    }
    
    /// Clear the buffer
    public func reset() {
        buffer.removeAll(keepingCapacity: true)
    }
    
    // MARK: - Private Parsing Methods
    
    private func parseVideoPacket(payloadStart: Int, payloadLength: Int) -> Packet? {
        // Video: [Width: u32][Height: u32][Format: u8][Timestamp: u64][CompressedData...]
        // Minimum: 4 + 4 + 1 + 8 = 17 bytes header
        guard payloadLength >= 17 else { return nil }
        
        var offset = payloadStart
        
        let width = UInt32(buffer[offset]) << 24 | UInt32(buffer[offset+1]) << 16 | UInt32(buffer[offset+2]) << 8 | UInt32(buffer[offset+3])
        offset += 4
        
        let height = UInt32(buffer[offset]) << 24 | UInt32(buffer[offset+1]) << 16 | UInt32(buffer[offset+2]) << 8 | UInt32(buffer[offset+3])
        offset += 4
        
        let formatByte = buffer[offset]
        offset += 1
        
        guard let format = PixelFormat(rawValue: formatByte) else { return nil }
        
        let timestampMicros = UInt64(buffer[offset]) << 56 | UInt64(buffer[offset+1]) << 48 | 
                              UInt64(buffer[offset+2]) << 40 | UInt64(buffer[offset+3]) << 32 |
                              UInt64(buffer[offset+4]) << 24 | UInt64(buffer[offset+5]) << 16 | 
                              UInt64(buffer[offset+6]) << 8 | UInt64(buffer[offset+7])
        offset += 8
        
        // Rest is compressed data with 4-byte size header
        let compressedStart = offset
        let compressedLength = payloadLength - 17
        
        guard compressedLength >= 4 else { return nil }
        
        // Read uncompressed size (little endian)
        let uncompressedSize = Int(buffer[compressedStart]) | 
                               Int(buffer[compressedStart+1]) << 8 | 
                               Int(buffer[compressedStart+2]) << 16 | 
                               Int(buffer[compressedStart+3]) << 24
        
        guard uncompressedSize > 0 && uncompressedSize < maxPacketSize else { return nil }
        
        // Decompress directly into reusable buffer
        let lz4DataStart = compressedStart + 4
        let lz4DataLength = compressedLength - 4
        
        // Ensure decompress buffer is big enough
        if uncompressedSize > decompressBufferSize {
            decompressBuffer.deallocate()
            decompressBufferSize = uncompressedSize + 1024*1024
            decompressBuffer = UnsafeMutablePointer<UInt8>.allocate(capacity: decompressBufferSize)
        }
        
        // Decompress using Apple's fast native decoder
        let decompressedSize = buffer.withUnsafeBufferPointer { bufferPtr in
            compression_decode_buffer(
                decompressBuffer,
                uncompressedSize,
                bufferPtr.baseAddress! + lz4DataStart,
                lz4DataLength,
                nil,
                COMPRESSION_LZ4_RAW
            )
        }
        
        // Accept any reasonable result
        let actualSize = decompressedSize > 0 ? min(decompressedSize, uncompressedSize) : uncompressedSize
        
        // Create Data from decompressed buffer
        let decompressedData: Data
        if decompressedSize > 0 {
            decompressedData = Data(bytes: decompressBuffer, count: actualSize)
        } else {
            // Decompression failed - return empty frame
            decompressedData = Data(count: uncompressedSize)
        }
        
        let frame = VideoFrame(
            width: width,
            height: height,
            format: format,
            flags: FrameFlags(),
            timestamp: TimeInterval(timestampMicros) / 1_000_000.0,
            data: decompressedData
        )
        
        return .video(frame)
    }
    
    private func parseAudioPacket(payloadStart: Int, payloadLength: Int) -> Packet? {
        // Audio: [SampleRate: u32][Channels: u32][Timestamp: u64][Data...]
        guard payloadLength >= 16 else { return nil }
        
        var offset = payloadStart
        
        let sampleRate = UInt32(buffer[offset]) << 24 | UInt32(buffer[offset+1]) << 16 | UInt32(buffer[offset+2]) << 8 | UInt32(buffer[offset+3])
        offset += 4
        
        let channels = UInt32(buffer[offset]) << 24 | UInt32(buffer[offset+1]) << 16 | UInt32(buffer[offset+2]) << 8 | UInt32(buffer[offset+3])
        offset += 4
        
        let timestampMicros = UInt64(buffer[offset]) << 56 | UInt64(buffer[offset+1]) << 48 | 
                              UInt64(buffer[offset+2]) << 40 | UInt64(buffer[offset+3]) << 32 |
                              UInt64(buffer[offset+4]) << 24 | UInt64(buffer[offset+5]) << 16 | 
                              UInt64(buffer[offset+6]) << 8 | UInt64(buffer[offset+7])
        offset += 8
        
        let audioDataLength = payloadLength - 16
        let audioData = Data(bytes: &buffer[offset], count: audioDataLength)
        
        let frame = AudioFrame(
            sampleRate: sampleRate,
            channels: channels,
            timestamp: TimeInterval(timestampMicros) / 1_000_000.0,
            data: audioData
        )
        
        return .audio(frame)
    }
    
    private func parseMetadataPacket(payloadStart: Int, payloadLength: Int) -> Packet? {
        // Metadata: [Timestamp: u64][Content...]
        guard payloadLength >= 8 else { return nil }
        
        var offset = payloadStart
        
        let timestampMicros = UInt64(buffer[offset]) << 56 | UInt64(buffer[offset+1]) << 48 | 
                              UInt64(buffer[offset+2]) << 40 | UInt64(buffer[offset+3]) << 32 |
                              UInt64(buffer[offset+4]) << 24 | UInt64(buffer[offset+5]) << 16 | 
                              UInt64(buffer[offset+6]) << 8 | UInt64(buffer[offset+7])
        offset += 8
        
        let contentLength = payloadLength - 8
        let contentData = Data(bytes: &buffer[offset], count: contentLength)
        
        guard let content = String(data: contentData, encoding: .utf8) else { return nil }
        
        let frame = MetadataFrame(
            timestamp: TimeInterval(timestampMicros) / 1_000_000.0,
            content: content
        )
        
        return .metadata(frame)
    }
}
