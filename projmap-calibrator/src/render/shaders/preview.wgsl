// Camera preview shader - displays BGRA camera frame.

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
};

@group(0) @binding(0)
var t_camera: texture_2d<f32>;
@group(0) @binding(1)
var s_camera: sampler;

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
    let bgra = textureSample(t_camera, s_camera, input.tex_coord);
    // BGRA to RGBA swap (camera frames are typically BGRA)
    return vec4<f32>(bgra.b, bgra.g, bgra.r, bgra.a);
}
