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
│   ├── gpu_context.rs       # wgpu device/queue initialization
│   ├── preview_player.rs    # Preview video player for UI
│   │
│   ├── audio/               # Audio input & FFT analysis
│   │   ├── mod.rs           # Module exports
│   │   ├── manager.rs       # AudioManager: coordinates sources & FFT
│   │   ├── fft.rs           # FFT analyzer (RustFFT, 2048 samples, Hann window)
│   │   ├── source.rs        # AudioSource trait & ring buffer
│   │   ├── system_input.rs  # System audio capture (CoreAudio on macOS)
│   │   ├── omt_source.rs    # OMT network audio source
│   │   ├── ndi_source.rs    # NDI network audio source
│   │   └── types.rs         # AudioBand, AudioBuffer, FftData types
│   │
│   ├── compositor/          # Composition engine
│   │   ├── environment.rs   # Fixed-resolution canvas with layer vector
│   │   ├── layer.rs         # Layer definition (source, transform, blend, clips)
│   │   ├── clip.rs          # ClipCell, ClipGrid, ClipTransition
│   │   ├── blend.rs         # BlendMode enum
│   │   └── viewport.rs      # Pan/zoom with spring physics
│   │
│   ├── video/               # Video decoding & rendering
│   │   ├── decoder.rs       # FFmpeg decoder with hwaccel (VideoToolbox/D3D11VA)
│   │   ├── player.rs        # Background-threaded playback
│   │   ├── renderer.rs      # GPU pipeline for video/layer rendering
│   │   ├── texture.rs       # GPU texture management
│   │   ├── hap.rs           # HAP codec (BC1/BC3 direct upload)
│   │   └── frame.rs         # DecodedFrame struct
│   │
│   ├── effects/             # Resolume-style stackable effects
│   │   ├── mod.rs           # Module documentation
│   │   ├── types.rs         # EffectStack, EffectInstance, parameters
│   │   ├── traits.rs        # EffectDefinition, GpuEffectRuntime traits
│   │   ├── registry.rs      # Effect factory registry
│   │   ├── runtime.rs       # GPU effect chain processing
│   │   ├── automation.rs    # BPM/LFO parameter modulation
│   │   ├── manager.rs       # Effect lifecycle management
│   │   └── builtin/         # Built-in effects
│   │       ├── color_correction.rs  # Brightness, saturation, hue
│   │       ├── invert.rs            # Video inversion
│   │       ├── heat.rs              # Heat vision effect
│   │       ├── auto_mask.rs         # Automatic masking
│   │       ├── image_rain.rs        # Particle rain with image sampling
│   │       ├── poop_rain.rs         # Emoji particle rain
│   │       ├── multiplex.rs         # Multi-input composition
│   │       └── slide.rs             # Slide/wipe transitions
│   │
│   ├── output/              # Multi-screen projection mapping
│   │   ├── mod.rs           # Module exports
│   │   ├── runtime.rs       # Output processing engine
│   │   ├── display.rs       # DisplayManager, multi-monitor detection
│   │   ├── screen.rs        # Physical display representation
│   │   ├── slice.rs         # Slice-based input selection
│   │   ├── warp.rs          # Perspective/mesh warp
│   │   ├── edge_blend.rs    # Seamless projector overlap
│   │   ├── mask.rs          # Per-output masking (Bezier)
│   │   ├── color.rs         # Per-output color correction
│   │   └── preset.rs        # Output configuration presets
│   │
│   ├── network/             # Streaming & discovery
│   │   ├── omt.rs           # OMT receiver via Aqueduct
│   │   ├── omt_ffi.rs       # OMT FFI bindings
│   │   ├── omt_capture.rs   # GPU readback for OMT output streaming
│   │   ├── ndi.rs           # NDI receiver
│   │   ├── ndi_ffi.rs       # NDI FFI bindings
│   │   ├── ndi_capture.rs   # NDI output streaming
│   │   ├── syphon.rs        # macOS Syphon output
│   │   ├── syphon_ffi.rs    # Syphon FFI bindings
│   │   ├── spout.rs         # Windows Spout output
│   │   ├── spout_ffi.rs     # Spout FFI bindings
│   │   ├── discovery.rs     # mDNS source discovery
│   │   └── texture_share.rs # Shared texture utilities
│   │
│   ├── api/                 # REST API & WebSocket control
│   │   ├── server.rs        # Axum server setup
│   │   ├── routes.rs        # REST API endpoints (40+)
│   │   ├── websocket.rs     # WebSocket event streaming
│   │   ├── shared.rs        # ApiCommand, AppSnapshot types
│   │   ├── types.rs         # Request/response structures
│   │   ├── dashboard.rs     # Dashboard HTML server
│   │   └── dashboard.html   # Browser-based control UI
│   │
│   ├── telemetry/           # Performance monitoring & logging
│   │   ├── logging.rs       # Structured logging (tracing crate)
│   │   ├── metrics.rs       # Frame timing, GPU memory stats
│   │   └── profiling.rs     # GPU timestamp queries
│   │
│   ├── previs/              # 3D wall layout preview
│   │   ├── camera.rs        # 3D preview camera
│   │   ├── mesh.rs          # 3D wall mesh generation
│   │   ├── renderer.rs      # 3D rendering pipeline
│   │   └── types.rs         # 3D data structures
│   │
│   ├── ui/                  # egui panels & windows
│   │   ├── mod.rs           # Panel boilerplate macro
│   │   ├── dock.rs          # Docking system (DockManager, DockZone)
│   │   ├── window_registry.rs     # Window state management
│   │   ├── widgets.rs       # Custom widgets (resettable sliders)
│   │   ├── icons.rs         # Icon definitions
│   │   ├── menu_bar.rs      # File/View/Help menus, status bar
│   │   ├── menu_definition.rs     # Menu structure definitions
│   │   ├── native_menu.rs   # Native OS menu support (macOS)
│   │   ├── properties_panel.rs    # Environment/Layer/Clip editing
│   │   ├── clip_grid_panel.rs     # VJ-style clip launcher
│   │   ├── sources_panel.rs       # OMT/NDI source browser
│   │   ├── effects_browser_panel.rs  # Effects browser
│   │   ├── performance_panel.rs   # Real-time metrics display
│   │   ├── preferences_window.rs  # Application settings
│   │   ├── advanced_output_window.rs  # Projection mapping UI
│   │   ├── preview_monitor_panel.rs   # Preview monitoring
│   │   ├── file_browser_panel.rs  # Media file browsing
│   │   ├── previs_panel.rs        # 3D wall preview
│   │   ├── viewport_widget.rs     # Main composition viewport
│   │   ├── thumbnail_cache.rs     # Video thumbnail caching
│   │   └── layout_preset.rs       # UI layout presets
│   │
│   ├── converter/           # HAP video converter tool
│   │   └── window.rs        # Converter UI
│   │
│   └── shaders/
│       ├── mod.rs           # Shader loading with hot-reload
│       ├── fullscreen_quad.wgsl
│       ├── test_pattern.wgsl
│       ├── previs_3d.wgsl
│       ├── effects/         # Effect-specific shaders
│       └── output/          # Output processing shaders
│
├── examples/
│   ├── decode_video.rs      # Standalone FFmpeg decode test
│   ├── decode_to_texture.rs # Decode → GPU texture test
│   └── play_video.rs        # Full playback example
│
├── docs/
│   └── omt-evaluation.md    # OMT protocol evaluation notes
│
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
        ├── EffectStack (stackable GPU/CPU effects)
        └── opacity, visible
```

**Key separation:** `Layer` is pure data; `LayerRuntime` holds GPU resources (video players, textures, bind groups). This separation allows serializing Layer state without GPU dependencies.

### Render Pipeline (app.rs)

Each frame executes these render passes:
1. **Checkerboard pass** - Fill environment texture with pattern background
2. **Layer composition** - Render each layer with blend mode into environment texture
3. **Effects processing** - Apply effect stacks to layers
4. **Viewport pass** - Scale/pan environment to fit window with zoom
5. **Output pass** - Apply warp/blend/mask for projection mapping
6. **egui pass** - UI overlay with `LoadOp::Load` (preserves previous content)

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

### Audio System

Real-time audio input with FFT analysis for audio-reactive effects:
- **Sources:** System audio (CoreAudio), OMT streams, NDI streams
- **FFT:** RustFFT with 2048-sample window, Hann windowing
- **Bands:** Configurable frequency bands with per-band sensitivity
- **Output:** FftData struct with smoothed band levels

### Effects System

Resolume-style stackable effects with BPM/LFO automation:
- **GPU Effects:** Shader-based processing (color correction, invert, heat)
- **CPU Effects:** Rust-based processing (auto-mask, particle systems)
- **Automation:** BPM sync, LFO modulation of any parameter
- **Registry:** Factory pattern for effect instantiation

### Output/Projection Mapping

Multi-screen output with advanced projection features:
- **Slices:** Crop/position regions from composition or layers
- **Warp:** Perspective and mesh-based warping
- **Edge Blend:** Seamless projector overlap
- **Masks:** Bezier-based per-output masking
- **Color:** Per-output color correction

### API & WebSocket

Remote control via Axum-based REST API and WebSocket:
- **REST:** 40+ endpoints for layer/clip/effect/source control
- **WebSocket:** Real-time state streaming and events
- **Dashboard:** Browser-based control UI (dashboard.html)

### UI Conventions

#### Slider/DragValue Reset on Right-Click

All `egui::Slider` and `egui::DragValue` widgets should include a right-click context menu with a "Reset to [default]" option:

```rust
let response = ui.add(egui::Slider::new(&mut value, min..=max));
if response.changed() {
    // handle change
}
response.context_menu(|ui| {
    if ui.button("Reset to [default value]").clicked() {
        value = DEFAULT;
        // emit action or set changed flag
        ui.close_menu();
    }
});
```

This provides a consistent UX where users can quickly reset any numeric parameter by right-clicking.

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
- **Audio:** RustFFT, CoreAudio (macOS)
- **Web:** Axum (REST API, WebSocket)
- **Async:** tokio 1
- **Streaming:** Aqueduct (OMT), NDI, Syphon, Spout, mdns-sd

## Development Roadmap

See `immersive-server/build_plan.md` for full details. Current status:

| Phase | Goal | Status |
|-------|------|--------|
| 1. Foundation | wgpu render loop, video playback | ✅ Complete |
| 2. Environment & Layers | Multi-layer composition, blend modes, clip grid | ✅ Complete |
| 2.5. Video Manipulation | Clone, multiplex, resize, position | ✅ Complete |
| 3. Hardware Decode | VideoToolbox/NVDEC, HAP codec | ✅ Complete |
| 4. OMT I/O | OMT streaming via Aqueduct | ✅ Complete |
| 5. Web Control | REST API + WebSocket (Axum) | ✅ Complete |
| 6. Web Dashboard | Browser-based control surface | ✅ Complete |
| 7. Polish & Performance | GPU tiling, profiling, installers | 🔶 In progress |
| 8. Effects System | Stackable effects with automation | ✅ Complete |
| 9. Projection Mapping | Mesh warp, edge blend, masking | ✅ Complete |
| 10. NDI I/O | NDI input/output streams | ✅ Complete |
| 11. Audio Reactivity | FFT analysis, audio-reactive effects | ✅ Complete |

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

- **Multi-Output Windows:** Independent fullscreen windows per output
- **Advanced Automation:** MIDI/OSC input for parameter control
- **GPU Profiling:** Detailed per-pass timing statistics
- **Installer Packages:** macOS .app and Windows .exe distributions
