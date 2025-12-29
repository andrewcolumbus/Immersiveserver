// Edge blending shader for multi-projector setups
//
// Applies soft-edge blending to overlapping projector regions using power curves.

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct BlendUniforms {
    // [width, power, gamma, black_level] for each edge
    left_blend: vec4<f32>,
    right_blend: vec4<f32>,
    top_blend: vec4<f32>,
    bottom_blend: vec4<f32>,
    // [width, height, 0, 0]
    resolution: vec4<f32>,
}

@group(0) @binding(0)
var t_texture: texture_2d<f32>;
@group(0) @binding(1)
var s_sampler: sampler;

@group(1) @binding(0)
var<uniform> uniforms: BlendUniforms;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.uv = input.uv;
    return output;
}

// Calculate blend factor using power curve
fn calculate_blend(t: f32, power: f32, gamma: f32, black_level: f32) -> f32 {
    let t_clamped = clamp(t, 0.0, 1.0);
    let blended = pow(t_clamped, power);
    let gamma_corrected = pow(blended, 1.0 / gamma);
    return gamma_corrected * (1.0 - black_level) + black_level * t_clamped;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_texture, s_sampler, input.uv);
    
    // Calculate pixel position
    let pixel_x = input.uv.x * uniforms.resolution.x;
    let pixel_y = input.uv.y * uniforms.resolution.y;
    
    var blend_factor = 1.0;
    
    // Left edge blend
    let left_width = uniforms.left_blend.x;
    if (left_width > 0.0 && pixel_x < left_width) {
        let t = pixel_x / left_width;
        blend_factor *= calculate_blend(t, uniforms.left_blend.y, uniforms.left_blend.z, uniforms.left_blend.w);
    }
    
    // Right edge blend
    let right_width = uniforms.right_blend.x;
    if (right_width > 0.0 && pixel_x > (uniforms.resolution.x - right_width)) {
        let t = (uniforms.resolution.x - pixel_x) / right_width;
        blend_factor *= calculate_blend(t, uniforms.right_blend.y, uniforms.right_blend.z, uniforms.right_blend.w);
    }
    
    // Top edge blend
    let top_width = uniforms.top_blend.x;
    if (top_width > 0.0 && pixel_y < top_width) {
        let t = pixel_y / top_width;
        blend_factor *= calculate_blend(t, uniforms.top_blend.y, uniforms.top_blend.z, uniforms.top_blend.w);
    }
    
    // Bottom edge blend
    let bottom_width = uniforms.bottom_blend.x;
    if (bottom_width > 0.0 && pixel_y > (uniforms.resolution.y - bottom_width)) {
        let t = (uniforms.resolution.y - pixel_y) / bottom_width;
        blend_factor *= calculate_blend(t, uniforms.bottom_blend.y, uniforms.bottom_blend.z, uniforms.bottom_blend.w);
    }
    
    return vec4<f32>(color.rgb * blend_factor, color.a);
}



