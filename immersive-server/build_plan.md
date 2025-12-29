# Immersive Server — Build Plan

A high-performance, cross-platform media server for macOS and Windows.  
Designed for professional projection mapping, NDI/OMT streaming, and real-time web control.

---

## 1. Technology Stack

| Layer | Technology | Rationale |
|-------|------------|-----------|
| **Language** | Rust | Memory safety, fearless concurrency, C++ performance |
| **Graphics** | wgpu (WebGPU) | Metal (macOS) / DX12+Vulkan (Windows), modern shader pipeline |
| **Shaders** | WGSL | Cross-platform, compiled at runtime via Naga |
| **Async Runtime** | Tokio | High-throughput networking for NDI/OMT |
| **Web Server** | Axum | Fastest Rust HTTP framework, native WebSocket support |
| **Video Decode** | FFmpeg (ffmpeg-next) | Hardware-accelerated decoding (VideoToolbox, NVDEC) |
| **NDI** | NDI SDK (via FFI) | Industry-standard IP video |
| **OMT** | Aqueduct / libOMT | Ultra-low latency open transport |
| **Local UI** | egui | Immediate-mode GPU UI for mapping overlays |
| **Frontend** | SvelteKit or React | Web-based control surface |

---

## 2. Core Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                          IMMERSIVE SERVER                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐            │
│  │  NDI Input   │   │  OMT Input   │   │ File Decoder │            │
│  │   Thread     │   │   Thread     │   │   Thread     │            │
│  └──────┬───────┘   └──────┬───────┘   └──────┬───────┘            │
│         │                  │                  │                     │
│         └──────────────────┼──────────────────┘                     │
│                            ▼                                        │
│                 ┌────────────────────┐                              │
│                 │  Lock-Free Ring    │                              │
│                 │  Buffer (Frames)   │                              │
│                 └─────────┬──────────┘                              │
│                           ▼                                         │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                     COMPOSITOR (wgpu)                         │  │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐          │  │
│  │  │ Layer 0 │  │ Layer 1 │  │ Layer 2 │  │ Layer N │          │  │
│  │  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘          │  │
│  │       └────────────┴────────────┴────────────┘                │  │
│  │                         │                                     │  │
│  │                         ▼                                     │  │
│  │  ┌─────────────────────────────────────────────────────────┐ │  │
│  │  │              MAPPING & BLENDING PIPELINE                │ │  │
│  │  │  • Mesh Warp  • Edge Blend  • Masking  • Color Correct  │ │  │
│  │  └─────────────────────────────────────────────────────────┘ │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                           │                                         │
│         ┌─────────────────┼─────────────────┐                       │
│         ▼                 ▼                 ▼                       │
│  ┌────────────┐    ┌────────────┐    ┌────────────┐                │
│  │ Display 1  │    │ Display 2  │    │ Display N NDI/OMT    │                │
│  │ (Projector)│    │ (Projector)│    │  Output    │                │
│  └────────────┘    └────────────┘    └────────────┘                │
│                                                                     │
├─────────────────────────────────────────────────────────────────────┤
│                         CONTROL PLANE                               │
│  ┌────────────────────────────────────────────────────────────────┐│
│  │  Axum HTTP/WebSocket Server  ←→  Web Dashboard (React/Svelte) ││
│  └────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────┘
```

---

## 3. Development Phases

> **⚠️ Testing Requirement:** Before checking off any checkbox, you MUST complete the associated verification steps. Each feature should have passing tests and manual verification before being marked complete.

### Phase 1: Foundation (Weeks 1–3)

**Goal:** Minimal rendering loop with wgpu, single video file playback.

- [x] Initialize Rust workspace with `cargo new immersive-server`
- [x] Set up wgpu device + surface for a single window
- [x] Implement basic render loop (60fps vsync)
- [x] Integrate `ffmpeg-next` for software video decoding
- [x] Upload decoded frames to GPU texture
- [x] Basic fullscreen quad shader to display video
- [x] Test on macOS (Metal) and Windows (DX12)

**✅ Verification Checklist (Phase 1):**
- [x] `cargo test` passes with no failures
- [x] `cargo clippy` reports no warnings
- [x] App launches and displays a window on macOS
- [x] App launches and displays a window on Windows
- [x] Video file (MP4/MOV) plays smoothly at correct speed
- [x] FPS counter shows stable ~60fps
- [x] No memory leaks after 5 minutes of playback (check Activity Monitor / Task Manager)
- [x] App closes cleanly without crashes

**Crates:**
```toml
[dependencies]
wgpu = "23"
winit = "0.30"
pollster = "0.4"
ffmpeg-next = "7"
log = "0.4"
env_logger = "0.11"
```

---

### Phase 2: Environment & Multi-Layer System (Weeks 4–6)

**Goal:** Environment container with configurable resolution, layer system with opacity and blending.

- [x] Add `Environment` struct to hold all layers
  - User-configurable resolution (width × height)
  - Videos larger than environment spill over edges
  - Videos smaller than environment don't fill the canvas
- [x] Define `Layer` struct (source, transform, opacity, blend mode)
- [x] Implement layer render order (back-to-front with alpha blending)
- [x] Add transform pipeline: position, scale, rotation
- [ ] Implement blend modes: Normal, Additive, Multiply, Screen
- [ ] Hot-reload shader system (development mode)
- [ ] **Clip Grid System**
  - Grid of clips (rows × columns) per layer
  - Each cell contains a video/source reference
  - Click cell to trigger playback on that layer
  - Only one clip active per layer at a time
  - Configurable grid dimensions (e.g., 4×4, 8×8)
  - Visual feedback: playing clip highlighted
  - Clip transition modes: cut, fade, crossfade

**Data Model:**
```rust
struct Environment {
    width: u32,
    height: u32,
    layers: Vec<Layer>,
}

struct Layer {
    id: u32,
    name: String,
    clip_grid: ClipGrid,
    active_clip: Option<(usize, usize)>,  // (row, col) of playing clip
    transform: Transform2D,
    opacity: f32,
    blend_mode: BlendMode,
    visible: bool,
}

struct ClipGrid {
    rows: usize,
    columns: usize,
    cells: Vec<Vec<Option<ClipCell>>>,  // [row][col]
}

struct ClipCell {
    source_path: String,           // File path, NDI source, etc.
    thumbnail: Option<TextureId>,  // Preview thumbnail
    label: Option<String>,         // User-defined label
    transition: ClipTransition,    // How to transition to this clip
}

enum ClipTransition {
    Cut,                           // Instant switch
    Fade { duration_ms: u32 },     // Fade out old, fade in new
    Crossfade { duration_ms: u32 }, // Overlap crossfade
}
```

**Shaders (WGSL):**
- `fullscreen_quad.wgsl` — basic texture sampling
- `blend_composite.wgsl` — multi-layer blending pass
- `transform.wgsl` — 2D affine transforms

**✅ Verification Checklist (Phase 2):**
- [ ] `cargo test` passes — include unit tests for `Environment` and `Layer` structs
- [ ] Environment can be created with custom resolution (test 1920×1080, 4096×2160, 800×600)
- [ ] Video larger than environment correctly spills over edges (visually verify)
- [ ] Video smaller than environment shows empty space around it (visually verify)
- [ ] 4+ layers render in correct order (back-to-front)
- [ ] Layer opacity slider works (0% = invisible, 100% = fully opaque)
- [ ] Each blend mode produces visually correct output (compare to reference images)
- [ ] Transform controls work: position, scale, rotation
- [ ] Shader hot-reload works in dev mode (modify .wgsl file, see changes without restart)
- [ ] Performance: 4 layers at 1080p maintains 60fps
- [ ] **Clip Grid:**
  - [ ] Layer displays clip grid with configurable dimensions (test 4×4, 8×8)
  - [ ] Clicking grid cell triggers that clip on the layer
  - [ ] Only one clip plays per layer at a time (previous stops)
  - [ ] Active clip cell is visually highlighted
  - [ ] Empty cells are clickable but do nothing (no crash)
  - [ ] Clip thumbnails display correctly in grid
  - [ ] Cut transition: instant switch, no visual glitch
  - [ ] Fade transition: old clip fades out, new fades in
  - [ ] Crossfade transition: smooth overlap between clips
  - [ ] Transition duration is respected (test 500ms, 1000ms)
  - [ ] Grid state persists across app restart

---

### Phase 2.5: Video Manipulation (Weeks 6–7)

**Goal:** Clone, multiplex, resize, and reposition videos within the environment.

- [ ] **Video Cloning**
  - Duplicate video sources to multiple layers
  - Shared decode, independent transforms
- [ ] **Video Multiplexing**
  - Single source feeding multiple outputs/regions
  - Efficient GPU resource sharing
- [ ] **Resize & Scale**
  - Arbitrary resize per layer (independent of source resolution)
  - Maintain or ignore aspect ratio option
- [ ] **Positioning**
  - Absolute pixel positioning within environment
  - Anchor points (center, corners, edges)
  - Drag-and-drop support in UI

**✅ Verification Checklist (Phase 2.5):**
- [ ] Clone same video to 3+ layers — verify single decode, multiple renders
- [ ] Cloned layers can have independent transforms (position/scale/rotation)
- [ ] Multiplex single source to multiple regions — verify GPU memory usage is shared
- [ ] Resize video up (2× scale) — verify quality/interpolation
- [ ] Resize video down (0.5× scale) — verify no aliasing artifacts
- [ ] "Maintain aspect ratio" option works correctly
- [ ] "Ignore aspect ratio" option allows stretching
- [ ] Position video at exact pixel coordinates (test: place at 100,100)
- [ ] Anchor points work: center places video center at position, top-left places corner
- [ ] Drag-and-drop in UI updates position values correctly
- [ ] Performance: 8 cloned layers maintains 60fps (shared decode)

---

### Phase 3: Projection Mapping (Weeks 7–10)

**Goal:** 2D mesh warping, edge blending, and soft-edge masking.

- [ ] **Mesh Warp Engine**
  - Define control point grid (e.g., 4×4, 8×8, arbitrary)
  - Bezier or linear interpolation for surfaces
  - Per-output mesh storage/serialization
- [ ] **Edge Blending**
  - Parametric blend curves (gamma-corrected)
  - Left/Right/Top/Bottom blend regions per output
  - Overlap calibration tools
- [ ] **Masking**
  - Bezier curve masks
  - Feathered edges
  - Invert/combine masks
- [ ] **Color Correction**
  - Per-output brightness, contrast, gamma
  - Color temperature adjustment
  - LUT support (optional)

**Data Model:**
```rust
struct OutputConfig {
    display_id: u32,
    mesh: WarpMesh,
    edge_blend: EdgeBlendConfig,
    masks: Vec<Mask>,
    color: ColorCorrection,
}
```

**✅ Verification Checklist (Phase 3):**
- [ ] Mesh warp: 4×4 grid deforms image correctly
- [ ] Mesh warp: 8×8 grid provides finer control
- [ ] Bezier interpolation produces smooth curves between control points
- [ ] Mesh configuration saves/loads correctly from file
- [ ] Edge blend: left/right blend creates smooth gradient overlap
- [ ] Edge blend: top/bottom blend works correctly
- [ ] Edge blend: gamma correction produces linear visual falloff
- [ ] Two overlapping outputs blend seamlessly (no visible seam)
- [ ] Bezier mask correctly hides portions of output
- [ ] Mask feathering produces soft edges (test 0px, 10px, 50px feather)
- [ ] Mask invert works correctly
- [ ] Multiple masks combine correctly (union/intersection)
- [ ] Color correction: brightness adjustment (-100% to +100%)
- [ ] Color correction: contrast adjustment works
- [ ] Color correction: gamma curve applies correctly
- [ ] All settings persist across app restart

---

### Phase 4: Hardware Video Decoding (Weeks 11–12)

**Goal:** Offload decode to GPU for 4K+ playback.

- [ ] **macOS:** VideoToolbox via `ffmpeg-next` hwaccel
- [ ] **Windows:** D3D11VA / NVDEC via `ffmpeg-next` hwaccel
- [ ] **Hap Codec Support**
  - Direct GPU texture upload (DXT/BC compression)
  - Use Hap library from `external_libraries/hap`
- [ ] Benchmark: target 4× 4K @ 60fps decode headroom

**✅ Verification Checklist (Phase 4):**
- [ ] macOS: VideoToolbox hwaccel enabled (check logs for "hwaccel: videotoolbox")
- [ ] macOS: CPU usage drops significantly vs software decode (measure with Activity Monitor)
- [ ] Windows: D3D11VA or NVDEC enabled (check logs)
- [ ] Windows: GPU decode visible in Task Manager GPU stats
- [ ] 4K H.264 video plays smoothly at 60fps
- [ ] 4K H.265/HEVC video plays smoothly at 60fps
- [ ] Hap codec: DXT texture uploads directly (no CPU conversion)
- [ ] Hap Alpha codec works correctly
- [ ] Benchmark test: decode 4× 4K streams simultaneously, verify <50% CPU usage
- [ ] Graceful fallback to software decode if hwaccel unavailable
- [ ] No visual artifacts or color space issues with hwaccel

---

### Phase 5: NDI Input/Output (Weeks 13–15)

**Goal:** Receive and send video over NDI.

- [ ] **NDI Receiver**
  - Enumerate NDI sources on network
  - Spawn receiver thread per source
  - Push frames to ring buffer
- [ ] **NDI Sender**
  - Capture compositor output
  - Encode and transmit via NDI
  - Support multiple simultaneous outputs
- [ ] **FFI Bindings**
  - Use `bindgen` for NDI SDK headers
  - Safe Rust wrapper around C API

**Crate:** Create `ndi-rs` wrapper or use existing community bindings.

**✅ Verification Checklist (Phase 5):**
- [ ] NDI sources on local network are discovered and listed
- [ ] NDI source discovery updates dynamically (new sources appear)
- [ ] Receive NDI stream and display as layer (test with NDI Test Patterns)
- [ ] Receive multiple NDI streams simultaneously (test 4 streams)
- [ ] NDI receiver handles source disconnect gracefully (no crash, shows placeholder)
- [ ] NDI receiver auto-reconnects when source comes back
- [ ] Send compositor output as NDI stream
- [ ] NDI output visible in NDI Studio Monitor
- [ ] NDI output maintains quality (compare to direct output)
- [ ] Multiple NDI outputs work simultaneously
- [ ] Latency measurement: NDI round-trip < 3 frames
- [ ] Memory stable after 1 hour of NDI streaming (no leaks)

---

### Phase 6: OMT Input/Output (Weeks 16–17)

**Goal:** Integrate Open Media Transport for ultra-low latency.

- [ ] Evaluate Aqueduct (Rust-native OMT implementation)
- [ ] OMT Discovery: announce and find sources
- [ ] OMT Receiver: QUIC-based frame reception
- [ ] OMT Sender: output compositor to OMT stream
- [ ] Fallback to provided `libOMT.dylib` / `.dll` if needed

**✅ Verification Checklist (Phase 6):**
- [ ] OMT sources are discovered on network
- [ ] OMT receiver connects and displays stream
- [ ] OMT latency is measurably lower than NDI (< 1 frame target)
- [ ] OMT sender outputs compositor to OMT stream
- [ ] OMT output receivable by other OMT clients
- [ ] QUIC transport handles packet loss gracefully
- [ ] Fallback to libOMT works when Aqueduct unavailable
- [ ] OMT and NDI can run simultaneously without conflicts
- [ ] OMT reconnects automatically on network interruption

---

### Phase 7: Web Control Server (Weeks 18–20)

**Goal:** Full HTTP/WebSocket API for remote control.

- [ ] **REST API (Axum)**
  - `GET /api/sources` — list available inputs
  - `GET /api/environment` — current environment and layer state
  - `PUT /api/environment` — update environment resolution
  - `POST /api/environment/layers` — add/modify layers
  - `DELETE /api/environment/layers/:id`
  - `GET /api/layers/:id/clips` — get clip grid for layer
  - `PUT /api/layers/:id/clips/:row/:col` — set clip at grid position
  - `POST /api/layers/:id/clips/:row/:col/trigger` — trigger clip playback
  - `DELETE /api/layers/:id/clips/:row/:col` — remove clip from grid
  - `GET /api/outputs` — list displays/projectors
  - `PUT /api/outputs/:id` — update mapping config
- [ ] **WebSocket (real-time)**
  - Subscribe to composition state changes
  - Push frame rate / performance metrics
  - Bi-directional control commands
- [ ] **Static File Serving**
  - Serve web dashboard from embedded assets
- [ ] **Authentication**
  - Token-based auth for production use

**Crates:**
```toml
axum = "0.8"
tokio = { version = "1", features = ["full"] }
tower = "0.5"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

**✅ Verification Checklist (Phase 7):**
- [ ] Server starts on configurable port (default 8080)
- [ ] `GET /api/sources` returns JSON list of available inputs
- [ ] `GET /api/environment` returns current environment state
- [ ] `PUT /api/environment` updates resolution (verify with GET)
- [ ] `POST /api/environment/layers` adds a new layer
- [ ] `DELETE /api/environment/layers/:id` removes layer
- [ ] `GET /api/layers/:id/clips` returns clip grid state
- [ ] `PUT /api/layers/:id/clips/:row/:col` assigns clip to grid cell
- [ ] `POST /api/layers/:id/clips/:row/:col/trigger` triggers clip playback
- [ ] `DELETE /api/layers/:id/clips/:row/:col` removes clip from cell
- [ ] `GET /api/outputs` lists connected displays
- [ ] `PUT /api/outputs/:id` updates output config
- [ ] WebSocket connection establishes successfully
- [ ] WebSocket receives real-time state updates when layers change
- [ ] WebSocket receives FPS/performance metrics
- [ ] WebSocket commands (play/pause/etc) execute correctly
- [ ] Static files served from `/` (test with simple HTML file)
- [ ] API responds in < 5ms for control commands (measure with curl)
- [ ] Token auth blocks unauthorized requests (401 response)
- [ ] Valid token allows access
- [ ] API handles malformed JSON gracefully (400 response, not crash)

---

### Phase 8: Web Dashboard (Weeks 21–24)

**Goal:** Professional browser-based control surface.

- [ ] **Tech Stack:** SvelteKit (or React + Vite)
- [ ] **Core Views:**
  - Source Browser (files, NDI, OMT)
  - Layer Timeline / Stack
  - **Clip Grid Launcher** (per-layer grid of triggerable clips)
  - Output Configuration (displays, mapping)
  - Live Preview (WebRTC or MJPEG)
- [ ] **Mapping Editor:**
  - Interactive mesh point editor
  - Edge blend sliders
  - Mask drawing tools
- [ ] **Responsive Design:** Tablet-friendly for on-site operation

**✅ Verification Checklist (Phase 8):**
- [ ] Dashboard loads in Chrome, Firefox, Safari, Edge
- [ ] Source browser lists files, NDI sources, and OMT sources
- [ ] Can add source to environment by clicking/dragging
- [ ] Layer stack shows all layers in correct order
- [ ] Layer reordering via drag-and-drop works
- [ ] Layer properties (opacity, blend, transform) editable
- [ ] **Clip Grid UI:**
  - [ ] Clip grid displays for each layer
  - [ ] Grid shows thumbnails for assigned clips
  - [ ] Clicking cell triggers clip playback
  - [ ] Active clip visually highlighted in grid
  - [ ] Drag source from browser to grid cell to assign
  - [ ] Right-click cell to remove clip
  - [ ] Grid dimensions configurable per layer
- [ ] Output configuration shows all connected displays
- [ ] Live preview displays current compositor output
- [ ] Preview updates in real-time (< 200ms latency)
- [ ] Mesh point editor: can drag control points
- [ ] Mesh changes apply to output in real-time
- [ ] Edge blend sliders update output immediately
- [ ] Mask drawing tool creates valid mask shapes
- [ ] Dashboard works on iPad (test Safari, touch interactions)
- [ ] All controls accessible without horizontal scrolling on tablet
- [ ] Dashboard reconnects automatically if server restarts

---

### Phase 9: Polish & Performance (Weeks 25–28)

**Goal:** Production-ready stability and optimization.

- [ ] **Performance Profiling**
  - GPU profiling with wgpu timestamps
  - CPU profiling with `perf` / Instruments
  - Memory leak detection
- [ ] **GPU Tiling for Large Environments**
  - Automatically tile environments exceeding GPU max texture size
  - Seamless rendering across tile boundaries
  - Dynamic tile allocation based on viewport
  - Query `device.limits().max_texture_dimension_2d` at runtime
- [ ] **Error Handling**
  - Graceful degradation on source loss
  - Auto-reconnect for NDI/OMT
  - User-visible error messages
- [ ] **Logging & Telemetry**
  - Structured logging with `tracing`
  - Optional remote log shipping
- [ ] **Installer/Packaging**
  - macOS: `.pkg` or `.dmg`
  - Windows: MSI or NSIS installer
  - Code signing for both platforms

**✅ Verification Checklist (Phase 9):**
- [ ] GPU profiling data collected and reviewed (identify bottlenecks)
- [ ] CPU profiling completed — no functions taking >10% of frame time
- [ ] Memory leak test: run 24 hours, memory usage stable (±10%)
- [ ] GPU tiling: create 16384×16384 environment (exceeds typical 8K limit)
- [ ] GPU tiling: verify no visible seams at tile boundaries
- [ ] GPU tiling: panning/scrolling across tiles is smooth
- [ ] Tile allocation updates dynamically when viewport moves
- [ ] App logs GPU max texture size at startup
- [ ] Source disconnect shows user-friendly message (not crash)
- [ ] NDI/OMT auto-reconnect works within 5 seconds
- [ ] All errors logged with `tracing` (verify structured JSON output)
- [ ] Log levels configurable (debug/info/warn/error)
- [ ] macOS installer: `.dmg` mounts and app drags to Applications
- [ ] macOS installer: app launches without Gatekeeper warnings (signed)
- [ ] Windows installer: MSI installs without admin elevation (if possible)
- [ ] Windows installer: app runs without "Unknown publisher" warning (signed)
- [ ] Uninstall cleans up all files on both platforms

---

### Phase 10: REST API Feature Planning (Weeks 29–30)

**Goal:** Design and implement comprehensive REST API based on user requirements.

- [ ] **API Discovery Session**
  - Document all controllable app functions
  - Survey users for required API endpoints
  - Prioritize based on use cases
- [ ] **Core API Endpoints** (to be expanded with user input)
  - Environment management (create, configure, list)
  - Layer manipulation (add, remove, reorder, transform)
  - Video source control (load, play, pause, seek)
  - Output configuration
- [ ] **API Documentation**
  - OpenAPI/Swagger spec generation
  - Interactive API explorer
- [ ] **User-Requested Features**
  - *TODO: Add endpoints based on user feedback*
  - *Ask: What functions should be exposed via API?*

**✅ Verification Checklist (Phase 10):**
- [ ] Complete list of app functions documented
- [ ] User survey sent and responses collected
- [ ] API endpoints prioritized based on user feedback
- [ ] All core endpoints implemented and tested:
  - [ ] Environment CRUD operations work via API
  - [ ] Layer manipulation works via API
  - [ ] Video control (load/play/pause/seek) works via API
  - [ ] Output configuration works via API
- [ ] OpenAPI/Swagger spec generated and valid
- [ ] API explorer accessible at `/api/docs`
- [ ] API explorer allows testing endpoints interactively
- [ ] User-requested features implemented (list specific ones as added)
- [ ] API versioning strategy documented (e.g., `/api/v1/`)
- [ ] Breaking changes documented in changelog

---

## 4. File Structure

```
immersive-server/
├── Cargo.toml
├── Cargo.lock
├── build.rs                    # Build script for FFI bindings
├── src/
│   ├── main.rs                 # Entry point
│   ├── lib.rs                  # Library root
│   ├── app.rs                  # Application state
│   ├── compositor/
│   │   ├── mod.rs
│   │   ├── layer.rs            # Layer definition
│   │   ├── environment.rs      # Environment (holds all layers)
│   │   ├── blend.rs            # Blend modes
│   │   └── clip_grid.rs        # Clip grid/launcher system
│   ├── render/
│   │   ├── mod.rs
│   │   ├── pipeline.rs         # wgpu pipeline setup
│   │   ├── texture.rs          # Texture management
│   │   └── shaders/
│   │       ├── quad.wgsl
│   │       ├── blend.wgsl
│   │       ├── warp.wgsl
│   │       └── edge_blend.wgsl
│   ├── mapping/
│   │   ├── mod.rs
│   │   ├── mesh.rs             # Warp mesh
│   │   ├── edge_blend.rs       # Edge blending
│   │   ├── mask.rs             # Masking
│   │   └── output.rs           # Output configuration
│   ├── video/
│   │   ├── mod.rs
│   │   ├── decoder.rs          # FFmpeg decoder
│   │   ├── hap.rs              # Hap codec
│   │   └── frame.rs            # Frame buffer
│   ├── network/
│   │   ├── mod.rs
│   │   ├── ndi.rs              # NDI bindings
│   │   ├── omt.rs              # OMT bindings
│   │   └── discovery.rs        # Source discovery
│   ├── api/
│   │   ├── mod.rs
│   │   ├── server.rs           # Axum server
│   │   ├── routes.rs           # REST endpoints
│   │   ├── websocket.rs        # WebSocket handler
│   │   └── types.rs            # API types
│   └── ui/
│       ├── mod.rs
│       └── overlay.rs          # egui debug overlay
├── web/                        # Web dashboard (SvelteKit)
│   ├── package.json
│   ├── src/
│   └── ...
├── assets/
│   └── shaders/                # Runtime shader loading
└── tests/
    ├── environment_tests.rs
    ├── mapping_tests.rs
    └── api_tests.rs
```

---

## 5. Key Dependencies

```toml
[package]
name = "immersive-server"
version = "0.1.0"
edition = "2024"

[dependencies]
# Graphics
wgpu = "23"
winit = "0.30"
bytemuck = { version = "1", features = ["derive"] }

# Async Runtime
tokio = { version = "1", features = ["full"] }

# Web Server
axum = "0.8"
tower = "0.5"
tower-http = { version = "0.6", features = ["fs", "cors"] }

# Video
ffmpeg-next = "7"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Utilities
anyhow = "1"
thiserror = "2"
parking_lot = "0.12"
crossbeam = "0.8"

# UI (optional)
egui = "0.30"
egui-wgpu = "0.30"
egui-winit = "0.30"

[build-dependencies]
bindgen = "0.71"  # For NDI/OMT FFI
```

---

## 6. Performance Targets

| Metric | Target |
|--------|--------|
| Frame Rate | 60fps locked (vsync) |
| Latency (glass-to-glass) | < 2 frames (< 33ms @ 60fps) |
| Max Layers | 16 simultaneous |
| Max Outputs | 8 displays / projectors |
| Max Resolution | 8K per output |
| NDI Streams | 8 simultaneous inputs |
| Video Decode | 4× 4K @ 60fps headroom |
| API Response | < 5ms for control commands |
| WebSocket Latency | < 10ms round-trip |

---

## 7. Risk Mitigation

| Risk | Mitigation |
|------|------------|
| FFmpeg licensing (GPL) | Use LGPL build, or switch to GStreamer |
| NDI SDK distribution | Bundle SDK, respect licensing terms |
| macOS GPU driver quirks | Test on Intel, M1, M2, M3 |
| Windows DX12 fallback | Implement Vulkan backend as fallback |
| Memory pressure (4K video) | Ring buffers with bounded capacity |
| Network congestion (NDI) | Implement adaptive bitrate / frame drop |

---

## 8. Milestones

| Milestone | Target Date | Deliverable |
|-----------|-------------|-------------|
| M1: First Frame | Week 3 | Video file plays in window |
| M2: Environment | Week 6 | Environment with layers and blending |
| M2.5: Video Manipulation | Week 7 | Clone, multiplex, resize, move videos |
| M3: Mapping v1 | Week 10 | Mesh warp + edge blend |
| M4: NDI Working | Week 15 | Receive/send NDI streams |
| M5: Web Control | Week 20 | REST API + basic dashboard |
| M6: Alpha Release | Week 24 | Feature-complete, internal testing |
| M7: Performance | Week 28 | GPU tiling, optimization, polish |
| M8: API Planning | Week 30 | User-driven API feature expansion |
| M9: v1.0 Release | Week 34 | Production-ready |

---

## 9. References

- [wgpu Documentation](https://wgpu.rs/)
- [NDI SDK](https://ndi.video/for-developers/ndi-sdk/)
- [Open Media Transport](https://www.intopix.com/omt)
- [FFmpeg Hardware Acceleration](https://trac.ffmpeg.org/wiki/HWAccelIntro)
- [Hap Codec](https://hap.video/)
- [Axum Web Framework](https://docs.rs/axum)
- [egui Immediate Mode GUI](https://www.egui.rs/)

---

*Last Updated: December 2024*

