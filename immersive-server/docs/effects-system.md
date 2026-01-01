# Effects System Specification

A Resolume-style effects system for immersive-server with stackable GPU effects, BPM automation, and real-time parameter control.

---

## Overview

The effects system provides:
- **Stackable effects** with bypass/solo/reorder controls
- **Three application levels**: Environment, Layer, Clip
- **Hybrid GPU + CPU processing**
- **BPM/LFO automation** for parameter modulation
- **Effects browser panel** with drag-and-drop
- **Full serialization** in .immersive project files

---

## Architecture

### Data Model (Serializable)

```
EffectStack (Vec<EffectInstance>)
  └── EffectInstance
        ├── id: u32
        ├── effect_type: String
        ├── name: String
        ├── parameters: Vec<Parameter>
        ├── bypassed: bool
        ├── soloed: bool
        └── expanded: bool (UI state)

Parameter
  ├── meta: ParameterMeta (name, label, range, default)
  ├── value: ParameterValue (Float/Int/Bool/Color/Vec2/Vec3/Enum)
  └── automation: Option<AutomationSource>

AutomationSource
  ├── Lfo { shape, frequency, phase, amplitude, sync_to_bpm }
  └── Beat { trigger_on, attack, decay, sustain, release }
```

### Runtime (GPU Resources)

```
EffectStackRuntime
  ├── effect_runtimes: HashMap<u32, EffectRuntimeEntry>
  └── texture_pool: EffectTexturePool (ping-pong textures)

EffectRuntimeEntry
  ├── gpu: Option<Box<dyn GpuEffectRuntime>>
  └── cpu: Option<Box<dyn CpuEffectRuntime>>
```

### Render Integration

Effects process between layer render and environment composite:

```
1. Checkerboard background
2. For each layer:
   a. Render layer content to effect input texture
   b. Process through layer's EffectStack (ping-pong)
   c. Composite result to environment with blend mode
3. Process environment's EffectStack (master effects)
4. Present to window
5. egui overlay
```

---

## File Structure

```
src/effects/
├── mod.rs              # Module exports
├── types.rs            # EffectStack, EffectInstance, Parameter, ParameterValue
├── traits.rs           # EffectDefinition, GpuEffectRuntime, CpuEffectRuntime
├── registry.rs         # EffectRegistry with category support
├── runtime.rs          # EffectStackRuntime, EffectTexturePool
├── automation.rs       # LfoSource, BeatEnvelopeState, BpmClock
├── manager.rs          # EffectManager (coordinates processing)
└── builtin/
    ├── mod.rs          # Registers all built-in effects
    ├── color_correction.rs
    └── invert.rs

src/shaders/effects/
├── common.wgsl         # Shared utilities (HSV conversion, etc.)
├── color_correction.wgsl
└── invert.wgsl

src/ui/
├── effects_browser_panel.rs  # Effects browser with categories
└── properties_panel.rs       # Extended with effect stack UI
```

---

## Creating New Effects

### Step 1: Create the Effect Definition

Create `src/effects/builtin/your_effect.rs`:

```rust
use crate::effects::{
    CpuEffectRuntime, EffectDefinition, EffectParams, EffectProcessor,
    GpuEffectRuntime, ParamBuilder, Parameter, ParameterMeta,
};

/// Your Effect definition
pub struct YourEffectDefinition;

impl EffectDefinition for YourEffectDefinition {
    fn effect_type(&self) -> &'static str {
        "your_effect"
    }

    fn display_name(&self) -> &'static str {
        "Your Effect"
    }

    fn category(&self) -> &'static str {
        "Color"  // or "Blur", "Distort", "Stylize", "Generate"
    }

    fn processor(&self) -> EffectProcessor {
        EffectProcessor::Gpu
    }

    fn default_parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new(ParameterMeta::float("amount", "Amount", 1.0, 0.0, 1.0)),
            Parameter::new(ParameterMeta::bool("enabled", "Enabled", true)),
        ]
    }

    fn create_gpu_runtime(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        output_format: wgpu::TextureFormat,
    ) -> Option<Box<dyn GpuEffectRuntime>> {
        Some(Box::new(YourEffectRuntime::new(device, queue, output_format)))
    }

    fn create_cpu_runtime(&self) -> Option<Box<dyn CpuEffectRuntime>> {
        None
    }
}

/// GPU runtime for the effect
pub struct YourEffectRuntime {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    params_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
}

impl YourEffectRuntime {
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        output_format: wgpu::TextureFormat,
    ) -> Self {
        // Load shader
        let shader_source = include_str!("../../shaders/effects/your_effect.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Your Effect Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Create bind group layout, pipeline, buffers...
        // (See color_correction.rs for full example)

        todo!("Implement GPU resources")
    }
}

impl GpuEffectRuntime for YourEffectRuntime {
    fn effect_type(&self) -> &'static str {
        "your_effect"
    }

    fn process(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        input: &wgpu::TextureView,
        output: &wgpu::TextureView,
        params: &EffectParams,
        queue: &wgpu::Queue,
    ) {
        // Write parameters to GPU buffer
        // Create bind group with input texture
        // Run render pass to output texture
    }
}
```

### Step 2: Create the WGSL Shader

Create `src/shaders/effects/your_effect.wgsl`:

```wgsl
// Vertex shader (fullscreen quad)
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
    );

    let pos = positions[vertex_index];
    var output: VertexOutput;
    output.position = vec4<f32>(pos, 0.0, 1.0);
    output.uv = pos * 0.5 + 0.5;
    output.uv.y = 1.0 - output.uv.y;  // Flip Y
    return output;
}

// Effect parameters
struct Params {
    time: f32,
    delta_time: f32,
    beat_phase: f32,
    bar_phase: f32,
    amount: f32,
    // Add your parameters here
};

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var<uniform> params: Params;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(input_texture, input_sampler, in.uv);

    // Apply your effect here
    let result = color;  // Modify this

    // Blend with original based on amount
    return mix(color, result, params.amount);
}
```

### Step 3: Register the Effect

Update `src/effects/builtin/mod.rs`:

```rust
mod color_correction;
mod invert;
mod your_effect;  // Add this

pub use color_correction::*;
pub use invert::*;
pub use your_effect::*;  // Add this

use super::EffectRegistry;

pub fn register_builtin_effects(registry: &mut EffectRegistry) {
    registry.register(ColorCorrectionDefinition);
    registry.register(InvertDefinition);
    registry.register(YourEffectDefinition);  // Add this
}
```

---

## Key Design Decisions

1. **Ping-pong textures**: Multi-effect chains use two reusable textures that alternate read/write. Avoids per-effect allocation.

2. **Data/Runtime separation**: `EffectStack` (serializable) vs `EffectStackRuntime` (GPU resources). Follows existing Layer/LayerRuntime pattern.

3. **Effect registration**: Factory pattern via `EffectDefinition` trait. Effects register with `EffectRegistry` at startup.

4. **Automation evaluation**: `BpmClock` updates each frame. Parameters with automation are evaluated at render time before passing to shaders.

5. **CPU effects**: Background thread with channels (similar to video decoder). Double-buffered with one-frame latency for synchronous effects.

---

## Built-in Effects

### Color Correction (GPU)

**Category:** Color

| Parameter | Type | Range | Default | Description |
|-----------|------|-------|---------|-------------|
| brightness | Float | -1.0 to 1.0 | 0.0 | Brightness adjustment |
| contrast | Float | 0.0 to 2.0 | 1.0 | Contrast multiplier |
| saturation | Float | 0.0 to 2.0 | 1.0 | Saturation multiplier |
| hue_shift | Float | 0.0 to 1.0 | 0.0 | Hue rotation (0-1 = 0-360°) |
| gamma | Float | 0.1 to 3.0 | 1.0 | Gamma correction |

### Invert (GPU)

**Category:** Color

| Parameter | Type | Range | Default | Description |
|-----------|------|-------|---------|-------------|
| amount | Float | 0.0 to 1.0 | 1.0 | Blend with original |
| invert_alpha | Bool | - | false | Also invert alpha channel |

---

## UI Controls

### Effects Browser Panel (View → Effects)

- Category tree with expandable sections
- Search filter
- Drag effects to layers/clips

### Properties Panel → Layer Tab → Effects Section

- **+ Add Effect** dropdown menu
- Per-effect controls:
  - **B** (Bypass) - Green when active, gray when bypassed
  - **S** (Solo) - Yellow when soloed
  - **▲/▼** - Reorder in chain
  - **✕** - Remove effect
- Parameter sliders/checkboxes based on type
- **Right-click** any slider or value to reset to default
- Bypassed effects show strikethrough name

---

## Automation System

### BPM Clock

```rust
let bpm_clock = BpmClock::new(120.0);
bpm_clock.set_bpm(140.0);
bpm_clock.tap();  // Tap tempo
let phase = bpm_clock.beat_phase();  // 0.0 to 1.0
```

### LFO Shapes

- **Sine** - Smooth oscillation
- **Triangle** - Linear up/down
- **Square** - On/off toggle
- **Sawtooth** - Linear ramp, instant reset
- **Random** - Random value each cycle

### Beat Envelope (ADSR)

Trigger effects on beat with Attack, Decay, Sustain, Release envelope.

---

## Performance Considerations

- Effects use shared ping-pong textures (2 textures per layer, reused across effects)
- GPU effects run in the render loop, no CPU readback
- Effect runtimes are created once and reused
- Parameters packed into single uniform buffer per effect
