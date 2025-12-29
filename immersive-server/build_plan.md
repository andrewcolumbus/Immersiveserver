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

### Phase 1: Foundation (Weeks 1–3)

**Goal:** Minimal rendering loop with wgpu, single video file playback.

- [x] Initialize Rust workspace with `cargo new immersive-server`
- [x] Set up wgpu device + surface for a single window
- [x] Implement basic render loop (60fps vsync)
- [x] Integrate `ffmpeg-next` for software video decoding
- [x] Upload decoded frames to GPU texture
- [x] Basic fullscreen quad shader to display video
- [ ] Test on macOS (Metal) and Windows (DX12)

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

- [ ] Add `Environment` struct to hold all layers
  - User-configurable resolution (width × height)
  - Videos larger than environment spill over edges
  - Videos smaller than environment don't fill the canvas
- [ ] Define `Layer` struct (source, transform, opacity, blend mode)
- [ ] Implement layer render order (back-to-front with alpha blending)
- [ ] Add transform pipeline: position, scale, rotation
- [ ] Implement blend modes: Normal, Additive, Multiply, Screen
- [ ] Hot-reload shader system (development mode)

**Data Model:**
```rust
struct Environment {
    width: u32,
    height: u32,
    layers: Vec<Layer>,
}
```

**Shaders (WGSL):**
- `fullscreen_quad.wgsl` — basic texture sampling
- `blend_composite.wgsl` — multi-layer blending pass
- `transform.wgsl` — 2D affine transforms

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

---

### Phase 4: Hardware Video Decoding (Weeks 11–12)

**Goal:** Offload decode to GPU for 4K+ playback.

- [ ] **macOS:** VideoToolbox via `ffmpeg-next` hwaccel
- [ ] **Windows:** D3D11VA / NVDEC via `ffmpeg-next` hwaccel
- [ ] **Hap Codec Support**
  - Direct GPU texture upload (DXT/BC compression)
  - Use Hap library from `external_libraries/hap`
- [ ] Benchmark: target 4× 4K @ 60fps decode headroom

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

---

### Phase 6: OMT Input/Output (Weeks 16–17)

**Goal:** Integrate Open Media Transport for ultra-low latency.

- [ ] Evaluate Aqueduct (Rust-native OMT implementation)
- [ ] OMT Discovery: announce and find sources
- [ ] OMT Receiver: QUIC-based frame reception
- [ ] OMT Sender: output compositor to OMT stream
- [ ] Fallback to provided `libOMT.dylib` / `.dll` if needed

---

### Phase 7: Web Control Server (Weeks 18–20)

**Goal:** Full HTTP/WebSocket API for remote control.

- [ ] **REST API (Axum)**
  - `GET /api/sources` — list available inputs
  - `GET /api/environment` — current environment and layer state
  - `PUT /api/environment` — update environment resolution
  - `POST /api/environment/layers` — add/modify layers
  - `DELETE /api/environment/layers/:id`
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

---

### Phase 8: Web Dashboard (Weeks 21–24)

**Goal:** Professional browser-based control surface.

- [ ] **Tech Stack:** SvelteKit (or React + Vite)
- [ ] **Core Views:**
  - Source Browser (files, NDI, OMT)
  - Layer Timeline / Stack
  - Output Configuration (displays, mapping)
  - Live Preview (WebRTC or MJPEG)
- [ ] **Mapping Editor:**
  - Interactive mesh point editor
  - Edge blend sliders
  - Mask drawing tools
- [ ] **Responsive Design:** Tablet-friendly for on-site operation

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
│   │   └── blend.rs            # Blend modes
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

