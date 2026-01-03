# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Immersive Server is a professional media server for GPU-accelerated video playback, compositing, and streaming. The ecosystem includes:
- **immersive-server/** - Main compositor server (Rust)
- **camera-effects/** - Real-time camera effects with ML-powered person segmentation (Rust)
- **immersive-receiver/** - macOS receiver app (Swift)
- **immersiver-receiver-ios/** - iOS receiver with camera broadcast (Swift)

## Build Commands

All commands run from `immersive-server/`:

```bash
cargo build                              # Debug build
cargo build --release                    # Release build
cargo clippy                             # Lint
cargo test                               # Run tests

# Run the main application
cargo run

# Run examples
cargo run --example decode_video         # Basic FFmpeg decoding
cargo run --example decode_to_texture    # Decode to GPU texture
cargo run --example play_video           # Full playback with rendering
```

### Prerequisites (macOS)

FFmpeg 7 required:
```bash
brew install ffmpeg@7
```

The `.cargo/config.toml` configures library paths automatically for Homebrew's FFmpeg location.

## immersive-server/ Structure

```
immersive-server/
├── src/
│   ├── main.rs              # Entry point, winit event loop, keyboard handling
│   ├── lib.rs               # Library root, public exports
│   ├── app.rs               # App struct: wgpu context, egui, render orchestration
│   ├── layer_runtime.rs     # Per-layer GPU resources (players, textures, transitions)
│   ├── settings.rs          # XML serialization for .immersive files
│   ├── compositor/          # Composition engine
│   │   ├── environment.rs   # Fixed-resolution canvas with layer vector
│   │   ├── layer.rs         # Layer definition (source, transform, blend, clips)
│   │   ├── clip.rs          # ClipCell, ClipGrid, ClipTransition
│   │   ├── blend.rs         # BlendMode enum
│   │   └── viewport.rs      # Pan/zoom with spring physics
│   ├── video/               # Video decoding & rendering
│   │   ├── decoder.rs       # FFmpeg decoder with hwaccel (VideoToolbox/D3D11VA)
│   │   ├── player.rs        # Background-threaded playback
│   │   ├── renderer.rs      # GPU pipeline for video/layer rendering
│   │   ├── texture.rs       # GPU texture management
│   │   ├── hap.rs           # HAP codec (BC1/BC3 direct upload)
│   │   └── frame.rs         # DecodedFrame struct
│   ├── network/             # Streaming & discovery
│   │   ├── omt.rs           # OmtReceiver/OmtSender via Aqueduct
│   │   ├── omt_capture.rs   # GPU readback for OMT output streaming
│   │   └── discovery.rs     # mDNS source discovery
│   ├── ui/                  # egui panels
│   │   ├── menu_bar.rs      # File/View menus, status bar
│   │   ├── dock.rs          # Docking system for panels
│   │   ├── clip_grid_panel.rs   # VJ-style clip launcher
│   │   ├── properties_panel.rs  # Environment/Layer/Clip editing
│   │   └── sources_panel.rs     # OMT/NDI source browser
│   ├── converter/           # HAP video converter tool
│   │   ├── mod.rs
│   │   └── window.rs
│   └── shaders/
│       ├── mod.rs           # Shader loading with hot-reload
│       └── fullscreen_quad.wgsl
├── examples/
│   ├── decode_video.rs      # Standalone FFmpeg decode test
│   ├── decode_to_texture.rs # Decode → GPU texture test
│   └── play_video.rs        # Full playback example
├── docs/
│   └── omt-evaluation.md    # OMT protocol evaluation notes
├── Cargo.toml
└── .cargo/config.toml       # FFmpeg library paths for macOS
```

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Escape` | Exit application |
| `F11` | Toggle fullscreen |
| `Space` | Pause/resume video |
| `R` | Restart video |
| `+` / `=` | Zoom in |
| `-` | Zoom out |
| `0` / `Home` | Reset viewport |
| `Cmd/Ctrl+S` | Save environment |

Right-click + drag pans the viewport.

## camera-effects/

Standalone camera effects application with ML-powered person segmentation and particle effects.

### Build & Run

```bash
cd camera-effects

# Build
cargo build --release

# Run (requires ONNX Runtime)
DYLD_LIBRARY_PATH=/opt/homebrew/lib cargo run --release
```

### Prerequisites

```bash
brew install onnxruntime
```

The `models/` directory must contain `selfie_segmentation.onnx` (256x256 NHWC format from PINTO Model Zoo).

### Structure

```
camera-effects/
├── src/
│   ├── main.rs              # Entry point, winit event loop
│   ├── app.rs               # wgpu context, egui, render orchestration
│   ├── camera/              # Camera capture via nokhwa
│   │   └── mod.rs
│   ├── ml/                  # ONNX Runtime inference
│   │   └── mod.rs           # Person segmentation model
│   ├── effects/             # Visual effects
│   │   ├── mod.rs
│   │   └── person_particles/
│   │       └── mod.rs       # Particle system with shapes/colors
│   ├── network/             # Output streaming
│   │   ├── mod.rs
│   │   ├── syphon.rs        # macOS Syphon output
│   │   └── texture_share.rs
│   └── shaders/
│       ├── particle.wgsl         # Particle rendering with SDF shapes
│       ├── passthrough.wgsl      # Camera passthrough
│       └── masked_passthrough.wgsl # Person-masked rendering
└── models/
    └── selfie_segmentation.onnx
```

### Person Particles Effect

Dissolves person silhouette into configurable particles:

**Shapes:** Circle, Square, Star, Heart, Diamond (SDF-based rendering)

**Color Modes:**
- Camera - Sample color from original camera pixels
- Solid - Single color with color picker
- Rainbow - Position-based rainbow colors
- Gradient - Interpolate between two colors over lifetime

**Parameters:**
- Spawn Rate (100-10,000 particles/sec)
- Size, Lifetime, Gravity, Wind, Turbulence
- Person Fade (0=visible, 1=invisible, only particles)
- Spawn Inside toggle (fill vs edge spawning)

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Escape` | Exit |
| `F11` | Toggle fullscreen |
| `T` | Spawn test particles |
| `1-3` | Select effect |

## Architecture

### Event Loop (main.rs)

Uses winit's `ApplicationHandler` trait with `ControlFlow::Wait` for low idle CPU:
1. `resumed` → Create window, init wgpu, init egui, start OMT broadcast
2. `about_to_wait` → FPS-locked rendering (target_fps from settings, default 60)
3. `window_event` → Input handling, egui integration, render passes

File dialogs run on background threads via `AsyncFileDialogs` to avoid blocking the UI.

### Layer-Based Composition Model

```
Environment (fixed-resolution GPU texture)
  └── Layer[] (rendered back-to-front)
        ├── ClipGrid (rows × columns of triggerable clips)
        ├── Transform2D (position, scale, rotation, anchor)
        ├── BlendMode (Normal, Additive, Multiply, Screen)
        └── opacity, visible
```

**Key separation:** `Layer` is pure data; `LayerRuntime` holds GPU resources (video players, textures, bind groups). This separation allows serializing Layer state without GPU dependencies.

### Render Pipeline (app.rs)

Each frame executes these render passes:
1. **Checkerboard pass** - Fill environment texture with pattern background
2. **Layer composition** - Render each layer with blend mode into environment texture
3. **Viewport pass** - Scale/pan environment to fit window with zoom
4. **egui pass** - UI overlay with `LoadOp::Load` (preserves previous content)

### Video Pipeline

- Background threads decode at video's native framerate using FFmpeg
- Main thread polls decoded frames and uploads to GPU without blocking
- Hardware acceleration: VideoToolbox (macOS), D3D11VA/NVDEC (Windows)
- HAP codec: Direct BC1/BC3 texture upload (no CPU decode)
- Pending/active swap pattern for smooth clip transitions

### Clip Transitions

When triggering a new clip on a layer:
- **Cut**: Immediate switch
- **Fade**: Old clip fades out, new clip fades in
- **Crossfade**: Both clips blend during transition

Implemented via `pending_runtimes` map that holds the new clip until first frame decoded.

### Viewport Navigation

Environment resolution is independent of window size. The `Viewport` handles pan/zoom navigation with spring physics for smooth right-click panning with rubber-band snap-back.

## External Libraries

Located in `external_libraries/`:
- **Aqueduct** - Rust OMT (Open Media Transport) implementation
- **wgpu** - Forked GPU abstraction layer
- **hap** - HAP codec library

## Data Format

Environment settings saved as `.immersive` XML files via `quick-xml` + serde.

## Tech Stack

- **Graphics:** wgpu 24 (Metal/DX12), winit 0.30
- **GUI:** egui 0.31
- **Video:** ffmpeg-next 7.1, HAP codec
- **Async:** tokio 1
- **Streaming:** Aqueduct (OMT), mdns-sd

## Development Roadmap

See `immersive-server/build_plan.md` for full details. Current status:

| Phase | Goal | Status |
|-------|------|--------|
| 1. Foundation | wgpu render loop, video playback | ✅ Complete |
| 2. Environment & Layers | Multi-layer composition, blend modes, clip grid | ✅ Complete |
| 2.5. Video Manipulation | Clone, multiplex, resize, position | ✅ Complete |
| 3. Hardware Decode | VideoToolbox/NVDEC, HAP codec | ✅ Complete |
| 4. OMT I/O | OMT streaming via Aqueduct | 🔶 Mostly complete |
| 5. Web Control | REST API + WebSocket (Axum) | ⬜ Not started |
| 6. Web Dashboard | Browser-based control surface | ⬜ Not started |
| 7. Polish & Performance | GPU tiling, profiling, installers | ⬜ Not started |
| 9. Projection Mapping | Mesh warp, edge blend, masking | ⬜ Not started |
| 10. NDI I/O | NDI input/output streams | ⬜ Not started |

### Performance Targets

| Metric | Target |
|--------|--------|
| Frame Rate | 60fps locked (vsync) |
| Latency | < 2 frames (< 33ms @ 60fps) |
| Max Layers | 16 simultaneous |
| Max Outputs | 8 displays/projectors |
| Max Resolution | 8K per output |
| Video Decode | 4× 4K @ 60fps headroom |
| API Response | < 5ms for control commands |

### Upcoming Features

- **Web Control Server**: Axum-based REST API and WebSocket for remote control
- **Projection Mapping**: Mesh warp, edge blending, masking, color correction
- **NDI Support**: Industry-standard IP video input/output
- **Multi-Output**: Multiple displays with independent mapping per output
