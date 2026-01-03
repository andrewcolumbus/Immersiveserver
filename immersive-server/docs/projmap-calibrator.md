# ProjMap Calibrator

A standalone Rust application for automated projection mapping calibration using structured light and multi-projector edge blending.

## Overview

ProjMap Calibrator provides automated camera-projector calibration using Gray code structured light patterns. It captures patterns via NDI camera input, computes homography transformations using OpenCV, and generates edge blend masks for seamless multi-projector setups.

## Features

- **NDI Camera Input** - Receive video from any NDI-compatible camera
- **Gray Code Structured Light** - GPU-accelerated pattern generation and decoding
- **OpenCV Homography** - RANSAC-based camera-to-projector transformation
- **Multi-Projector Support** - Unlimited projectors with automatic overlap detection
- **Edge Blend Masks** - Configurable blend curves (Linear, Gamma, Cosine, Smoothstep)
- **Export** - PNG blend masks (8-bit/16-bit), XML/JSON project files

## Requirements

### macOS

```bash
# Install OpenCV 4.x
brew install opencv

# NDI SDK must be installed at /Library/NDI SDK for Apple/
```

### Build

```bash
cd projmap-calibrator
cargo build --release
```

## Usage

```bash
cargo run
```

### Workflow

1. **Connect Camera** - Select an NDI source from the dropdown
2. **Configure Projectors** - Add projectors and set their resolution
3. **Calibrate** - Click "Start Calibration" to begin pattern projection and capture
4. **Detect Overlaps** - After calibration, click "Detect Overlaps" to find overlap regions
5. **Export** - Export blend masks as PNG images

## Architecture

### Calibration Pipeline

```
NDI Camera → Frame Capture → Pattern Decode → Homography → Blend Masks
     ↑                                              ↓
Projector ← Pattern Display ← Gray Code Generator
```

### Pattern Sequence

For a 1920x1080 projector:
- 11 horizontal bits × 2 (positive + inverted) = 22 patterns
- 11 vertical bits × 2 = 22 patterns
- 2 reference patterns (white, black)
- **Total: 46 patterns per projector**

### Timing

- **Settle Time**: 100ms (projector stabilization)
- **Frame Averaging**: 3 frames per pattern (noise reduction)
- **Total Time**: ~7 seconds per projector

## File Structure

```
projmap-calibrator/
├── Cargo.toml
├── .cargo/config.toml          # NDI SDK + OpenCV paths
└── src/
    ├── main.rs                 # Entry point
    ├── app.rs                  # Application state & egui UI
    ├── calibration/
    │   ├── gray_code.rs        # Pattern generation
    │   ├── decoder.rs          # Correspondence storage
    │   ├── session.rs          # Workflow state machine
    │   └── homography.rs       # OpenCV RANSAC homography
    ├── camera/
    │   ├── ndi_ffi.rs          # NDI SDK FFI bindings
    │   └── ndi_input.rs        # Background NDI receiver
    ├── blending/
    │   ├── mod.rs              # Blend mask generation
    │   └── overlap.rs          # Overlap auto-detection
    ├── render/
    │   ├── pipeline.rs         # wgpu render pipeline
    │   ├── pattern.rs          # Gray code shader renderer
    │   ├── preview.rs          # Camera preview renderer
    │   └── shaders/
    │       ├── gray_code.wgsl  # Pattern generation shader
    │       ├── preview.wgsl    # Camera preview shader
    │       └── edge_blend.wgsl # Blend application shader
    ├── export/mod.rs           # PNG/XML/JSON export
    ├── config/mod.rs           # Project configuration
    └── ui/mod.rs               # UI state
```

## Configuration

### Calibration Config

```rust
CalibrationConfig {
    settle_time: Duration::from_millis(100),  // Projector stabilization
    frames_to_average: 3,                      // Noise reduction
    contrast_threshold: 0.1,                   // Valid pixel threshold
    camera_width: 1920,
    camera_height: 1080,
}
```

### Overlap Detection Config

```rust
OverlapConfig {
    min_overlap_width: 10,           // Minimum pixels to consider overlap
    blend_curve: BlendCurve::Smoothstep,
    padding: 0,                      // Additional overlap padding
}
```

### Homography Config

```rust
HomographyComputer {
    ransac_threshold: 3.0,    // Reprojection error threshold (pixels)
    max_iters: 2000,          // Maximum RANSAC iterations
    confidence: 0.995,        // Confidence level
    min_points: 100,          // Minimum correspondences required
    sample_stride: 4,         // Sampling stride for efficiency
}
```

## Blend Curves

| Curve | Formula | Use Case |
|-------|---------|----------|
| Linear | `t` | Simple falloff |
| Gamma | `t^2.2` | Perceptually linear |
| Cosine | `0.5 - 0.5*cos(πt)` | Smooth S-curve |
| Smoothstep | `3t² - 2t³` | Very smooth transitions |

## Export Formats

### Blend Masks (PNG)

- 8-bit grayscale: Standard compatibility
- 16-bit grayscale: Higher precision for professional use

### Project File (.projmap)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<ProjectMapCalibration version="1">
    <name>Venue Name</name>
    <canvasWidth>3840</canvasWidth>
    <canvasHeight>1080</canvasHeight>
    <projectors>
        <projector id="1">
            <name>Projector Left</name>
            <resolution width="1920" height="1200"/>
            <homography>1.02, -0.001, 15.2, ...</homography>
            <blendRight width="200" gamma="2.2" curve="cosine"/>
        </projector>
    </projectors>
    <overlaps>
        <overlap projectorA="1" projectorB="2" edge="right" pixels="200"/>
    </overlaps>
</ProjectMapCalibration>
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Escape` | Exit application |
| `Space` | Start/stop calibration |

## Troubleshooting

### OpenCV not found

```bash
# Ensure OpenCV is installed and pkg-config path is set
export PKG_CONFIG_PATH="/opt/homebrew/opt/opencv/lib/pkgconfig"
```

### NDI sources not appearing

1. Ensure NDI SDK is installed at `/Library/NDI SDK for Apple/`
2. Check that NDI sources are on the same network
3. Verify firewall allows NDI traffic (port 5353 for mDNS)

### Low inlier count in homography

- Increase camera exposure to reduce motion blur
- Ensure projector is in focus
- Reduce ambient light
- Check that camera can see the full projected area

## Tests

```bash
cargo test
```

Tests include:
- Gray code conversion (binary ↔ gray)
- Pattern count calculation
- Homography identity transform
- Matrix inversion
- Overlap bounds computation
- Edge detection (left/right)

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| wgpu | 24 | GPU rendering |
| winit | 0.30 | Window management |
| egui | 0.31 | Immediate mode GUI |
| opencv | 0.92 | Computer vision |
| image | 0.25 | Image I/O |
| tokio | 1 | Async runtime |

## Integration with Immersive Server

The calibration data exported by ProjMap Calibrator can be used with Immersive Server for:

1. **Mesh Warp** - Apply homography transformation to video layers
2. **Edge Blending** - Load PNG blend masks as layer masks
3. **Multi-Output** - Configure multiple projector outputs with proper alignment

See the Immersive Server documentation for details on importing calibration data.
