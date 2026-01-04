# Immersive Receiver

A native SwiftUI application for macOS and iOS that receives and displays video streams from Aqueduct/OMT sources.

## Features

- **Source Discovery**: Automatically discovers Aqueduct video sources on the local network via mDNS/Bonjour
- **Feed Selection**: Choose which video feed to display from available sources
- **Remote Control**: Controllable by immersive-player via TCP control channel
- **Receiver Broadcasting**: Announces itself on the network so immersive-player can discover and control it
- **Metal Rendering**: GPU-accelerated video display using Metal

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Immersive Receiver                        │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐                   │
│  │ Source Discovery │  │ Receiver        │                   │
│  │ (_omt._tcp)      │  │ Broadcast       │                   │
│  │                  │  │ (_omt-receiver) │                   │
│  └────────┬─────────┘  └────────┬────────┘                   │
│           │                     │                            │
│  ┌────────▼─────────────────────▼────────┐                   │
│  │           Video Receiver              │◄── TCP:9000       │
│  │         (Packet Parser + LZ4)         │   (Video Stream)  │
│  └────────────────┬──────────────────────┘                   │
│                   │                                          │
│  ┌────────────────▼──────────────────────┐                   │
│  │          Metal Video Renderer         │                   │
│  └───────────────────────────────────────┘                   │
│                                                              │
│  ┌───────────────────────────────────────┐                   │
│  │          Control Server               │◄── TCP:9001       │
│  │         (JSON Commands)               │   (Control)       │
│  └───────────────────────────────────────┘                   │
└─────────────────────────────────────────────────────────────┘
```

## Control Protocol

The receiver listens on TCP port 9001 for JSON control commands:

### Commands (from immersive-player)

```json
{"type": "switch_feed", "source_addr": "192.168.1.50:9000"}
{"type": "get_status"}
{"type": "disconnect"}
```

### Responses (from receiver)

```json
{"type": "status", "name": "Living Room Display", "connected_source": "192.168.1.50:9000", "resolution": "1920x1080", "fps": 30}
{"type": "ack", "success": true}
{"type": "error", "message": "Failed to connect to source"}
```

## Building

### Requirements

- Xcode 15.0+
- macOS 14.0+ / iOS 17.0+
- Swift 5.9+

### Build Commands

```bash
# Build for macOS
swift build

# Run on macOS
swift run ImmersiveReceiverMac

# Build for release
swift build -c release
```

### Xcode Project

For iOS development or full Xcode integration:

```bash
# Generate Xcode project
swift package generate-xcodeproj

# Or open in Xcode directly
open Package.swift
```

## Dependencies

- **swift-lz4**: LZ4 decompression for video frames
- **Network.framework**: Apple's modern networking framework for TCP connections
- **Metal**: GPU-accelerated video rendering
- **Bonjour/mDNS**: Service discovery via NWBrowser

## License

MIT License - See LICENSE file for details.







