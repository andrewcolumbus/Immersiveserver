// Poop Rain Effect Shader - Instanced Quad Rendering
//
// Each particle is rendered as an instanced quad, not per-pixel loops.

struct VertexInput {
    @location(0) local_pos: vec2<f32>,  // Quad corner (-1 to 1)
    @location(1) local_uv: vec2<f32>,   // UV for emoji texture (0 to 1)
}

struct InstanceInput {
    @location(2) pos: vec2<f32>,        // Screen position (0-1, Y down)
    @location(3) size_rot: vec2<f32>,   // Size and rotation
    @location(4) alpha: f32,            // Opacity
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha: f32,
}

@group(0) @binding(0) var t_emoji: texture_2d<f32>;
@group(0) @binding(1) var s_emoji: sampler;

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    let size = instance.size_rot.x;
    let rotation = instance.size_rot.y;

    // Aspect ratio correction (16:9 = 1920/1080)
    let aspect_ratio = 1920.0 / 1080.0;

    // Apply rotation
    let cos_r = cos(rotation);
    let sin_r = sin(rotation);
    let rotated = vec2<f32>(
        vertex.local_pos.x * cos_r - vertex.local_pos.y * sin_r,
        vertex.local_pos.x * sin_r + vertex.local_pos.y * cos_r
    );

    // Scale by size, correcting X for aspect ratio to keep emoji square
    let scaled = vec2<f32>(
        rotated.x * size / aspect_ratio,
        rotated.y * size
    );

    // Translate to screen position (instance.pos is in 0-1 space)
    // Convert to clip space: 0-1 -> -1 to 1, with Y flipped
    let screen_pos = instance.pos + scaled;
    let clip_pos = vec2<f32>(
        screen_pos.x * 2.0 - 1.0,
        1.0 - screen_pos.y * 2.0  // Flip Y
    );

    out.position = vec4<f32>(clip_pos, 0.0, 1.0);
    out.uv = vertex.local_uv;
    out.alpha = instance.alpha;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let emoji = textureSample(t_emoji, s_emoji, in.uv);
    return vec4<f32>(emoji.rgb, emoji.a * in.alpha);
}
