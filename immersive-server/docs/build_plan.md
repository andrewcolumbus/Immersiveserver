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
- [x] Implement blend modes: Normal, Additive, Multiply, Screen
- [x] Hot-reload shader system (development mode)
- [x] **Clip Grid System**
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
- [x] `cargo test` passes — include unit tests for `Environment` and `Layer` structs
- [x] Environment can be created with custom resolution (test 1920×1080, 4096×2160, 800×600)
- [x] Video larger than environment correctly spills over edges (visually verify)
- [x] Video smaller than environment shows empty space around it (visually verify)
- [x] 4+ layers render in correct order (back-to-front)
- [ ] Layer opacity slider works (0% = invisible, 100% = fully opaque)
- [x] Each blend mode produces visually correct output (compare to reference images)
- [ ] Transform controls work: position, scale, rotation
- [x] Shader hot-reload works in dev mode (modify .wgsl file, see changes without restart)
- [x] Performance: 4 layers at 1080p maintains 60fps
- [ ] **Clip Grid:**
  - [x] Layer displays clip grid with configurable dimensions (test 4×4, 8×8)
  - [x] Clicking grid cell triggers that clip on the layer
  - [x] Only one clip plays per layer at a time (previous stops)
  - [x] Active clip cell is visually highlighted
  - [x] Empty cells are clickable but do nothing (no crash)
  - [x] Clip thumbnails display correctly in grid
  - [x] Cut transition: instant switch, no visual glitch
  - [x] Fade transition: old clip fades out, new fades in
  - [x] Transition duration is respected (test 500ms, 1000ms)
  - [x] Grid state persists across app restart

---

### Phase 2.5: Video Manipulation (Weeks 6–7)

**Goal:** Clone, multiplex, resize, and reposition videos within the environment.

- [x] **Video Cloning**
  - Duplicate video sources to multiple layers
  - Shared decode, independent transforms
- [x] **Video Multiplexing**
  - Single source feeding multiple outputs/regions
  - Efficient GPU resource sharing
  - Establishes clip/layer/environment page, this is the first (optional) effect
similar to this: /Users/andrewcolumbus/Documents/Code/Immersiveserver/image.png
- [x] **Resize & Scale**
  - Arbitrary resize per layer (independent of source resolution)
  - Maintain or ignore aspect ratio option
  - this is also on the clip/layer page
- [x] **Positioning**
  - Absolute pixel positioning within environment
  - Anchor points (center, corners, edges)
  - Drag-and-drop support in UI

**✅ Verification Checklist (Phase 2.5):**
- [x] Clone same video to 3+ layers — verify single decode, multiple renders
- [x] Cloned layers can have independent transforms (position/scale/rotation)
- [x] Multiplex single source to multiple regions — verify GPU memory usage is shared
- [x] Resize video up (2× scale) — verify quality/interpolation
- [x] Resize video down (0.5× scale) — verify no aliasing artifacts
- [x] "Maintain aspect ratio" option works correctly
- [x] "Ignore aspect ratio" option allows stretching
- [x] Position video at exact pixel coordinates (test: place at 100,100)
- [x] Anchor points work: center places video center at position, top-left places corner
- [x] Drag-and-drop in UI updates position values correctly
- [x] Performance: 8 cloned layers maintains 60fps (shared decode)

---

### Phase 2.6: Effects System (Weeks 8–10)

**Goal:** Resolume-style stackable effects with GPU processing and BPM automation.

- [x] **Core Effect System**
  - Data/runtime separation pattern (EffectStack / EffectStackRuntime)
  - Effect registry with category-based organization
  - Ping-pong texture pool for multi-effect GPU chains
  - Parameter system with Float, Int, Bool, Vec2, Vec3, Color, Enum support

- [x] **GPU Effect Processing**
  - GpuEffectRuntime trait for WGSL-based effects
  - Integration with layer render loop
  - Efficient texture management (reusable ping-pong buffers)

- [x] **Built-in Effects**
  - Color Correction (brightness, contrast, saturation, hue, gamma)
  - Invert (amount, invert_alpha)

- [x] **BPM/LFO Automation**
  - BpmClock with tap tempo support
  - LFO waveforms: Sine, Triangle, Square, Sawtooth, Random
  - Beat-sync envelope (Attack, Decay, Sustain, Release)
  - AutomationSource for parameter modulation

- [x] **UI**
  - Effects Browser Panel (category tree, search, drag-and-drop)
  - Properties Panel Effects Section (bypass, solo, reorder, parameters)
  - Per-effect bypass (B) and solo (S) buttons
  - Parameter sliders/controls based on type

**Files Created:**
```
src/effects/
├── mod.rs              # Module exports
├── types.rs            # EffectStack, EffectInstance, Parameter, ParameterValue
├── traits.rs           # EffectDefinition, GpuEffectRuntime, CpuEffectRuntime
├── registry.rs         # EffectRegistry with category support
├── runtime.rs          # EffectStackRuntime, EffectTexturePool
├── automation.rs       # BpmClock, LfoSource, BeatEnvelopeState
├── manager.rs          # EffectManager (coordinates processing)
└── builtin/
    ├── mod.rs
    ├── color_correction.rs
    └── invert.rs

src/shaders/effects/
├── common.wgsl         # Shared utilities (HSV conversion, etc.)
├── color_correction.wgsl
└── invert.wgsl

src/ui/effects_browser_panel.rs
```

**✅ Verification Checklist (Phase 2.6):**
- [x] `cargo test` passes — all 72+ effect-related tests pass
- [x] Effects Browser Panel shows Color and other categories
- [x] Add Effect to layer via Properties Panel works
- [x] Effect parameter sliders update in real-time
- [x] Bypass (B) button disables effect visually (strikethrough)
- [x] Solo (S) button isolates single effect
- [x] Reorder (▲/▼) buttons move effects in chain
- [x] Remove (✕) button deletes effect
- [x] Effects serialize/deserialize in .immersive files
- [x] Multiple effects chain correctly (ping-pong rendering)
- [x] Color Correction effect: brightness, contrast, saturation work
- [x] Invert effect: amount slider blends with original

---

### Phase 3: Hardware Video Decoding (Weeks 11–12)

**Goal:** Offload decode to GPU for 4K+ playback.

- [x] **macOS:** VideoToolbox via `ffmpeg-next` hwaccel
- [x] **Windows:** D3D11VA / NVDEC via `ffmpeg-next` hwaccel
- [x] **Hap Codec Support**
  - Direct GPU texture upload (DXT/BC compression)
  - HapDecoder with BC1/BC3 texture format support
- [x] Benchmark: target 4× 4K @ 60fps decode headroom

**✅ Verification Checklist (Phase 3):**
- [x] macOS: VideoToolbox hwaccel enabled (check logs for "hwaccel: videotoolbox")
- [x] macOS: CPU usage drops significantly vs software decode (measure with Activity Monitor)
- [x] Windows: D3D11VA or NVDEC enabled (check logs)
- [t] Windows: GPU decode visible in Task Manager GPU stats
- [x] 4K H.264 video plays smoothly at 60fps
- [x] 4K H.265/HEVC video plays smoothly at 60fps
- [x] Hap codec: DXT texture uploads directly (no CPU conversion)
- [x] Hap Alpha codec works correctly
- [x] Benchmark test: decode 4× 4K streams simultaneously, verify <50% CPU usage
- [x] Graceful fallback to software decode if hwaccel unavailable
- [x] No visual artifacts or color space issues with hwaccel

---

### Phase 4: OMT Input/Output (Weeks 16–17)

**Goal:** Integrate Open Media Transport for ultra-low latency.

- [x] Evaluate Aqueduct (Rust-native OMT implementation)
- [x] OMT Discovery: announce and find sources
- [] OMT Receiver: QUIC-based frame reception
- [x] OMT Sender: output environment to OMT stream
- [ ] Fallback to provided `libOMT.dylib` / `.dll` if needed

**✅ Verification Checklist (Phase 4):**
- [x] OMT sources are discovered on network
- [x] OMT receiver connects and displays stream
- [x] OMT latency is measurably lower than NDI (< 1 frame target) [AFTER NDI IMPLTEMENTATION]
- [x] OMT sender outputs compositor to OMT stream
- [x] OMT output receivable by other OMT clients
- [x] QUIC transport handles packet loss gracefully
- [x] OMT and NDI can run simultaneously without conflicts - [AFTER NDI IMPLTEMENTATION]
- [x] OMT reconnects automatically on network interruption

---

### Phase 5: Web Control Server (Weeks 18–20)

**Goal:** Full HTTP/WebSocket API for remote control.

- [x] **REST API (Axum)**

  **Environment Management**
  - `GET  /api/environment` — get full environment state (resolution, fps, thumbnail mode)
  - `PUT  /api/environment` — update environment settings
  - `GET  /api/environment/effects` — list master effects
  - `POST /api/environment/effects` — add effect to master chain
  - `PUT  /api/environment/effects/:id` — update effect parameters
  - `DELETE /api/environment/effects/:id` — remove effect
  - `POST /api/environment/effects/:id/bypass` — toggle bypass
  - `POST /api/environment/effects/:id/solo` — toggle solo
  - `POST /api/environment/effects/reorder` — reorder effects

  **Layer Management**
  - `GET    /api/layers` — list all layers
  - `POST   /api/layers` — create new layer
  - `GET    /api/layers/:id` — get layer details
  - `PUT    /api/layers/:id` — update layer properties
  - `DELETE /api/layers/:id` — delete layer
  - `POST   /api/layers/:id/clone` — clone layer
  - `POST   /api/layers/reorder` — reorder layers (move_to_front/back)

  **Layer Transform**
  - `PUT  /api/layers/:id/transform` — set position, scale, rotation, anchor
  - `PUT  /api/layers/:id/position` — set x, y position
  - `PUT  /api/layers/:id/scale` — set scale_x, scale_y
  - `PUT  /api/layers/:id/rotation` — set rotation (degrees)

  **Layer Properties**
  - `PUT  /api/layers/:id/opacity` — set opacity (0.0-1.0)
  - `PUT  /api/layers/:id/blend` — set blend mode (Normal, Additive, Multiply, Screen)
  - `PUT  /api/layers/:id/visibility` — set visible (true/false)
  - `PUT  /api/layers/:id/tiling` — set tile_x, tile_y
  - `PUT  /api/layers/:id/transition` — set clip transition (Cut, Fade with duration)

  **Layer Effects**
  - `GET    /api/layers/:id/effects` — list layer effects
  - `POST   /api/layers/:id/effects` — add effect
  - `PUT    /api/layers/:id/effects/:eid` — update effect parameters
  - `DELETE /api/layers/:id/effects/:eid` — remove effect
  - `POST   /api/layers/:id/effects/:eid/bypass` — toggle bypass
  - `POST   /api/layers/:id/effects/:eid/solo` — toggle solo
  - `POST   /api/layers/:id/effects/reorder` — reorder effects

  **Clip Management**
  - `GET    /api/layers/:id/clips` — list all clips in layer
  - `GET    /api/layers/:id/clips/:slot` — get clip at slot
  - `PUT    /api/layers/:id/clips/:slot` — set clip (file path, OMT, or NDI source)
  - `DELETE /api/layers/:id/clips/:slot` — clear clip slot
  - `POST   /api/layers/:id/clips/:slot/trigger` — trigger clip playback
  - `POST   /api/layers/:id/clips/stop` — stop current clip
  - `POST   /api/layers/:id/clips/stop-fade` — stop with fade transition

  **Clip Effects**
  - `GET    /api/layers/:id/clips/:slot/effects` — list clip effects
  - `POST   /api/layers/:id/clips/:slot/effects` — add effect
  - `PUT    /api/layers/:id/clips/:slot/effects/:eid` — update effect
  - `DELETE /api/layers/:id/clips/:slot/effects/:eid` — remove effect
  - `POST   /api/layers/:id/clips/:slot/effects/:eid/bypass` — toggle bypass

  **Clip Clipboard**
  - `POST   /api/layers/:id/clips/:slot/copy` — copy clip to clipboard
  - `POST   /api/layers/:id/clips/:slot/paste` — paste from clipboard

  **Grid Management**
  - `POST   /api/layers/columns` — add column to all layers
  - `DELETE /api/layers/columns/:index` — delete column from all layers

  **Playback Control**
  - `POST /api/playback/pause` — pause all layers
  - `POST /api/playback/resume` — resume all layers
  - `POST /api/playback/toggle` — toggle pause state
  - `POST /api/playback/restart` — restart all videos
  - `GET  /api/playback/status` — get playback state (paused, playing)

  **Per-Layer Playback**
  - `POST /api/layers/:id/playback/pause` — pause specific layer
  - `POST /api/layers/:id/playback/resume` — resume specific layer
  - `POST /api/layers/:id/playback/toggle` — toggle specific layer
  - `POST /api/layers/:id/playback/restart` — restart specific layer video

  **Effects Registry**
  - `GET /api/effects` — list all available effect types
  - `GET /api/effects/:type` — get effect definition (parameters, category)
  - `GET /api/effects/categories` — list effect categories

  **Source Discovery**
  - `GET  /api/sources` — list all discovered sources
  - `GET  /api/sources/omt` — list OMT sources only
  - `GET  /api/sources/ndi` — list NDI sources only
  - `POST /api/sources/omt/refresh` — refresh OMT discovery
  - `POST /api/sources/ndi/start` — start NDI discovery
  - `POST /api/sources/ndi/stop` — stop NDI discovery
  - `POST /api/sources/ndi/refresh` — refresh NDI sources

  **OMT Broadcast**
  - `GET  /api/streaming/omt` — get OMT broadcast status
  - `POST /api/streaming/omt/start` — start OMT broadcast (name, port)
  - `POST /api/streaming/omt/stop` — stop OMT broadcast
  - `PUT  /api/streaming/omt/fps` — set capture FPS (1-60)

  **NDI Broadcast**
  - `GET  /api/streaming/ndi` — get NDI broadcast status
  - `POST /api/streaming/ndi/start` — start NDI broadcast (name)
  - `POST /api/streaming/ndi/stop` — stop NDI broadcast
  - `PUT  /api/streaming/ndi/fps` — set capture FPS (1-60)

  **Texture Sharing (Syphon/Spout)**
  - `GET  /api/streaming/texture` — get texture share status
  - `POST /api/streaming/texture/start` — start texture sharing
  - `POST /api/streaming/texture/stop` — stop texture sharing

  **Output Displays**
  - `GET  /api/outputs` — list connected displays
  - `GET  /api/outputs/:id` — get output configuration
  - `PUT  /api/outputs/:id` — update output config (mapping, blend)

  **Viewport Control**
  - `GET  /api/viewport` — get current viewport state
  - `POST /api/viewport/reset` — reset to fit-to-window
  - `PUT  /api/viewport/zoom` — set zoom level (0.1-8.0)
  - `PUT  /api/viewport/pan` — set pan offset (x, y)

  **File Operations**
  - `GET  /api/files/current` — get current file path
  - `POST /api/files/open` — open environment file
  - `POST /api/files/save` — save to current file
  - `POST /api/files/save-as` — save to new file
  - `GET  /api/files/recent` — list recent files

  **Status & Metrics**
  - `GET /api/status` — full system status
  - `GET /api/status/fps` — current FPS and frame time
  - `GET /api/status/connections` — OMT/NDI connection counts
  - `GET /api/status/performance` — GPU/memory metrics

- [x] **WebSocket (real-time)**
  - Subscribe to composition state changes
  - Push frame rate / performance metrics
  - Bi-directional control commands
  - Event types:
    - `layer:added`, `layer:removed`, `layer:updated`
    - `clip:triggered`, `clip:stopped`
    - `effect:added`, `effect:removed`, `effect:updated`
    - `playback:paused`, `playback:resumed`
    - `source:discovered`, `source:lost`
    - `streaming:started`, `streaming:stopped`
    - `viewport:changed`
    - `file:opened`, `file:saved`
    - `status:fps` (periodic)
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

**✅ Verification Checklist (Phase 5):**

*Server & General*
- [x] Server starts on configurable port (default 8080)
- [x] API responds in < 5ms for control commands
- [x] Token auth blocks unauthorized requests (401 response)
- [x] Valid token allows access
- [x] API handles malformed JSON gracefully (400 response)
- [x] Static files served from `/`

*Environment*
- [ ] `GET /api/environment` returns current environment state
- [ ] `PUT /api/environment` updates resolution, fps, thumbnail mode
- [ ] Environment effects CRUD works (add, update, remove, bypass, solo, reorder)

*Layers*
- [ ] `GET /api/layers` returns all layers
- [ ] `POST /api/layers` creates new layer
- [ ] `GET /api/layers/:id` returns layer details
- [ ] `PUT /api/layers/:id` updates layer properties
- [ ] `DELETE /api/layers/:id` removes layer
- [ ] `POST /api/layers/:id/clone` duplicates layer
- [ ] `POST /api/layers/reorder` changes layer order
- [ ] Transform endpoints work (position, scale, rotation)
- [ ] Property endpoints work (opacity, blend, visibility, tiling, transition)
- [ ] Layer effects CRUD works

*Clips*
- [ ] `GET /api/layers/:id/clips` returns clip grid state
- [ ] `PUT /api/layers/:id/clips/:slot` assigns clip to slot (file, OMT, NDI)
- [ ] `DELETE /api/layers/:id/clips/:slot` clears clip slot
- [ ] `POST /api/layers/:id/clips/:slot/trigger` triggers clip playback
- [ ] `POST /api/layers/:id/clips/stop` stops current clip
- [ ] `POST /api/layers/:id/clips/stop-fade` stops with fade
- [ ] Clip effects CRUD works
- [ ] Copy/paste endpoints work
- [ ] Column add/delete works

*Playback*
- [ ] Global pause/resume/toggle/restart work
- [ ] Per-layer playback controls work
- [ ] `GET /api/playback/status` returns correct state

*Effects*
- [ ] `GET /api/effects` lists all effect types
- [ ] `GET /api/effects/:type` returns effect definition with parameters
- [ ] `GET /api/effects/categories` lists categories

*Sources*
- [ ] `GET /api/sources` returns discovered sources
- [ ] OMT/NDI source filtering works
- [ ] Discovery refresh endpoints work
- [ ] NDI discovery start/stop works

*Streaming*
- [ ] OMT broadcast start/stop works
- [ ] NDI broadcast start/stop works
- [ ] Capture FPS settings work
- [ ] Texture sharing start/stop works
- [ ] Status endpoints return correct state

*Outputs*
- [ ] `GET /api/outputs` lists connected displays
- [ ] `PUT /api/outputs/:id` updates output config

*Viewport*
- [ ] `GET /api/viewport` returns zoom/pan state
- [ ] Reset, zoom, pan endpoints work

*Files*
- [ ] Open/save/save-as work
- [ ] Current file path returned correctly
- [ ] Recent files list works

*Status*
- [ ] `GET /api/status` returns full system status
- [ ] FPS, connections, performance metrics work

*WebSocket*
- [ ] WebSocket connection establishes successfully
- [ ] Receives real-time state updates when layers change
- [ ] Receives FPS/performance metrics
- [ ] All event types fire correctly
- [ ] Bi-directional commands execute correctly

---

### Phase 6: Web Dashboard (Weeks 21–24)

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

**✅ Verification Checklist (Phase 6):**
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

### Phase 7: Polish & Performance (Weeks 25–28)

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

**✅ Verification Checklist (Phase 7):**
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

### Phase 8: REST API Feature Planning (Weeks 29–30)

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

**✅ Verification Checklist (Phase 8):**
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

### Phase 9: Projection Mapping (Weeks 31–34)

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

**✅ Verification Checklist (Phase 9):**
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

### Phase 10: NDI Input/Output (Weeks 35–37)

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

**✅ Verification Checklist (Phase 10):**
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

## 5. UI Guidelines

### Window/Panel Management Rules

1. **All windows and panels must be registered in the View menu** with visibility toggle checkboxes. This ensures users can always respawn a panel that was accidentally closed or lost.

2. **Dockable panels must have fallback rendering** — if a panel can be docked to a zone (Left, Right, Top, Bottom), that zone must have rendering logic. If not implemented, disable the dock zone detection for that edge.

3. **Panel state must persist** — open/closed state and dock position should be saved to settings and restored on app restart.

---

## 6. Key Dependencies

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

## 7. Performance Targets

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

## 8. Risk Mitigation

| Risk | Mitigation |
|------|------------|
| FFmpeg licensing (GPL) | Use LGPL build, or switch to GStreamer |
| NDI SDK distribution | Bundle SDK, respect licensing terms |
| macOS GPU driver quirks | Test on Intel, M1, M2, M3 |
| Windows DX12 fallback | Implement Vulkan backend as fallback |
| Memory pressure (4K video) | Ring buffers with bounded capacity |
| Network congestion (NDI) | Implement adaptive bitrate / frame drop |

---

## 9. Milestones

| Milestone | Target Date | Deliverable |
|-----------|-------------|-------------|
| M1: First Frame | Week 3 | Video file plays in window |
| M2: Environment | Week 6 | Environment with layers and blending |
| M2.5: Video Manipulation | Week 7 | Clone, multiplex, resize, move videos |
| M3: Hardware Decode | Week 12 | GPU-accelerated video decoding |
| M4: OMT Working | Week 17 | OMT streaming input/output |
| M5: Web Control | Week 20 | REST API + basic dashboard |
| M6: Alpha Release | Week 24 | Feature-complete, internal testing |
| M7: Performance | Week 28 | GPU tiling, optimization, polish |
| M8: API Planning | Week 30 | User-driven API feature expansion |
| M9: Projection Mapping | Week 34 | Mesh warp + edge blend |
| M10: NDI Working | Week 37 | Receive/send NDI streams |
| M11: v1.0 Release | Week 40 | Production-ready |

---

## 10. Technical Notes

### XML Serialization (quick-xml)

The `.immersive` project files use XML format via `quick-xml`. This crate has limitations with enum struct variants.

**Problem:** `quick-xml` cannot serialize enum variants with named fields by default:
```rust
// This FAILS to serialize
pub enum ClipSource {
    File { path: PathBuf },  // Struct variant - not supported
    Omt { address: String, name: String },
}
```

**Solution:** Use custom Serialize/Deserialize implementations with helper structs:
```rust
// Helper struct flattens the enum for XML compatibility
#[derive(Serialize, Deserialize)]
struct ClipSourceHelper {
    #[serde(rename = "type")]
    source_type: String,    // "File" or "Omt"
    #[serde(default)]
    path: Option<PathBuf>,  // Only for File
    #[serde(default)]
    address: Option<String>, // Only for Omt
    #[serde(default)]
    name: Option<String>,   // Only for Omt
}

impl Serialize for ClipSource { /* use helper */ }
impl Deserialize for ClipSource { /* use helper */ }
```

**Affected Types (custom serialization implemented):**
- `ClipSource` (`src/compositor/clip.rs`) - helper struct with type discriminant
- `ParameterValue` (`src/effects/types.rs`) - helper struct with type discriminant

---

## 11. References

- [wgpu Documentation](https://wgpu.rs/)
- [NDI SDK](https://ndi.video/for-developers/ndi-sdk/)
- [Open Media Transport](https://www.intopix.com/omt)
- [FFmpeg Hardware Acceleration](https://trac.ffmpeg.org/wiki/HWAccelIntro)
- [Hap Codec](https://hap.video/)
- [Axum Web Framework](https://docs.rs/axum)
- [egui Immediate Mode GUI](https://www.egui.rs/)

---

*Last Updated: December 2024*

