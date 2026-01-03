// Masked passthrough shader - only renders pixels where segmentation mask indicates a person

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var camera_texture: texture_2d<f32>;
@group(0) @binding(1) var camera_sampler: sampler;
@group(0) @binding(2) var mask_texture: texture_2d<f32>;
@group(0) @binding(3) var<uniform> params: MaskParams;

struct MaskParams {
    threshold: f32,
    fade_amount: f32,
    _pad: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;

    // Full-screen triangle
    let x = f32(i32(vertex_index) - 1);
    let y = f32(i32(vertex_index & 1u) * 2 - 1);

    output.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    output.uv = vec2<f32>((x + 1.0) / 2.0, (1.0 - y) / 2.0);

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let camera_color = textureSample(camera_texture, camera_sampler, input.uv);

    // Load mask value using textureLoad (R32Float is not filterable)
    let mask_size = textureDimensions(mask_texture);
    let mask_coord = vec2<i32>(input.uv * vec2<f32>(mask_size));
    let mask_value = textureLoad(mask_texture, mask_coord, 0).r;

    // Apply threshold and fade
    let alpha = smoothstep(params.threshold - 0.1, params.threshold + 0.1, mask_value);
    let final_alpha = alpha * (1.0 - params.fade_amount);

    return vec4<f32>(camera_color.rgb, final_alpha);
}
