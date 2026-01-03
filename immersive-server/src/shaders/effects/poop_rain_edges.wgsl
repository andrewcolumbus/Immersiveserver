// Poop Rain Edge Detection Shader (Sobel)
//
// Outputs edge strength to R channel for collision detection.

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var t_input: texture_2d<f32>;
@group(0) @binding(1) var s_input: sampler;

fn luminance(rgb: vec3<f32>) -> f32 {
    return dot(rgb, vec3<f32>(0.299, 0.587, 0.114));
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32((vertex_index << 1u) & 2u);
    let y = f32(vertex_index & 2u);
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(t_input));
    let texel = 1.0 / dims;

    // Sample 3x3 neighborhood
    let tl = luminance(textureSample(t_input, s_input, in.uv + vec2(-texel.x, -texel.y)).rgb);
    let t  = luminance(textureSample(t_input, s_input, in.uv + vec2(0.0, -texel.y)).rgb);
    let tr = luminance(textureSample(t_input, s_input, in.uv + vec2(texel.x, -texel.y)).rgb);
    let l  = luminance(textureSample(t_input, s_input, in.uv + vec2(-texel.x, 0.0)).rgb);
    let r  = luminance(textureSample(t_input, s_input, in.uv + vec2(texel.x, 0.0)).rgb);
    let bl = luminance(textureSample(t_input, s_input, in.uv + vec2(-texel.x, texel.y)).rgb);
    let b  = luminance(textureSample(t_input, s_input, in.uv + vec2(0.0, texel.y)).rgb);
    let br = luminance(textureSample(t_input, s_input, in.uv + vec2(texel.x, texel.y)).rgb);

    // Sobel kernels
    let gx = -tl - 2.0*l - bl + tr + 2.0*r + br;
    let gy = -tl - 2.0*t - tr + bl + 2.0*b + br;

    let edge = sqrt(gx*gx + gy*gy);

    return vec4<f32>(edge, edge, edge, 1.0);
}
