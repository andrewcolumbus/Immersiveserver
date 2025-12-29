// Warping shader for geometric correction
//
// Supports perspective and bezier warping for projection surface alignment.

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct WarpUniforms {
    // Perspective warp corners (normalized 0-1)
    // [tl.x, tl.y, tr.x, tr.y]
    corners_01: vec4<f32>,
    // [br.x, br.y, bl.x, bl.y]
    corners_23: vec4<f32>,
    // [width, height, warp_mode, 0]
    // warp_mode: 0 = none, 1 = perspective, 2 = bezier
    config: vec4<f32>,
}

@group(0) @binding(0)
var t_texture: texture_2d<f32>;
@group(0) @binding(1)
var s_sampler: sampler;

@group(1) @binding(0)
var<uniform> uniforms: WarpUniforms;

// Bilinear interpolation for perspective warp
fn bilinear_interp(uv: vec2<f32>, tl: vec2<f32>, tr: vec2<f32>, bl: vec2<f32>, br: vec2<f32>) -> vec2<f32> {
    let top = mix(tl, tr, uv.x);
    let bottom = mix(bl, br, uv.x);
    return mix(top, bottom, uv.y);
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    
    let warp_mode = i32(uniforms.config.z);
    
    if (warp_mode == 1) {
        // Perspective warp
        let tl = uniforms.corners_01.xy;
        let tr = uniforms.corners_01.zw;
        let br = uniforms.corners_23.xy;
        let bl = uniforms.corners_23.zw;
        
        // Convert input UV (0-1) to warped position (-1 to 1)
        let warped_uv = bilinear_interp(input.uv, tl, tr, bl, br);
        let warped_pos = warped_uv * 2.0 - 1.0;
        
        output.position = vec4<f32>(warped_pos, 0.0, 1.0);
    } else {
        // No warp or bezier (bezier uses pre-computed mesh)
        output.position = vec4<f32>(input.position, 0.0, 1.0);
    }
    
    output.uv = input.uv;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_texture, s_sampler, input.uv);
}



