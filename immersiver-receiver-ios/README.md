# Immersive Receiver iOS

An OMT (Open Media Transport) receiver app for iOS with fullscreen/HDMI output support and camera broadcasting capabilities.

## Features

### Video Receiver Mode
- **OMT Stream Reception**: Receives video streams from OMT sources on the local network
- **Bonjour Discovery**: Automatically discovers available OMT sources using mDNS/Bonjour
- **Manual Source Entry**: Add sources manually by IP address and port
- **Persistent Source Selection**: Selected source is remembered across app launches

### Camera Output Mode
- **OMT Source Broadcasting**: Acts as an OMT video source, broadcasting device camera
- **Bonjour Advertisement**: Advertises itself on the network for easy discovery
- **Front/Back Camera Support**: Switch between available cameras

### Display Features
- **Fullscreen Playback**: Edge-to-edge video display with hidden status bar
- **HDMI/External Display**: Automatic detection and output to connected external displays
- **Zoom & Pan**: Pinch to zoom and drag to pan when resolution differs from display
- **Aspect Ratio Preservation**: Proper letterboxing/pillarboxing for all resolutions

### User Interface
- **Transparent Overlay Menu**: Tap anywhere to reveal the control menu
- **Network Info Display**: Shows device IP address and connection status
- **Source List**: Browse and select from discovered OMT sources
- **Play/Pause Control**: Start and stop stream reception or camera broadcast
- **Resolution Display**: Shows current video resolution

## Requirements

- iOS 15.0 or later
- Device with Metal support
- Network access for OMT streaming
- Camera access for broadcast mode

## Building

1. Open `ImmersiveReceiverIOS.xcodeproj` in Xcode
2. Select your development team in Signing & Capabilities
3. Build and run on your iOS device

## Usage

### Receiving Video
1. Launch the app
2. Tap the screen to open the menu
3. Select a source from the discovered list (or add one manually)
4. Tap "Start Receiving"

### Broadcasting Camera
1. Tap the screen to open the menu
2. Switch mode to "Camera Output"
3. Tap "Start Broadcasting"
4. Connect to this device from another OMT receiver

### HDMI Output
1. Connect your iOS device to an external display via HDMI adapter or AirPlay
2. The video will automatically mirror to the external display
3. The external display runs at its native resolution

### Zoom & Pan
- Pinch to zoom in/out (0.5x to 5x)
- Drag to pan when zoomed in
- Use "Reset View" in the menu to return to default

## Architecture

```
immersiver-receiver-ios/
├── ImmersiveReceiverIOS/          # SwiftUI App
│   ├── ImmersiveReceiverApp.swift # App entry point
│   ├── AppState.swift             # Central state management
│   ├── ContentView.swift          # Main view
│   └── Views/
│       ├── VideoPlayerView.swift  # Metal video view
│       └── OverlayMenuView.swift  # Control overlay
│
└── Sources/                       # Core logic
    ├── Models/
    │   ├── SourceInfo.swift       # Source data model
    │   └── VideoFrame.swift       # Frame data model
    ├── Networking/
    │   ├── AqueductReceiver.swift # OMT receiver
    │   ├── AqueductSender.swift   # OMT sender (camera mode)
    │   └── SourceDiscovery.swift  # Bonjour discovery
    ├── Protocol/
    │   └── PacketParser.swift     # OMT packet parsing
    ├── Rendering/
    │   └── VideoRenderer.swift    # Metal renderer
    ├── Display/
    │   └── ExternalDisplayManager.swift # HDMI output
    ├── Camera/
    │   └── CameraManager.swift    # Camera capture
    └── Utilities/
        └── NetworkUtility.swift   # IP address helpers
```

## OMT Protocol

The app implements the OMT (Open Media Transport) protocol:
- TCP-based streaming
- LZ4 compression for video frames
- BGRA pixel format
- Bonjour service type: `_omt._tcp`

## License

Copyright (c) 2024. All rights reserved.

