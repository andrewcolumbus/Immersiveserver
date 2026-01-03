// Edge blend shader - applies blend mask to final output.
//
// Multiplies the source texture by a blend mask texture.

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
};

@group(0) @binding(0)
var t_source: texture_2d<f32>;
@group(0) @binding(1)
var t_blend: texture_2d<f32>;
@group(0) @binding(2)
var s_sampler: sampler;

struct BlendParams {
    // Brightness multiplier
    brightness: f32,
    // Gamma correction
    gamma: f32,
    // Black level offset (0-1)
    black_level: f32,
    _padding: f32,
};

@group(1) @binding(0)
var<uniform> params: BlendParams;

// Full-screen triangle vertex shader
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;

    // Generate full-screen triangle
    let x = f32(i32(vertex_index) - 1);
    let y = f32(i32(vertex_index & 1u) * 2 - 1);

    output.position = vec4<f32>(x, y, 0.0, 1.0);
    output.tex_coord = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample source and blend mask
    let source = textureSample(t_source, s_sampler, input.tex_coord);
    let blend = textureSample(t_blend, s_sampler, input.tex_coord).r;

    // Apply blend mask with optional adjustments
    var color = source.rgb;

    // Apply brightness
    color *= params.brightness;

    // Apply gamma correction
    if (params.gamma != 1.0) {
        color = pow(color, vec3<f32>(params.gamma));
    }

    // Apply blend mask
    color *= blend;

    // Add black level offset (for projector black level compensation)
    color += vec3<f32>(params.black_level * (1.0 - blend));

    return vec4<f32>(color, source.a);
}

// Simpler variant without params (just multiplies source by blend mask)
@fragment
fn fs_simple(input: VertexOutput) -> @location(0) vec4<f32> {
    let source = textureSample(t_source, s_sampler, input.tex_coord);
    let blend = textureSample(t_blend, s_sampler, input.tex_coord).r;

    return vec4<f32>(source.rgb * blend, source.a);
}
