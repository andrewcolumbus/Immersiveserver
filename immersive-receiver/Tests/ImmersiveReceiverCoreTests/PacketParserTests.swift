import XCTest
@testable import ImmersiveReceiverCore

final class PacketParserTests: XCTestCase {
    
    func testVideoFrameModel() {
        let data = Data(repeating: 0xFF, count: 1920 * 1080 * 4)
        let frame = VideoFrame(
            width: 1920,
            height: 1080,
            format: .bgra,
            flags: FrameFlags(),
            timestamp: 1.5,
            data: data
        )
        
        XCTAssertEqual(frame.width, 1920)
        XCTAssertEqual(frame.height, 1080)
        XCTAssertEqual(frame.format, .bgra)
        XCTAssertEqual(frame.expectedDataSize, 1920 * 1080 * 4)
    }
    
    func testSourceInfo() {
        let source = SourceInfo(
            id: "test-1",
            name: "Test Sender",
            host: "192.168.1.100",
            port: 9000
        )
        
        XCTAssertEqual(source.address, "192.168.1.100:9000")
        XCTAssertEqual(source.displayName, "Test Sender")
    }
    
    func testSourceInfoEmptyName() {
        let source = SourceInfo(
            id: "test-1",
            name: "",
            host: "192.168.1.100",
            port: 9000
        )
        
        XCTAssertEqual(source.displayName, "192.168.1.100:9000")
    }
    
    func testConnectionState() {
        let disconnected = ConnectionState.disconnected
        XCTAssertFalse(disconnected.isConnected)
        XCTAssertEqual(disconnected.displayText, "Disconnected")
        
        let source = SourceInfo(id: "1", name: "Test", host: "127.0.0.1", port: 9000)
        let connected = ConnectionState.connected(source: source)
        XCTAssertTrue(connected.isConnected)
        
        let error = ConnectionState.error("Network error")
        XCTAssertFalse(error.isConnected)
        XCTAssertEqual(error.displayText, "Error: Network error")
    }
    
    func testDataReadBigEndian() {
        // Test UInt32 big endian reading
        let data = Data([0x00, 0x00, 0x07, 0x80]) // 1920 in BE
        XCTAssertEqual(data.readUInt32BE(at: 0), 1920)
        
        // Test UInt64 big endian reading
        let data64 = Data([0x00, 0x00, 0x00, 0x00, 0x00, 0x0F, 0x42, 0x40]) // 1000000 in BE
        XCTAssertEqual(data64.readUInt64BE(at: 0), 1000000)
    }
}






