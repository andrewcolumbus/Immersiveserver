// Multiplex Effect Shader
//
// Tiles/repeats the input texture in a grid pattern while preserving aspect ratio.
// Each tile shows the full video content fitted within its cell.
//
// Parameters:
//   tile_x (1 to 16) - number of horizontal tiles
//   tile_y (1 to 16) - number of vertical tiles
//   spacing_x (0 to 1) - horizontal gap between tiles (0 = no gap, 1 = max gap)
//   spacing_y (0 to 1) - vertical gap between tiles (0 = no gap, 1 = max gap)
//   size_scale_x/y - video content size relative to environment (for proper content area mapping)

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct EffectParams {
    time: f32,
    delta_time: f32,
    beat_phase: f32,
    bar_phase: f32,
    tile_x: f32,
    tile_y: f32,
    spacing_x: f32,
    spacing_y: f32,
    size_scale_x: f32,
    size_scale_y: f32,
    _pad: vec2<f32>,
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
    let tile_count = max(vec2<f32>(params.tile_x, params.tile_y), vec2<f32>(1.0, 1.0));
    let size_scale = vec2<f32>(params.size_scale_x, params.size_scale_y);
    let spacing = clamp(vec2<f32>(params.spacing_x, params.spacing_y), vec2<f32>(0.0), vec2<f32>(0.99));

    // Determine which tile cell and position within it
    let scaled_uv = in.uv * tile_count;
    let cell_uv = fract(scaled_uv);  // Position within cell (0-1)

    // Calculate aspect ratios (NO spacing distortion here - spacing is mask-only)
    let cell_aspect = tile_count.y / tile_count.x;
    let video_aspect = size_scale.x / size_scale.y;
    let relative_aspect = video_aspect / cell_aspect;

    // Calculate fit_scale to preserve video aspect ratio within cell
    var fit_scale = vec2<f32>(1.0);
    if (relative_aspect > 1.0) {
        // Video is wider than cell - fit to width, letterbox vertically
        fit_scale.y = 1.0 / relative_aspect;
    } else {
        // Video is taller/equal to cell - fit to height, pillarbox horizontally
        fit_scale.x = relative_aspect;
    }

    // Center the fitted video within the cell
    let centered_uv = (cell_uv - 0.5) / fit_scale + 0.5;

    // Bounds check - outside fitted video area is transparent
    if (centered_uv.x < 0.0 || centered_uv.x > 1.0 ||
        centered_uv.y < 0.0 || centered_uv.y > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Map to video content area in texture
    let texture_uv = centered_uv * size_scale + (1.0 - size_scale) * 0.5;

    // Sample the texture (video content is at full size, correct aspect ratio)
    var color = textureSample(t_input, s_input, texture_uv);

    // SPACING MASK: Apply spacing as transparent edges (does NOT affect video content)
    let half_gap = spacing * 0.5;
    let in_x_gap = cell_uv.x < half_gap.x || cell_uv.x > (1.0 - half_gap.x);
    let in_y_gap = cell_uv.y < half_gap.y || cell_uv.y > (1.0 - half_gap.y);

    if (in_x_gap || in_y_gap) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);  // Transparent gap
    }

    return color;
}
