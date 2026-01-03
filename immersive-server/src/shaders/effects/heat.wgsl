// Heat/Thermal Camera Effect Shader
//
// Converts the image to a thermal camera look by mapping luminance to a heat color palette.
//
// Parameters:
//   amount (0 to 1, default 1) - blend between original and heat effect
//   sensitivity (0.5 to 2, default 1) - adjust contrast/sensitivity of heat detection
//   cold_offset (-0.5 to 0.5, default 0) - shift the temperature range

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct EffectParams {
    time: f32,
    delta_time: f32,
    beat_phase: f32,
    bar_phase: f32,
    amount: f32,       // params[0]
    sensitivity: f32,  // params[1]
    cold_offset: f32,  // params[2]
    _pad: f32,         // Padding for alignment
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
// Heat Color Palette
// ============================================================================

// Classic thermal/infrared palette: black -> blue -> magenta -> red -> orange -> yellow -> white
fn heat_palette(t: f32) -> vec3<f32> {
    // Clamp input to 0-1
    let temp = clamp(t, 0.0, 1.0);

    // Color stops for thermal palette
    // 0.0 = black (cold)
    // 0.15 = deep blue
    // 0.3 = blue/purple
    // 0.45 = magenta/red
    // 0.6 = red/orange
    // 0.75 = orange/yellow
    // 0.9 = yellow
    // 1.0 = white (hot)

    var color: vec3<f32>;

    if (temp < 0.15) {
        // Black to deep blue
        let local_t = temp / 0.15;
        color = mix(vec3<f32>(0.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 0.5), local_t);
    } else if (temp < 0.3) {
        // Deep blue to blue/purple
        let local_t = (temp - 0.15) / 0.15;
        color = mix(vec3<f32>(0.0, 0.0, 0.5), vec3<f32>(0.3, 0.0, 0.7), local_t);
    } else if (temp < 0.45) {
        // Blue/purple to magenta
        let local_t = (temp - 0.3) / 0.15;
        color = mix(vec3<f32>(0.3, 0.0, 0.7), vec3<f32>(0.8, 0.0, 0.5), local_t);
    } else if (temp < 0.6) {
        // Magenta to red
        let local_t = (temp - 0.45) / 0.15;
        color = mix(vec3<f32>(0.8, 0.0, 0.5), vec3<f32>(1.0, 0.1, 0.0), local_t);
    } else if (temp < 0.75) {
        // Red to orange
        let local_t = (temp - 0.6) / 0.15;
        color = mix(vec3<f32>(1.0, 0.1, 0.0), vec3<f32>(1.0, 0.5, 0.0), local_t);
    } else if (temp < 0.9) {
        // Orange to yellow
        let local_t = (temp - 0.75) / 0.15;
        color = mix(vec3<f32>(1.0, 0.5, 0.0), vec3<f32>(1.0, 1.0, 0.0), local_t);
    } else {
        // Yellow to white
        let local_t = (temp - 0.9) / 0.1;
        color = mix(vec3<f32>(1.0, 1.0, 0.0), vec3<f32>(1.0, 1.0, 1.0), local_t);
    }

    return color;
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

    // Apply sensitivity and offset
    // Sensitivity > 1 increases contrast, < 1 decreases it
    // Offset shifts the temperature range (negative = colder overall, positive = hotter)
    let adjusted = (luminance - 0.5) * params.sensitivity + 0.5 + params.cold_offset;

    // Map to heat palette
    let heat_color = heat_palette(adjusted);

    // Blend between original and heat effect
    let final_color = mix(color.rgb, heat_color, params.amount);

    return vec4<f32>(final_color, color.a);
}
