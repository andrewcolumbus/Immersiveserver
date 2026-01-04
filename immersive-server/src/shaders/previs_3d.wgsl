// 3D Previs Shader
// Renders environment texture onto 3D mesh surfaces (circle, walls, dome)
// Supports separate floor texture for walls mode

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _padding: f32,
}

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) tex_index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) world_position: vec3<f32>,
    @location(3) @interpolate(flat) tex_index: u32,
}

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var env_texture: texture_2d<f32>;
@group(0) @binding(2) var env_sampler: sampler;
@group(0) @binding(3) var floor_texture: texture_2d<f32>;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.uv = in.uv;
    out.world_normal = in.normal; // No model transform, world = model
    out.world_position = in.position;
    out.tex_index = in.tex_index;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample from appropriate texture based on tex_index
    var color: vec4<f32>;
    if (in.tex_index == 0u) {
        // Walls/environment texture
        color = textureSample(env_texture, env_sampler, in.uv);
    } else {
        // Floor texture (layer 0)
        color = textureSample(floor_texture, env_sampler, in.uv);
    }

    // Simple ambient + directional lighting for depth perception
    let ambient = 0.4;
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let normal = normalize(in.world_normal);

    // Diffuse lighting (abs for inside-facing surfaces)
    let ndotl = abs(dot(normal, light_dir));
    let diffuse = ndotl * 0.6;

    let lighting = ambient + diffuse;

    return vec4<f32>(color.rgb * lighting, color.a);
}
