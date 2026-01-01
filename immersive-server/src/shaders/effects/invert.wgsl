// Invert Effect Shader
//
// Inverts the colors of the input.
//
// Parameters:
//   params[0] = amount (0 to 1, default 1) - blend between original and inverted
//   params[1] = invert_alpha (0 or 1, default 0) - whether to invert alpha channel

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct EffectParams {
    time: f32,
    delta_time: f32,
    beat_phase: f32,
    bar_phase: f32,
    amount: f32,        // params[0]
    invert_alpha: f32,  // params[1]
    _pad: vec2<f32>,    // Padding for alignment
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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_input, s_input, in.uv);

    // Invert RGB
    let inverted_rgb = vec3<f32>(1.0) - color.rgb;

    // Optionally invert alpha
    let inverted_alpha = select(color.a, 1.0 - color.a, params.invert_alpha > 0.5);

    // Blend between original and inverted based on amount
    let final_rgb = mix(color.rgb, inverted_rgb, params.amount);
    let final_alpha = mix(color.a, inverted_alpha, params.amount);

    return vec4<f32>(final_rgb, final_alpha);
}
