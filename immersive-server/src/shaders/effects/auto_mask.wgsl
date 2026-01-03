// Auto Mask Effect Shader (Resolume-style Luma Key)
//
// Keys out dark (or bright) areas of video based on luminance,
// making them transparent.
//
// Parameters:
//   threshold (0 to 1, default 0.1) - luminance cutoff point
//   softness (0 to 0.5, default 0.1) - feather/transition width
//   invert (0 or 1, default 0) - key out bright areas instead
//   amount (0 to 1, default 1) - blend between original and masked

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct EffectParams {
    time: f32,
    delta_time: f32,
    beat_phase: f32,
    bar_phase: f32,
    threshold: f32,  // params[0]
    softness: f32,   // params[1]
    invert: f32,     // params[2] (0 or 1)
    amount: f32,     // params[3]
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

    // Calculate luminance (perceived brightness)
    // Using standard luminance coefficients for sRGB
    let luminance = dot(color.rgb, vec3<f32>(0.299, 0.587, 0.114));

    // Calculate mask using smoothstep for gradual transition
    // When softness is 0, this becomes a hard cutoff
    let soft_half = params.softness * 0.5;
    let lower = params.threshold - soft_half;
    let upper = params.threshold + soft_half;

    // smoothstep gives 0 below lower, 1 above upper, smooth transition between
    var mask = smoothstep(lower, upper, luminance);

    // Invert if requested (key out bright instead of dark)
    if (params.invert > 0.5) {
        mask = 1.0 - mask;
    }

    // Apply mask to alpha channel
    let masked_alpha = color.a * mask;

    // Blend between original and masked based on amount
    let final_alpha = mix(color.a, masked_alpha, params.amount);

    return vec4<f32>(color.rgb, final_alpha);
}
