// Paint Warp effect shader
// Creates fluid displacement that smears the image like wet paint

struct TouchPoint {
    pos: vec2<f32>,
    prev_pos: vec2<f32>,
    pressure: f32,
    active: f32,
    _pad: vec2<f32>,
}

struct TouchData {
    points: array<TouchPoint, 10>,
    point_count: u32,
    _pad: vec3<u32>,
}

struct PaintWarpParams {
    time: f32,
    delta_time: f32,
    viscosity: f32,
    displacement_strength: f32,
    brush_radius: f32,
    brush_softness: f32,
    smear_length: f32,
    flow_speed: f32,
}

// Displacement field update compute shader
@group(0) @binding(0) var displacement_in: texture_2d<f32>;
@group(0) @binding(1) var displacement_out: texture_storage_2d<rg32float, write>;
@group(0) @binding(2) var<uniform> params: PaintWarpParams;
@group(0) @binding(3) var<uniform> touch_data: TouchData;

@compute @workgroup_size(8, 8)
fn update_displacement(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(displacement_in);
    if global_id.x >= dims.x || global_id.y >= dims.y {
        return;
    }

    let uv = vec2<f32>(f32(global_id.x) / f32(dims.x), f32(global_id.y) / f32(dims.y));

    // Read current displacement
    var current = textureLoad(displacement_in, vec2<i32>(global_id.xy), 0).xy;

    // Apply viscosity decay
    current *= params.viscosity;

    // Add displacement from touch points
    for (var i = 0u; i < touch_data.point_count; i++) {
        let touch = touch_data.points[i];
        if touch.active < 0.5 {
            continue;
        }

        // Calculate distance to touch point
        let dist = length(uv - touch.pos);

        // Brush falloff
        let falloff = 1.0 - smoothstep(
            params.brush_radius * (1.0 - params.brush_softness),
            params.brush_radius,
            dist
        );

        if falloff > 0.001 {
            // Calculate touch velocity
            let velocity = (touch.pos - touch.prev_pos) * params.smear_length;

            // Add displacement based on velocity
            current += velocity * falloff * params.displacement_strength * touch.pressure;
        }
    }

    // Clamp displacement
    let max_disp = 0.3;
    current = clamp(current, vec2<f32>(-max_disp), vec2<f32>(max_disp));

    textureStore(displacement_out, vec2<i32>(global_id.xy), vec4<f32>(current, 0.0, 1.0));
}

// Apply displacement fragment shader
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var camera_texture: texture_2d<f32>;
@group(0) @binding(1) var displacement_texture: texture_2d<f32>;
@group(0) @binding(2) var tex_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;

    // Fullscreen triangle
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index >> 1u) * 4 - 1);

    output.position = vec4<f32>(x, y, 0.0, 1.0);
    output.uv = vec2<f32>((x + 1.0) * 0.5, 1.0 - (y + 1.0) * 0.5);

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample displacement
    let displacement = textureSample(displacement_texture, tex_sampler, input.uv).xy;

    // Apply displacement to UV
    let warped_uv = input.uv + displacement;

    // Sample camera at warped position
    let color = textureSample(camera_texture, tex_sampler, warped_uv);

    return color;
}
