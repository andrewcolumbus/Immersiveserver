# Immersive Player

A cross-platform (Windows/macOS) media player built in Rust for GPU-accelerated HAP video playback with advanced projection output features.

## Features

- **HAP Video Playback**: GPU-accelerated decoding of HAP, HAP Alpha, and HAP Q video formats
- **Multi-Output Support**: Route video to multiple screens/projectors
- **Edge Blending**: Soft-edge blending for seamless multi-projector setups
- **Geometric Warping**: Perspective and Bezier grid warping for projection surface alignment
- **Masking**: Soft and hard masks for complex output shapes
- **Preset System**: Save and load complete output configurations

## Requirements

- Rust 1.70 or later
- GPU with Vulkan (Windows/Linux) or Metal (macOS) support
- For HAP video files: .mov, .avi, or .mp4 containers with HAP codec

## Building

```bash
# Clone the repository
cd immersive-player

# Build in release mode
cargo build --release

# Run the application
cargo run --release
```

## Usage

### Basic Playback

1. Launch the application
2. Click "Open File" to load a HAP video
3. Use transport controls to play/pause/seek
4. Adjust playback speed and loop mode as needed

### Multi-Projector Setup

1. Click "Add Screen" or "Dual Setup" to create outputs
2. Select a screen to configure its properties
3. Enable edge blending in the Blend panel
4. Adjust blend width, power curve, and black level
5. Use the Warp panel for geometric correction

### Test Patterns

- Click "Test Pattern" to generate a test signal
- Enable "Test Pattern" on individual screens
- Use test patterns for projector alignment

### Saving/Loading Presets

- Projects are saved as `.ron` files (Rusty Object Notation)
- Save complete configurations including all screens, slices, and blend settings
- Load presets to quickly restore venue setups

## HAP Video Format

HAP is a video codec that uses GPU-native texture compression:

| Variant | Format | Use Case |
|---------|--------|----------|
| HAP | BC1 (DXT1) | RGB video, smallest files |
| HAP Alpha | BC3 (DXT5) | RGBA with alpha channel |
| HAP Q | BC7 | Highest quality, larger files |

To create HAP videos, use:
- [FFmpeg](https://ffmpeg.org/) with HAP encoder
- [Adobe Media Encoder](https://www.adobe.com/products/media-encoder.html) with AfterCodecs
- [Resolume](https://resolume.com/) Alley

## Edge Blending

The edge blend system uses power curves for natural falloff:

```
blend_factor = pow(t, power) * (1 - black_level) + black_level * t
```

Parameters:
- **Width**: Overlap region in pixels
- **Power**: Curve steepness (2.0-2.5 typical)
- **Gamma**: Color correction for projector response
- **Black Level**: Compensates for projector black point

## HAP Converter

The built-in HAP Converter allows you to convert various video formats to HAP directly within the application.

### Accessing the Converter

1. From the menu bar, select **Tools → HAP Converter**
2. A separate window will open for batch video conversion

### Supported Input Formats

- H.264/AVC (.mp4, .mov, .avi, .mkv)
- H.265/HEVC
- ProRes (.mov)
- DNxHD/DNxHR (.mxf, .mov)
- DXV
- VP9 (.webm)
- Any format supported by FFmpeg

### Output HAP Variants

| Variant | Use Case | Description |
|---------|----------|-------------|
| HAP | RGB video | Smallest files (DXT1 compression) |
| HAP Alpha | RGBA with transparency | For overlays and compositing (DXT5) |
| HAP Q | Highest quality | Best visual quality, larger files (BC7) |

### Quality Presets

| Preset | Speed | File Size |
|--------|-------|-----------|
| Fast | Fastest | Largest |
| Balanced | Moderate | Medium |
| Quality | Slowest | Smallest |

### Using the Converter

1. Click **Add Files** or drag-and-drop videos into the window
2. Set the output folder using **Set Output Folder**
3. Select the desired HAP format (HAP, HAP Alpha, or HAP Q)
4. Choose a quality preset
5. Click **Start Conversion** to begin

The converter processes files sequentially and shows real-time progress with ETA.

### FFmpeg Requirement

The HAP Converter requires FFmpeg to be installed on your system.

#### Installing FFmpeg

**macOS (Homebrew):**
```bash
brew install ffmpeg
```

**macOS (MacPorts):**
```bash
sudo port install ffmpeg
```

**Windows:**
1. Download FFmpeg from https://ffmpeg.org/download.html
2. Extract to `C:\ffmpeg`
3. Add `C:\ffmpeg\bin` to your system PATH

**Linux (Ubuntu/Debian):**
```bash
sudo apt update && sudo apt install ffmpeg
```

**Linux (Fedora):**
```bash
sudo dnf install ffmpeg
```

#### Bundling FFmpeg (Optional)

For portable installations, you can place FFmpeg binaries in the `assets/ffmpeg/` folder:

```
assets/ffmpeg/
├── ffmpeg-macos    # macOS binary
├── ffmpeg.exe      # Windows binary
└── ffprobe         # Optional, for video info
```

The application searches for FFmpeg in this order:
1. `assets/ffmpeg/` (bundled)
2. System PATH
3. Common installation directories

## Architecture

```
src/
├── main.rs          # Application entry point
├── app.rs           # Main application state
├── converter/       # HAP video converter
├── video/           # HAP decoder and playback
├── output/          # Screens, slices, blending, warping
├── render/          # GPU rendering pipeline
├── ui/              # egui interface components
└── project/         # Save/load presets
```

## Dependencies

- **wgpu**: Cross-platform GPU abstraction
- **egui/eframe**: Immediate-mode GUI
- **glam**: Vector/matrix math
- **serde/ron**: Serialization for presets

## Building macOS Installer

Immersive Player includes a build script to create a signed macOS pkg installer.

### Prerequisites

- Xcode Command Line Tools (`xcode-select --install`)
- Developer ID Application certificate in Keychain
- Developer ID Installer certificate in Keychain
- FFmpeg binaries (optional, for bundling)

### Building the Installer

```bash
cd immersive-player/installer
./build-pkg.sh
```

The script will:
1. Build the release binary for your architecture
2. Create the `.app` bundle with all assets
3. Bundle FFmpeg if available
4. Code sign the application
5. Create and sign the pkg installer

### Environment Variables

| Variable | Description |
|----------|-------------|
| `DEVELOPER_ID_APPLICATION` | Certificate name for signing the app |
| `DEVELOPER_ID_INSTALLER` | Certificate name for signing the pkg |
| `APPLE_ID` | Apple ID for notarization (optional) |
| `APPLE_PASSWORD` | App-specific password for notarization |
| `APPLE_TEAM_ID` | Team ID for notarization |
| `SKIP_NOTARIZATION` | Set to "1" to skip notarization |
| `KEEP_BUILD` | Set to "1" to keep build artifacts |

### Notarization

For distribution outside the Mac App Store, notarization is recommended:

```bash
export APPLE_ID="your@email.com"
export APPLE_PASSWORD="xxxx-xxxx-xxxx-xxxx"  # App-specific password
export APPLE_TEAM_ID="XXXXXXXXXX"
./build-pkg.sh
```

### Output

The installer will be created at:
```
installer/output/ImmersivePlayer-{version}.pkg
```

### Bundling FFmpeg

To include FFmpeg in the installer, either:
1. Place static FFmpeg binaries in `assets/ffmpeg/ffmpeg` and `assets/ffmpeg/ffprobe`
2. Or install FFmpeg via Homebrew (`brew install ffmpeg`) - the script will copy system FFmpeg

## License

MIT License - see LICENSE file for details.

## Contributing

Contributions welcome! Please open an issue or pull request.

