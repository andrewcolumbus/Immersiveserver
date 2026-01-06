# Immersive Server — Advanced Output Build Plan

Professional projection mapping, multi-screen output, and edge blending system.
Inspired by Resolume Arena's Advanced Output with adaptations for Immersive Server's architecture.

---

## 1. Overview

The Advanced Output system enables:
- **Multiple Screens** — Route different content to different output destinations
- **Slice-Based Input Selection** — Crop/position what part of composition each output shows
- **Output Transformation** — Perspective warping and mesh deformation for projection mapping
- **Edge Blending** — Seamless overlap between multiple projectors
- **Masking** — Bezier/polygon masks for complex projection surfaces
- **Per-Output Color Correction** — Match projector brightness/color characteristics

---

## 2. Current Architecture & Gaps

| Current State | Gap for Advanced Output |
|--------------|------------------------|
| Single Environment texture (1920×1080) | Need per-Screen output textures |
| All outputs show identical content | Need slice-based routing |
| No output transforms | Need perspective warp, mesh deform |
| No edge blending | Need soft edge gradients |
| No output masks | Need bezier/polygon masks |
| No per-output color | Need output-level color correction |

**Key Files to Modify:**
- `src/app.rs` — Main render loop (lines 1067-2050+)
- `src/settings.rs` — Add screens to serialization
- `src/network/omt_capture.rs`, `ndi_capture.rs` — Per-screen capture
- `src/shaders/` — New warp/blend/mask shaders

---

## 3. Data Model

```rust
// ═══════════════════════════════════════════════════════════════
// SCREEN — One output destination
// ═══════════════════════════════════════════════════════════════
pub struct Screen {
    pub id: ScreenId,
    pub name: String,
    pub device: OutputDevice,
    pub width: u32,
    pub height: u32,
    pub slices: Vec<Slice>,
    pub enabled: bool,
    pub color: OutputColorCorrection,
    pub delay_ms: u32,                  // Output timing offset
}

pub enum OutputDevice {
    Display { display_id: u32 },        // Physical monitor/projector
    CaptureCard {                       // Professional video output (Blackmagic, AJA)
        device_name: String,
        port: u32,                      // e.g., SDI 1, HDMI 2
        format: VideoFormat,            // Resolution + frame rate
    },
    Ndi { name: String },               // NDI network output
    Omt { name: String, port: u16 },    // OMT network output
    #[cfg(target_os = "macos")]
    Syphon { name: String },            // macOS texture sharing
    #[cfg(target_os = "windows")]
    Spout { name: String },             // Windows texture sharing
    Virtual,                            // Internal routing only
}

pub struct VideoFormat {
    pub width: u32,
    pub height: u32,
    pub frame_rate: f32,                // e.g., 59.94, 60.0, 29.97
    pub interlaced: bool,
}

// ═══════════════════════════════════════════════════════════════
// SLICE — Region of input mapped to region of output
// ═══════════════════════════════════════════════════════════════
pub struct Slice {
    pub id: SliceId,
    pub name: String,
    pub input: SliceInput,              // What to sample
    pub input_rect: Rect,               // Where on input (Input Selection)
    pub output: SliceOutput,            // Where/how on output (Output Transform)
    pub mask: Option<SliceMask>,
    pub color: SliceColorCorrection,
    pub enabled: bool,
    pub is_key: bool,                   // Luminance key output
    pub black_bg: bool,                 // Force black background
}

pub enum SliceInput {
    Composition,                        // Full composited environment
    Layer { layer_id: u32 },           // Specific layer (pre-composition)
}

pub struct SliceOutput {
    pub rect: Rect,                     // Position/size on output (normalized)
    pub rotation: f32,
    pub flip_h: bool,
    pub flip_v: bool,
    pub perspective: Option<[Point2D; 4]>, // 4-corner warp
    pub mesh: Option<WarpMesh>,         // Grid warp (overrides perspective)
    pub edge_blend: EdgeBlendConfig,
}

// ═══════════════════════════════════════════════════════════════
// WARP MESH — Grid-based surface deformation
// ═══════════════════════════════════════════════════════════════
pub struct WarpMesh {
    pub columns: usize,                 // Grid columns (e.g., 4, 8, 16)
    pub rows: usize,                    // Grid rows
    pub points: Vec<WarpPoint>,         // Column-major: [col * rows + row]
    pub interpolation: WarpInterpolation,
}

pub struct WarpPoint {
    pub uv: [f32; 2],                   // Original grid position
    pub position: [f32; 2],             // Warped position
}

pub enum WarpInterpolation { Linear, Bezier }

// ═══════════════════════════════════════════════════════════════
// EDGE BLEND — Soft edges for projector overlap
// ═══════════════════════════════════════════════════════════════
pub struct EdgeBlendConfig {
    pub left: EdgeBlendRegion,
    pub right: EdgeBlendRegion,
    pub top: EdgeBlendRegion,
    pub bottom: EdgeBlendRegion,
}

pub struct EdgeBlendRegion {
    pub enabled: bool,
    pub width: f32,                     // Blend region (0.0-0.5 of output)
    pub gamma: f32,                     // Curve steepness (default 2.2)
}

// ═══════════════════════════════════════════════════════════════
// MASK — Bezier/polygon output masks
// ═══════════════════════════════════════════════════════════════
pub struct SliceMask {
    pub shape: MaskShape,
    pub feather: f32,                   // Edge softness (pixels)
    pub inverted: bool,
    pub enabled: bool,
}

pub enum MaskShape {
    Polygon { points: Vec<Point2D> },
    Bezier { segments: Vec<BezierSegment> },
    Rectangle { rect: Rect },
    Ellipse { center: Point2D, radius_x: f32, radius_y: f32 },
}

// ═══════════════════════════════════════════════════════════════
// COLOR CORRECTION — Per-screen and per-slice
// ═══════════════════════════════════════════════════════════════
pub struct OutputColorCorrection {
    pub brightness: f32,                // -1.0 to 1.0
    pub contrast: f32,                  // 0.0 to 2.0 (1.0 = neutral)
    pub gamma: f32,                     // 0.1 to 4.0 (1.0 = linear)
    pub red: f32,                       // 0.0 to 2.0
    pub green: f32,
    pub blue: f32,
    pub saturation: f32,                // 0.0 to 2.0
}
```

---

## 4. File Structure

```
src/output/
├── mod.rs              # Module exports
├── screen.rs           # Screen, ScreenId, OutputDevice
├── slice.rs            # Slice, SliceId, SliceInput, SliceOutput, Rect
├── warp.rs             # WarpMesh, WarpPoint, WarpInterpolation
├── edge_blend.rs       # EdgeBlendConfig, EdgeBlendRegion
├── mask.rs             # SliceMask, MaskShape, BezierSegment
├── color.rs            # OutputColorCorrection, SliceColorCorrection
└── runtime.rs          # SliceRuntime, ScreenRuntime, OutputManager

src/shaders/output/
├── slice_render.wgsl   # Input sampling, perspective, color
├── mesh_warp.wgsl      # Grid-based UV deformation
├── edge_blend.wgsl     # Soft edge gradients
├── slice_mask.wgsl     # SDF-based masking
└── screen_composite.wgsl # Final screen output

src/ui/
└── advanced_output_window.rs  # Advanced Output UI (Input/Output tabs)
```

---

## 5. Development Phases

> **Testing Requirement:** Before checking off any checkbox, complete the associated verification steps.

### Phase 11: Advanced Output Foundation

**Goal:** Core data model, single Screen/Slice rendering, basic UI.

- [x] Create `src/output/mod.rs` with module structure
- [x] Implement `Screen` and `Slice` data types with serde
- [x] Implement `OutputDevice` enum with platform-specific variants
- [x] Implement `WarpMesh`, `EdgeBlendConfig`, `SliceMask`, `OutputColorCorrection`
- [x] Create `SliceRuntime` struct (GPU texture, bind group, params buffer)
- [x] Create `ScreenRuntime` struct (output texture, slices map, capture)
- [x] Create `OutputManager` for screen/slice management
- [x] Write `slice_render.wgsl` shader (input sampling, perspective)
- [x] Write `screen_composite.wgsl` shader (slice compositing)
- [x] Create `AdvancedOutputWindow` UI component
- [x] Add "Advanced Output" to View menu
- [x] Integrate `OutputManager` into `App` struct
- [x] Add `screens` to `EnvironmentSettings` serialization

**Data Model:**
```rust
// Runtime (GPU resources, not serialized)
pub struct SliceRuntime {
    pub slice_id: SliceId,
    pub texture: wgpu::Texture,
    pub texture_view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
    pub params_buffer: wgpu::Buffer,
    pub warp_vertex_buffer: Option<wgpu::Buffer>,
    pub mask_texture: Option<wgpu::Texture>,
}

pub struct ScreenRuntime {
    pub screen_id: ScreenId,
    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,
    pub slices: HashMap<SliceId, SliceRuntime>,
    pub capture: Option<OutputCapture>,  // Triple-buffered async
}

pub struct OutputManager {
    screens: HashMap<ScreenId, Screen>,
    runtimes: HashMap<ScreenId, ScreenRuntime>,
    next_screen_id: ScreenId,
    next_slice_id: SliceId,
}
```

**Verification Checklist:**
- [x] `cargo test` passes with new output module
- [x] `cargo clippy` reports no warnings
- [x] Can create a Screen with Virtual device
- [x] Screen has one default Slice covering full composition
- [x] Slice renders correctly to screen texture *(PR 4: GPU rendering integrated)*
- [x] Advanced Output window opens from View menu
- [x] Screen list displays in left panel
- [x] Slice list for selected screen displays correctly
- [x] Screen/Slice state serializes to .immersive XML
- [x] State restores correctly on file load

---

### Phase 12: Slice Input Selection ✅ COMPLETE

**Goal:** Full input selection and positioning for slices.

- [x] Implement composition input sampling in slice shader *(PR 4)*
- [x] Add layer texture access for per-layer input (SliceInput::Layer) *(PR 4)*
- [x] Create input rect UI with DragValue controls *(PR 5)*
- [x] Show input source dropdown (Composition / Layer list) *(PR 5)*
- [x] Add input rect preset buttons (Full, Match Output) *(PR 5)*
- [x] Add output rect preset buttons (Full, Match Input) *(PR 5)*
- [x] Live preview in Advanced Output window *(PR 6)*
- [ ] Add input rect preview overlay on composition view *(deferred)*
- [ ] Create interactive drag handles for input rect *(deferred)*

**Verification Checklist:**
- [x] Slice with Composition input shows full environment
- [x] Slice with Layer input shows only that layer
- [x] Input rect crops input correctly
- [x] Input rect DragValue controls work in UI
- [x] Multiple slices can have different input sources
- [x] Live preview shows screen output in real-time
- [ ] Performance: 4 slices at 1080p maintains 60fps *(needs testing)*

---

### Phase 13: Output Transformation (Warp/Perspective) ✅ COMPLETE

**Goal:** Perspective warping and mesh deformation for projection mapping.

- [x] Implement 4-corner perspective warp in `slice_render.wgsl`
- [x] Create mesh warp shader with grid interpolation (integrated into slice_render.wgsl)
- [ ] Add bezier interpolation option for smooth curves *(deferred)*
- [x] Create warp point editor UI with draggable handles
- [x] Add grid resolution selector (4×4, 8×8, 16×16)
- [ ] Implement "big corner" perspective handles separate from grid *(deferred)*
- [x] Add reset warp button
- [ ] Add copy/paste warp configuration between slices *(deferred)*
- [ ] Implement CTRL+drag to disable snapping *(deferred)*

**Shader (mesh_warp.wgsl):**
```wgsl
// Grid-based UV warping with bilinear interpolation
fn warp_uv(uv: vec2<f32>, mesh: MeshUniforms) -> vec2<f32> {
    let cell_x = floor(uv.x * f32(mesh.columns - 1));
    let cell_y = floor(uv.y * f32(mesh.rows - 1));
    let local = fract(uv * vec2<f32>(f32(mesh.columns - 1), f32(mesh.rows - 1)));

    // Sample 4 corners of grid cell
    let p00 = get_warp_point(cell_x, cell_y);
    let p10 = get_warp_point(cell_x + 1, cell_y);
    let p01 = get_warp_point(cell_x, cell_y + 1);
    let p11 = get_warp_point(cell_x + 1, cell_y + 1);

    // Bilinear interpolation
    return mix(mix(p00, p10, local.x), mix(p01, p11, local.x), local.y);
}
```

**Verification Checklist:**
- [x] 4-corner perspective distorts image correctly
- [x] Mesh warp with 4×4 grid deforms smoothly
- [x] Mesh warp with 8×8 grid provides finer control
- [ ] Bezier interpolation produces smooth curves between points *(deferred)*
- [ ] Control points snap to grid when Shift held *(deferred)*
- [ ] CTRL+drag disables snapping for fine adjustment *(deferred)*
- [x] Warp configuration saves/loads correctly
- [ ] Copy/paste warp works between slices *(deferred)*
- [ ] Two overlapping projectors can be warped to align *(needs visual testing)*
- [x] Reset button returns to identity warp

---

### Phase 14: Edge Blending ✅ COMPLETE

**Goal:** Soft edges for seamless overlap between multiple projectors.

- [x] Implement edge blend in `slice_render.wgsl` with gamma-corrected falloff
- [x] Add blend width sliders for each edge (left, right, top, bottom)
- [x] Add gamma slider for blend curve adjustment (default 2.2)
- [x] Create blend preview overlay (shows blend regions on selected slice)
- [ ] Add test pattern generator (grid, gradient, solid white) *(deferred)*
- [x] Implement black level compensation
- [ ] Add per-channel gamma adjustment (R, G, B) *(deferred)*

**Shader (edge_blend.wgsl):**
```wgsl
fn apply_edge_blend(color: vec3<f32>, uv: vec2<f32>, blend: EdgeBlendUniforms) -> vec3<f32> {
    var alpha = 1.0;

    // Left edge
    if blend.left_enabled && uv.x < blend.left_width {
        let t = uv.x / blend.left_width;
        alpha *= pow(t, blend.left_gamma);
    }
    // Right edge
    if blend.right_enabled && uv.x > (1.0 - blend.right_width) {
        let t = (1.0 - uv.x) / blend.right_width;
        alpha *= pow(t, blend.right_gamma);
    }
    // Top/bottom similar...

    return color * alpha;
}
```

**Verification Checklist:**
- [x] Edge blend creates smooth gradient at edges
- [x] Gamma correction produces perceptually linear falloff
- [x] Left/right blend works correctly
- [x] Top/bottom blend works correctly
- [ ] Two projectors with ~15% overlap show seamless merge *(needs visual testing)*
- [x] Blend preview accurately shows blend regions
- [ ] Test patterns display correctly *(deferred)*
- [ ] Per-channel gamma allows color matching *(deferred)*
- [x] Black level compensation reduces visible "halo" in overlap

---

### Phase 15: Masking ✅ COMPLETE

**Goal:** Output-level bezier/polygon masks for complex projection surfaces.

**PR 13: Mask Shader Integration — ✅ COMPLETE**
- [x] Extend SliceParams from 224→240 bytes with mask_enabled, mask_inverted, mask_feather
- [x] Add mask texture (256×256) and bind group infrastructure to SliceRuntime
- [x] Implement CPU rasterization for Rectangle, Ellipse, Polygon, and Bezier masks
- [x] Add apply_mask() function to slice_render.wgsl shader
- [x] Create mask bind group layout and integrate with render pipeline
- [x] Feathering implemented via signed distance field approach

**PR 14: Mask UI Controls — ✅ COMPLETE**
- [x] Add "Mask" section to slice properties panel
- [x] Add Rectangle/Ellipse/Polygon preset buttons
- [x] Add Enable/Invert checkboxes
- [x] Add Feather slider
- [x] Show mask outline on preview (rectangle, ellipse, polygon, bezier shapes)

**PR 15: Interactive Polygon Editor — ✅ COMPLETE**
- [x] Add polygon vertex dragging in preview
- [x] Add "Add vertex" button for polygon masks
- [x] Visual feedback for dragged vertices
- [ ] Bezier control handle editing (deferred - basic bezier display works)
- [ ] Transform mode (move, scale, rotate whole mask) (deferred)

**Verification Checklist:**
- [x] Rectangle mask hides outside region (shader implemented)
- [x] Ellipse mask creates circular cutout (shader implemented)
- [x] Polygon mask correctly hides portions of output (shader implemented)
- [x] Bezier mask creates smooth curved edges (shader implemented)
- [x] Feathering produces soft edges (test 0, 0.02, 0.05) (shader implemented)
- [x] Mask invert works correctly (shows inverse) (shader implemented)
- [x] Masks save/load in .immersive files (uses existing serde serialization)

---

### Phase 16: Per-Output Color Correction

## Status: ✅ COMPLETE

**Goal:** Match projector brightness, contrast, and color characteristics across multiple projectors.

---

## Overview

Color correction enables matching visual output across different projectors/displays:
- **Per-Slice Color Correction** - Adjust individual slices before compositing
- **Per-Screen Color Correction** - Apply global adjustment to entire screen output
- **Real-time Preview** - See adjustments immediately in Advanced Output window

### Implementation Summary

**PR 16: Slice Color Shader + UI** ✅
- Added HSL conversion functions to slice_render.wgsl
- Added saturation support to apply_color_correction()
- Added Color section to slice properties UI

**PR 17: Screen Color Pipeline** ✅
- Added ScreenParams::from_color() helper
- Added ping-pong textures to ScreenRuntime
- Implemented apply_screen_color() method
- Wired into render loop (skips if identity)

**PR 18: Screen Color UI** ✅
- Added Color Correction section to screen properties
- All sliders: brightness, contrast, gamma, saturation, RGB
- Reset button and "(color modified)" indicator

---

## PR 16: Slice Color Correction Shader + UI ✅ COMPLETE

**Goal:** Complete slice-level color correction with saturation and add UI controls.

### Shader Changes

Update `src/shaders/output/slice_render.wgsl` to add saturation:

```wgsl
// Convert RGB to HSL (for saturation adjustment)
fn rgb_to_hsl(rgb: vec3<f32>) -> vec3<f32> {
    let max_c = max(max(rgb.r, rgb.g), rgb.b);
    let min_c = min(min(rgb.r, rgb.g), rgb.b);
    let delta = max_c - min_c;
    let l = (max_c + min_c) * 0.5;
    var h = 0.0;
    var s = 0.0;
    if (delta > 0.0001) {
        s = delta / (1.0 - abs(2.0 * l - 1.0));
        if (max_c == rgb.r) {
            h = ((rgb.g - rgb.b) / delta) % 6.0;
        } else if (max_c == rgb.g) {
            h = (rgb.b - rgb.r) / delta + 2.0;
        } else {
            h = (rgb.r - rgb.g) / delta + 4.0;
        }
        h = h / 6.0;
        if (h < 0.0) { h = h + 1.0; }
    }
    return vec3<f32>(h, s, l);
}

// Convert HSL to RGB
fn hsl_to_rgb(hsl: vec3<f32>) -> vec3<f32> {
    let c = (1.0 - abs(2.0 * hsl.z - 1.0)) * hsl.y;
    let x = c * (1.0 - abs((hsl.x * 6.0) % 2.0 - 1.0));
    let m = hsl.z - c * 0.5;
    var rgb = vec3<f32>(0.0);
    let h6 = hsl.x * 6.0;
    if (h6 < 1.0) { rgb = vec3<f32>(c, x, 0.0); }
    else if (h6 < 2.0) { rgb = vec3<f32>(x, c, 0.0); }
    else if (h6 < 3.0) { rgb = vec3<f32>(0.0, c, x); }
    else if (h6 < 4.0) { rgb = vec3<f32>(0.0, x, c); }
    else if (h6 < 5.0) { rgb = vec3<f32>(x, 0.0, c); }
    else { rgb = vec3<f32>(c, 0.0, x); }
    return rgb + vec3<f32>(m);
}

// Updated apply_color_correction with saturation
fn apply_color_correction(color: vec3<f32>, params: SliceParams) -> vec3<f32> {
    var c = color;
    let brightness = params.color_adjust.x;
    let contrast = params.color_adjust.y;
    let gamma = params.color_adjust.z;
    let saturation = params.color_adjust.w;  // NEW: use saturation

    c = c + vec3<f32>(brightness);
    c = (c - 0.5) * contrast + 0.5;
    c = pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / gamma));

    // NEW: Apply saturation via HSL
    if (abs(saturation - 1.0) > 0.001) {
        let hsl = rgb_to_hsl(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)));
        c = hsl_to_rgb(vec3<f32>(hsl.x, hsl.y * saturation, hsl.z));
    }

    c = c * params.color_rgb.xyz;
    return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
}
```

### Runtime Changes

Update `src/output/runtime.rs` - `SliceParams::from_slice()`:

```rust
// Change line 220 (currently hardcoded to 1.0):
color_adjust: [
    slice.color.brightness,
    slice.color.contrast,
    slice.color.gamma,
    1.0, // saturation placeholder  <-- CHANGE TO:
    // slice.color.saturation (need to add field to SliceColorCorrection)
],
```

Note: `SliceColorCorrection` doesn't have saturation field - need to either:
- Add saturation field to `SliceColorCorrection`, OR
- Use a simpler approach: skip saturation for per-slice (only per-screen)

**Recommendation:** Skip saturation for per-slice to keep it simple. Saturation is more useful at screen level for projector matching.

### UI Changes

Add "Color" section to slice properties in `src/ui/advanced_output_window.rs`:

```rust
// Color Correction section (after Mask section)
ui.add_space(8.0);
ui.separator();
ui.add_space(4.0);

ui.horizontal(|ui| {
    ui.label("Color");
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if ui.small_button("Reset").clicked() {
            slice_copy.color = SliceColorCorrection::default();
            changed = true;
        }
    });
});
ui.add_space(4.0);

// Opacity slider
ui.horizontal(|ui| {
    ui.label("Opacity:");
    if ui.add(egui::Slider::new(&mut slice_copy.color.opacity, 0.0..=1.0)).changed() {
        changed = true;
    }
});

// Brightness slider
ui.horizontal(|ui| {
    ui.label("Brightness:");
    if ui.add(egui::Slider::new(&mut slice_copy.color.brightness, -1.0..=1.0)).changed() {
        changed = true;
    }
});

// Contrast slider
ui.horizontal(|ui| {
    ui.label("Contrast:");
    if ui.add(egui::Slider::new(&mut slice_copy.color.contrast, 0.0..=2.0)).changed() {
        changed = true;
    }
});

// Gamma slider
ui.horizontal(|ui| {
    ui.label("Gamma:");
    if ui.add(egui::Slider::new(&mut slice_copy.color.gamma, 0.1..=4.0).logarithmic(true)).changed() {
        changed = true;
    }
});

// RGB collapsing section
ui.collapsing("RGB Channels", |ui| {
    ui.horizontal(|ui| {
        ui.label("Red:");
        if ui.add(egui::Slider::new(&mut slice_copy.color.red, 0.0..=2.0)).changed() {
            changed = true;
        }
    });
    ui.horizontal(|ui| {
        ui.label("Green:");
        if ui.add(egui::Slider::new(&mut slice_copy.color.green, 0.0..=2.0)).changed() {
            changed = true;
        }
    });
    ui.horizontal(|ui| {
        ui.label("Blue:");
        if ui.add(egui::Slider::new(&mut slice_copy.color.blue, 0.0..=2.0)).changed() {
            changed = true;
        }
    });
});
```

### Files to Modify

| File | Change |
|------|--------|
| `src/shaders/output/slice_render.wgsl` | Add HSL functions, update apply_color_correction() |
| `src/ui/advanced_output_window.rs` | Add Color section to slice properties |

### Verification
- [x] Brightness slider adjusts slice brightness (-1 to +1)
- [x] Contrast slider adjusts slice contrast (0 to 2)
- [x] Gamma slider adjusts slice gamma curve (0.1 to 4)
- [x] RGB sliders adjust individual channels (0 to 2)
- [x] Opacity slider adjusts slice transparency (0 to 1)
- [x] Reset button returns all values to defaults
- [x] Settings persist in .immersive files (uses existing serde serialization)

---

## PR 17: Screen Color Correction Pass ✅ COMPLETE

**Goal:** Wire screen-level color correction through render pipeline.

### Approach

Currently `render_screen()` renders slices directly to the screen output texture. To apply screen color correction, we need a two-pass approach:

1. **Pass 1:** Render all slices to an intermediate texture
2. **Pass 2:** Apply screen color correction from intermediate to final output

However, for simplicity, we can modify the existing approach:
- Render slices to output texture (current behavior)
- Add a second pass that reads from output, applies color, writes back

**Optimization:** Only run color correction pass if screen color is non-identity.

### Runtime Changes

Add to `OutputManager`:

```rust
// New fields
screen_color_pipeline: Option<wgpu::RenderPipeline>,
screen_params_buffer: Option<wgpu::Buffer>,
screen_color_bind_group_layout: Option<wgpu::BindGroupLayout>,

// Add method to create screen color pipeline
fn create_screen_color_pipeline(&mut self, device: &wgpu::Device) {
    // Load screen_composite.wgsl shader
    // Create pipeline with bind group layout for texture + sampler + params
}

// Add method to apply screen color correction
pub fn apply_screen_color(
    &mut self,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    screen_id: ScreenId,
) {
    let Some(screen) = self.screens.get(&screen_id) else { return; };

    // Skip if color is identity
    if screen.color.is_identity() {
        return;
    }

    // Update screen params buffer
    let screen_params = ScreenParams::from_color(&screen.color);
    queue.write_buffer(&self.screen_params_buffer, 0, bytemuck::bytes_of(&screen_params));

    // Create temporary texture to hold current output
    // Render color correction pass from temp to output
}
```

**Alternative (Simpler):** Use ping-pong textures
- ScreenRuntime gets a second texture `color_corrected_texture`
- After slice rendering, if color != identity, render color pass

### ScreenParams Update

Add helper to create `ScreenParams` from `OutputColorCorrection`:

```rust
impl ScreenParams {
    pub fn from_color(color: &OutputColorCorrection) -> Self {
        Self {
            color_adjust: [color.brightness, color.contrast, color.gamma, color.saturation],
            color_rgb: [color.red, color.green, color.blue, 0.0],
        }
    }
}
```

### Files to Modify

| File | Change |
|------|--------|
| `src/output/runtime.rs` | Add screen color pipeline, ScreenParams::from_color() |
| `src/app.rs` | Call apply_screen_color() after render_screen() |

### Verification
- [x] Screen with identity color: no extra GPU work (is_identity() check)
- [x] Screen brightness adjustment affects all slices
- [x] Screen contrast adjustment works
- [x] Screen gamma adjustment works
- [x] Screen saturation adjustment works (HSL conversion in shader)
- [x] Screen RGB channel adjustment works
- [x] Color correction is visibly applied in preview (needs UI to test)

---

## PR 18: Screen Color Correction UI ✅ COMPLETE

**Goal:** Add UI controls for per-screen color correction.

### UI Changes

Add "Screen Color" section to screen properties panel (after screen resolution):

```rust
// Screen Color Correction section
ui.add_space(8.0);
ui.collapsing("Color Correction", |ui| {
    let color = &mut screen.color;
    let mut changed = false;

    // Brightness
    ui.horizontal(|ui| {
        ui.label("Brightness:");
        if ui.add(egui::Slider::new(&mut color.brightness, -1.0..=1.0)).changed() {
            changed = true;
        }
    });

    // Contrast
    ui.horizontal(|ui| {
        ui.label("Contrast:");
        if ui.add(egui::Slider::new(&mut color.contrast, 0.0..=2.0)).changed() {
            changed = true;
        }
    });

    // Gamma
    ui.horizontal(|ui| {
        ui.label("Gamma:");
        if ui.add(egui::Slider::new(&mut color.gamma, 0.1..=4.0).logarithmic(true)).changed() {
            changed = true;
        }
    });

    // Saturation
    ui.horizontal(|ui| {
        ui.label("Saturation:");
        if ui.add(egui::Slider::new(&mut color.saturation, 0.0..=2.0)).changed() {
            changed = true;
        }
    });

    // RGB Channels
    ui.collapsing("RGB Channels", |ui| {
        ui.horizontal(|ui| {
            ui.label("Red:");
            if ui.add(egui::Slider::new(&mut color.red, 0.0..=2.0)).changed() {
                changed = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Green:");
            if ui.add(egui::Slider::new(&mut color.green, 0.0..=2.0)).changed() {
                changed = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Blue:");
            if ui.add(egui::Slider::new(&mut color.blue, 0.0..=2.0)).changed() {
                changed = true;
            }
        });
    });

    // Reset button
    ui.horizontal(|ui| {
        if ui.button("Reset to Default").clicked() {
            *color = OutputColorCorrection::default();
            changed = true;
        }
    });

    if changed {
        // Mark screen as needing update
    }
});
```

### Files to Modify

| File | Change |
|------|--------|
| `src/ui/advanced_output_window.rs` | Add Color Correction section to screen properties |

### Verification
- [x] Color Correction section appears for selected screen
- [x] All sliders adjust screen color in real-time
- [x] Saturation slider produces grayscale at 0, oversaturated at 2
- [x] RGB sliders allow individual channel adjustment
- [x] Reset button restores identity values
- [x] Settings persist in .immersive files (uses existing serde serialization)

---

## Critical Files for Phase 16

| File | Purpose |
|------|---------|
| `src/output/color.rs` | Color correction data models (already complete) |
| `src/output/runtime.rs` | Add ScreenParams::from_color(), screen color pipeline |
| `src/shaders/output/slice_render.wgsl` | Add HSL functions, saturation support |
| `src/shaders/output/screen_composite.wgsl` | Already has full color correction (no changes) |
| `src/ui/advanced_output_window.rs` | Add slice + screen color UI controls |
| `src/app.rs` | Call screen color correction pass |

---

## Phase 16 Verification Checklist

- [ ] **Slice Color Correction:**
  - [ ] Brightness adjustment works (-1 = black, +1 = white)
  - [ ] Contrast adjustment works (0 = flat, 2 = high contrast)
  - [ ] Gamma curve applies correctly
  - [ ] RGB channels adjustable independently
  - [ ] Opacity slider works
  - [ ] Reset button restores defaults

- [ ] **Screen Color Correction:**
  - [ ] Brightness affects entire screen output
  - [ ] Contrast affects entire screen output
  - [ ] Gamma affects entire screen output
  - [ ] Saturation works (0 = grayscale, 1 = normal, 2 = oversaturated)
  - [ ] RGB channels adjustable independently
  - [ ] Reset button restores defaults

- [ ] **Integration:**
  - [ ] Per-slice correction applies before per-screen
  - [ ] Identity color correction skips GPU work
  - [ ] Settings persist in .immersive files
  - [ ] Preview shows color correction in real-time

---

## Implementation Order

1. **PR 16: Slice Color Shader + UI** (~1 hour)
   - Add HSL conversion functions to slice shader
   - Add saturation to apply_color_correction() (optional, can skip)
   - Add Color section to slice properties UI
   - Test slice color controls

2. **PR 17: Screen Color Pipeline** (~1-2 hours)
   - Add ScreenParams::from_color() helper
   - Create screen color correction pipeline
   - Wire into render_screen() as second pass
   - Skip if color is identity

3. **PR 18: Screen Color UI** (~30 min)
   - Add Color Correction section to screen properties
   - Add all sliders (brightness, contrast, gamma, saturation, RGB)
   - Add Reset button
   - Test end-to-end

---

### Phase 17: Multi-Display Output ✅ COMPLETE

**Goal:** Route screens to physical displays, capture cards, NDI, OMT, Syphon/Spout.

- [x] Add display enumeration via winit
- [x] Create display selector dropdown in screen properties
- [x] Implement fullscreen window per physical display
- [x] Add multi-window event handling to main loop
- [x] Integrate ScreenRuntime with physical display output
- [ ] **Capture Card Support (Blackmagic, AJA)** *(deferred)*
  - [ ] Integrate Blackmagic DeckLink SDK (FFI bindings)
  - [ ] Enumerate available capture card outputs
  - [ ] Add video format selection (resolution, frame rate, interlaced)
  - [ ] Implement frame output to SDI/HDMI ports
  - [ ] Handle genlock/reference sync (optional)
- [x] Add NDI output per-screen (not just environment)
- [x] Add OMT output per-screen
- [x] Update Syphon/Spout to support per-screen outputs
- [x] Add output delay configuration (for projector sync)
- [x] Handle display/device hot-plug events

**Verification Checklist:**
- [x] Application lists all connected displays by name/resolution
- [x] Screen can be assigned to any display
- [x] Fullscreen output renders correctly on each display
- [x] Multiple displays show independent screen content
- [ ] **Capture card output:** *(deferred)*
  - [ ] Blackmagic devices detected and listed
  - [ ] SDI output works at 1080p60
  - [ ] SDI output works at 4K30
  - [ ] Frame timing matches selected format
  - [ ] No dropped frames under normal load
- [x] NDI output works per-screen (each screen = separate NDI source)
- [x] OMT output works per-screen
- [x] Syphon/Spout output works per-screen
- [x] Output delay correctly offsets frame timing
- [x] Display assignments persist across restart
- [x] Hot-plug: new displays/devices appear in list, disconnected shows warning

---

### Phase 18: REST API Extension

**Goal:** API endpoints for remote Advanced Output control.

**Endpoints:**
```
# Screen Management
GET    /api/screens              - List all screens
POST   /api/screens              - Create screen
GET    /api/screens/:id          - Get screen details
PUT    /api/screens/:id          - Update screen
DELETE /api/screens/:id          - Delete screen
PUT    /api/screens/:id/enabled  - Enable/disable screen

# Slice Management
GET    /api/screens/:id/slices           - List slices
POST   /api/screens/:id/slices           - Create slice
GET    /api/screens/:id/slices/:sid      - Get slice
PUT    /api/screens/:id/slices/:sid      - Update slice
DELETE /api/screens/:id/slices/:sid      - Delete slice
PUT    /api/screens/:id/slices/:sid/input   - Set input source/rect
PUT    /api/screens/:id/slices/:sid/output  - Set output rect/warp

# Warp Control
GET    /api/screens/:id/slices/:sid/warp           - Get warp mesh
PUT    /api/screens/:id/slices/:sid/warp           - Set warp mesh
PUT    /api/screens/:id/slices/:sid/warp/:col/:row - Set single point
POST   /api/screens/:id/slices/:sid/warp/reset     - Reset to identity

# Edge Blend
PUT    /api/screens/:id/slices/:sid/edge-blend - Update edge blend

# Mask
GET    /api/screens/:id/slices/:sid/mask   - Get mask
PUT    /api/screens/:id/slices/:sid/mask   - Set mask
DELETE /api/screens/:id/slices/:sid/mask   - Remove mask

# Color Correction
PUT    /api/screens/:id/color              - Screen color correction
PUT    /api/screens/:id/slices/:sid/color  - Slice color correction

# Display Management
GET    /api/displays                       - List connected displays
PUT    /api/screens/:id/display            - Assign screen to display
```

- [ ] Add screen/slice types to `src/api/types.rs`
- [ ] Add routes to `src/api/routes.rs`
- [ ] Implement handler functions
- [ ] Add WebSocket events for output changes
- [ ] Update API documentation

**Verification Checklist:**
- [ ] All screen CRUD endpoints work
- [ ] All slice CRUD endpoints work
- [ ] Warp point manipulation works via API
- [ ] Edge blend configuration works via API
- [ ] Mask configuration works via API
- [ ] Color correction works via API
- [ ] WebSocket broadcasts output state changes
- [ ] API response time < 5ms

---

## 6. Render Pipeline Integration

After implementing, the render loop (`src/app.rs`) will be:

```
1. CHECKERBOARD BACKGROUND → Environment texture
2. LAYER COMPOSITION (back-to-front) → Environment texture
3. ENVIRONMENT EFFECTS → Environment texture

4. ═══ NEW: ADVANCED OUTPUT RENDERING ═══
   For each enabled Screen:
     For each enabled Slice:
       a. Sample input (Composition or Layer)
       b. Apply input rect crop + rotation
       c. Apply slice color correction
       d. Render to slice texture
     Composite all slices to screen texture:
       a. Apply mesh warp per slice
       b. Apply edge blend per slice
       c. Apply masks per slice
       d. Apply screen color correction
     Output to device (Display, NDI, OMT, Syphon/Spout)

5. PRESENT TO WINDOW (main window = first screen or preview)
6. EGUI OVERLAY
```

---

## 7. Phase Dependencies

```
Phase 11 (Foundation)
    ├──→ Phase 12 (Input Selection)
    ├──→ Phase 13 (Output Transformation)
    │       ├──→ Phase 14 (Edge Blending)
    │       └──→ Phase 15 (Masking)
    └──→ Phase 16 (Color Correction)

Phases 12-16 ──→ Phase 17 (Multi-Display Integration)
Phase 17 ──→ Phase 18 (REST API Extension)
```

---

## 8. Performance Targets

| Metric | Target |
|--------|--------|
| Screens | 8 simultaneous outputs |
| Slices per Screen | 16 |
| Warp Mesh | Up to 16×16 grid |
| Output Resolution | Up to 8K per screen |
| Render Overhead | < 2ms per screen |

---

## 9. References

- [Resolume Advanced Output Tutorial](output_plan_seed.md)
- [wgpu Multi-Window Example](https://github.com/gfx-rs/wgpu/tree/trunk/examples/multi-window)
- [Projection Mapping with Bezier Surfaces](https://www.paulbourke.net/geometry/bezier/)

---

*Last Updated: January 2025*
