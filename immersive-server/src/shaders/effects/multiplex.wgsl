// Multiplex Effect Shader
//
// Creates horizontal copies of the video at its exact size.
// Copies are centered in the environment and spacing pushes them apart.
//
// Parameters:
//   copies (1 to 16) - number of horizontal copies (1 = pass-through)
//   spacing (0 to 1) - gap between copies as fraction of video width (0 = touching, 1 = one video-width gap)
//   size_scale_x/y - video dimensions relative to environment (from copy shader)

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct EffectParams {
    time: f32,
    delta_time: f32,
    beat_phase: f32,
    bar_phase: f32,
    copies: f32,        // Number of horizontal copies (1 = pass-through)
    spacing: f32,       // Gap between copies as fraction of video width (0 = touching)
    size_scale_x: f32,  // Video width / env width
    size_scale_y: f32,  // Video height / env height
    _pad: vec4<f32>,
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
// Fragment Shader
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let copies = max(params.copies, 1.0);
    let spacing = max(params.spacing, 0.0);
    let size_scale = vec2<f32>(params.size_scale_x, params.size_scale_y);

    // Pass-through: when copies=1, return input unchanged
    if (copies <= 1.0) {
        return textureSample(t_input, s_input, in.uv);
    }

    // Video dimensions in UV space (how much of the environment the video occupies)
    let copy_width = size_scale.x;
    let copy_height = size_scale.y;
    let gap_width = spacing * size_scale.x;  // Video-relative (fraction of video width)

    // Total width of all copies + gaps between them
    let total_width = copies * copy_width + (copies - 1.0) * gap_width;

    // Center the group of copies horizontally in the environment
    let start_x = 0.5 - total_width / 2.0;
    let offset_x = in.uv.x - start_x;

    // Outside the group of copies? Return transparent
    if (offset_x < 0.0 || offset_x >= total_width) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Calculate stride (copy width + gap) and find which copy we're in
    let stride = copy_width + gap_width;
    let copy_index = floor(offset_x / stride);
    let pos_in_stride = offset_x - copy_index * stride;

    // In a gap between copies? Return transparent
    // (Only check for gaps if we're not on the last copy)
    if (copy_index < copies - 1.0 && pos_in_stride >= copy_width) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Past the last copy? Return transparent
    if (copy_index >= copies) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Local position within the video (0 to 1)
    let local_x = min(pos_in_stride / copy_width, 1.0);

    // Vertical bounds check - video is centered vertically
    let video_top = 0.5 - copy_height / 2.0;
    let video_bottom = 0.5 + copy_height / 2.0;

    if (in.uv.y < video_top || in.uv.y > video_bottom) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    let local_y = (in.uv.y - video_top) / copy_height;

    // Sample from the video content area in the input texture
    // The video is centered in the input texture at size_scale dimensions
    let video_left = 0.5 - size_scale.x / 2.0;
    let sample_uv = vec2<f32>(
        video_left + local_x * size_scale.x,
        video_top + local_y * size_scale.y
    );

    return textureSample(t_input, s_input, sample_uv);
}
