// Slide Effect Shader
//
// Shifts the source image horizontally (X) and/or vertically (Y).
//
// Parameters:
//   params[0] = offset_x (-2 to 2, default 0) - horizontal shift
//   params[1] = offset_y (-2 to 2, default 0) - vertical shift
//   params[2] = wrap_mode (0=Clamp, 1=Repeat, 2=Mirror)

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct EffectParams {
    time: f32,
    delta_time: f32,
    beat_phase: f32,
    bar_phase: f32,
    offset_x: f32,    // params[0]
    offset_y: f32,    // params[1]
    wrap_mode: f32,   // params[2]
    _pad: f32,        // Padding for alignment
}

@group(0) @binding(0) var t_input: texture_2d<f32>;
@group(0) @binding(1) var s_input: sampler;
@group(0) @binding(2) var<uniform> params: EffectParams;

// ============================================================================
// Vertex Shader
// ============================================================================

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Fullscreen triangle
    let x = f32((vertex_index << 1u) & 2u);
    let y = f32(vertex_index & 2u);
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

// ============================================================================
// Fragment Shader
// ============================================================================

// Mirror wrapping function - reflects at boundaries
fn mirror_wrap(uv: vec2<f32>) -> vec2<f32> {
    // Use modular arithmetic to create mirroring
    let period = floor(uv);
    let frac_uv = fract(uv);

    // If period is odd, flip the coordinate
    let flip_x = select(frac_uv.x, 1.0 - frac_uv.x, i32(period.x) % 2 != 0);
    let flip_y = select(frac_uv.y, 1.0 - frac_uv.y, i32(period.y) % 2 != 0);

    return vec2<f32>(flip_x, flip_y);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Apply offset to UV coordinates
    var uv = in.uv + vec2<f32>(params.offset_x, params.offset_y);

    // Handle wrap modes
    if (params.wrap_mode > 1.5) {
        // Mirror mode
        uv = mirror_wrap(uv);
    } else if (params.wrap_mode > 0.5) {
        // Repeat mode - wrap around
        uv = fract(uv);
    }
    // Clamp mode (default) - sampler handles clamping, but we need to
    // return transparent for out-of-bounds areas

    // For clamp mode, check if UV is out of bounds and return transparent
    if (params.wrap_mode < 0.5) {
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
    }

    let color = textureSample(t_input, s_input, uv);
    return color;
}
