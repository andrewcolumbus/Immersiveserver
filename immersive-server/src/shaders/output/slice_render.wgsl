// Slice Render Shader
//
// Renders a slice by sampling from the composition (or layer) texture
// with input rect cropping, output positioning, rotation, flipping, and color correction.

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Slice parameters uniform
// IMPORTANT: This struct must match the Rust SliceParams layout exactly (240 bytes)
struct SliceParams {
    // Input rect (x, y, width, height) - normalized 0.0-1.0
    input_rect: vec4<f32>,
    // Output rect (x, y, width, height) - normalized 0.0-1.0
    output_rect: vec4<f32>,
    // Rotation in radians
    rotation: f32,
    // Flip flags (x = horizontal, y = vertical)
    flip: vec2<f32>,
    // Opacity
    opacity: f32,
    // Color correction: brightness, contrast, gamma, saturation
    color_adjust: vec4<f32>,
    // RGB channel multipliers + padding
    color_rgb: vec4<f32>,
    // Perspective warp corners (normalized 0-1, relative to output rect)
    perspective_tl: vec2<f32>,  // Top-left
    perspective_tr: vec2<f32>,  // Top-right
    perspective_br: vec2<f32>,  // Bottom-right
    perspective_bl: vec2<f32>,  // Bottom-left
    // Perspective enabled flag (1.0 = enabled, 0.0 = disabled)
    perspective_enabled: f32,
    // Padding for perspective
    _pad0: vec3<f32>,
    // Mesh warp parameters
    mesh_columns: u32,
    mesh_rows: u32,
    mesh_enabled: f32,
    _pad1: f32,
    // Edge blend: [enabled, width, gamma, black_level] for each edge
    edge_left: vec4<f32>,
    edge_right: vec4<f32>,
    edge_top: vec4<f32>,
    edge_bottom: vec4<f32>,
    // Mask parameters
    mask_enabled: f32,    // 1.0 = enabled, 0.0 = disabled
    mask_inverted: f32,   // 1.0 = show outside, 0.0 = show inside
    mask_feather: f32,    // Feather amount (0.0-0.5)
    _pad2: f32,
}

// Warp point in storage buffer: [uv.x, uv.y, position.x, position.y]
struct WarpPoint {
    uv: vec2<f32>,
    position: vec2<f32>,
}

@group(0) @binding(0) var t_input: texture_2d<f32>;
@group(0) @binding(1) var s_input: sampler;
@group(0) @binding(2) var<uniform> params: SliceParams;

// Optional: Mesh warp points storage buffer (bind group 1)
// Only bound when mesh warp is enabled
@group(1) @binding(0) var<storage, read> warp_points: array<WarpPoint>;

// Optional: Mask texture (bind group 2)
// Only bound when masking is enabled
@group(2) @binding(0) var t_mask: texture_2d<f32>;
@group(2) @binding(1) var s_mask: sampler;

// Vertex shader - generates fullscreen triangle
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // Generate fullscreen triangle coordinates
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index >> 1u) * 4 - 1);

    out.position = vec4<f32>(x, y, 0.0, 1.0);

    // UV coordinates (0,0 at top-left, 1,1 at bottom-right)
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);

    return out;
}

// Apply perspective warp using bilinear interpolation
// Maps UV coordinates through the perspective quad defined by the 4 corners
fn apply_perspective_warp(uv: vec2<f32>) -> vec2<f32> {
    if (params.perspective_enabled < 0.5) {
        return uv;
    }

    // Bilinear interpolation: map UV through the perspective quad
    // uv.x interpolates along horizontal edges, uv.y interpolates vertically
    let top = mix(params.perspective_tl, params.perspective_tr, uv.x);
    let bottom = mix(params.perspective_bl, params.perspective_br, uv.x);
    return mix(top, bottom, uv.y);
}

// Get warp point from storage buffer by grid coordinates
// Points are stored column-major: index = col * rows + row
fn get_warp_point(col: u32, row: u32) -> WarpPoint {
    let idx = col * params.mesh_rows + row;
    return warp_points[idx];
}

// Apply mesh warp using bilinear interpolation between grid cells
// Maps UV coordinates through the warp mesh grid
fn apply_mesh_warp(uv: vec2<f32>) -> vec2<f32> {
    if (params.mesh_enabled < 0.5 || params.mesh_columns < 2u || params.mesh_rows < 2u) {
        return uv;
    }

    // Scale UV to grid space (0 to columns-1, 0 to rows-1)
    let grid_uv = uv * vec2<f32>(f32(params.mesh_columns - 1u), f32(params.mesh_rows - 1u));

    // Find which cell we're in
    let cell = floor(grid_uv);
    let local = fract(grid_uv);

    // Clamp cell indices to valid range
    let col = u32(clamp(cell.x, 0.0, f32(params.mesh_columns - 2u)));
    let row = u32(clamp(cell.y, 0.0, f32(params.mesh_rows - 2u)));

    // Get the 4 corners of the grid cell
    let p00 = get_warp_point(col, row);         // Top-left
    let p10 = get_warp_point(col + 1u, row);    // Top-right
    let p01 = get_warp_point(col, row + 1u);    // Bottom-left
    let p11 = get_warp_point(col + 1u, row + 1u); // Bottom-right

    // Bilinear interpolation of warped positions
    let top = mix(p00.position, p10.position, local.x);
    let bottom = mix(p01.position, p11.position, local.x);
    return mix(top, bottom, local.y);
}

// Apply color correction to a color
fn apply_color_correction(color: vec3<f32>, params: SliceParams) -> vec3<f32> {
    var c = color;

    // Extract adjustment values
    let brightness = params.color_adjust.x;  // -1.0 to 1.0
    let contrast = params.color_adjust.y;    // 0.0 to 2.0
    let gamma = params.color_adjust.z;       // 0.1 to 4.0

    // Apply brightness (add)
    c = c + vec3<f32>(brightness);

    // Apply contrast (around 0.5)
    c = (c - 0.5) * contrast + 0.5;

    // Apply gamma
    c = pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / gamma));

    // Apply RGB channel multipliers
    c = c * params.color_rgb.xyz;

    // Clamp to valid range
    return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
}

// Apply edge blending with gamma-corrected falloff
// Edge blend vec4 format: [enabled, width, gamma, black_level]
// uv is in 0-1 range (output rect normalized coordinates)
fn apply_edge_blend(color: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    var alpha = 1.0;

    // Left edge (uv.x goes from 0 at left to 1 at right)
    if (params.edge_left.x > 0.5 && uv.x < params.edge_left.y) {
        let t = uv.x / params.edge_left.y;
        let gamma = params.edge_left.z;
        let black = params.edge_left.w;
        alpha *= pow(t, gamma) * (1.0 - black) + black;
    }

    // Right edge (1.0 - uv.x gives distance from right)
    if (params.edge_right.x > 0.5 && uv.x > (1.0 - params.edge_right.y)) {
        let t = (1.0 - uv.x) / params.edge_right.y;
        let gamma = params.edge_right.z;
        let black = params.edge_right.w;
        alpha *= pow(t, gamma) * (1.0 - black) + black;
    }

    // Top edge (uv.y goes from 0 at top to 1 at bottom)
    if (params.edge_top.x > 0.5 && uv.y < params.edge_top.y) {
        let t = uv.y / params.edge_top.y;
        let gamma = params.edge_top.z;
        let black = params.edge_top.w;
        alpha *= pow(t, gamma) * (1.0 - black) + black;
    }

    // Bottom edge (1.0 - uv.y gives distance from bottom)
    if (params.edge_bottom.x > 0.5 && uv.y > (1.0 - params.edge_bottom.y)) {
        let t = (1.0 - uv.y) / params.edge_bottom.y;
        let gamma = params.edge_bottom.z;
        let black = params.edge_bottom.w;
        alpha *= pow(t, gamma) * (1.0 - black) + black;
    }

    return color * alpha;
}

// Apply mask from rasterized mask texture
// Samples mask texture and applies alpha based on mask shape
// uv is in 0-1 range (output rect normalized coordinates)
fn apply_mask(color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    if (params.mask_enabled < 0.5) {
        return color;
    }

    // Sample mask texture (alpha channel contains mask value)
    var mask_alpha = textureSample(t_mask, s_mask, uv).a;

    // Apply inversion if requested
    if (params.mask_inverted > 0.5) {
        mask_alpha = 1.0 - mask_alpha;
    }

    // Apply mask to color alpha
    return vec4<f32>(color.rgb, color.a * mask_alpha);
}

// Fragment shader - samples input with slice transformation
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var uv = in.uv;

    // Check if UV is within output rect bounds
    let out_x = params.output_rect.x;
    let out_y = params.output_rect.y;
    let out_w = params.output_rect.z;
    let out_h = params.output_rect.w;

    // If UV is outside output rect, return transparent
    if (uv.x < out_x || uv.x > out_x + out_w || uv.y < out_y || uv.y > out_y + out_h) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Map UV from output rect to 0-1 range
    uv = (uv - vec2<f32>(out_x, out_y)) / vec2<f32>(out_w, out_h);

    // Save output UV for edge blending (before warp transformations)
    let output_uv = uv;

    // Apply perspective warp (keystone correction)
    uv = apply_perspective_warp(uv);

    // Apply mesh warp (grid-based deformation)
    // Note: mesh warp overrides perspective when enabled
    uv = apply_mesh_warp(uv);

    // Apply rotation around center
    if (abs(params.rotation) > 0.0001) {
        let center = vec2<f32>(0.5, 0.5);
        uv = uv - center;

        // Correct for aspect ratio
        let aspect = out_w / max(out_h, 0.0001);
        uv.x = uv.x * aspect;

        let cos_r = cos(-params.rotation);
        let sin_r = sin(-params.rotation);
        uv = vec2<f32>(
            uv.x * cos_r - uv.y * sin_r,
            uv.x * sin_r + uv.y * cos_r
        );

        uv.x = uv.x / aspect;
        uv = uv + center;
    }

    // Apply flip
    if (params.flip.x > 0.5) {
        uv.x = 1.0 - uv.x;
    }
    if (params.flip.y > 0.5) {
        uv.y = 1.0 - uv.y;
    }

    // Map UV to input rect (crop from input)
    let in_x = params.input_rect.x;
    let in_y = params.input_rect.y;
    let in_w = params.input_rect.z;
    let in_h = params.input_rect.w;

    uv = vec2<f32>(in_x, in_y) + uv * vec2<f32>(in_w, in_h);

    // Bounds check for input
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Sample input texture
    let color = textureSample(t_input, s_input, uv);

    // Apply color correction
    let corrected = apply_color_correction(color.rgb, params);

    // Apply edge blending (using output UV, not warped/sampled UV)
    let blended = apply_edge_blend(corrected, output_uv);

    // Apply opacity
    let with_opacity = vec4<f32>(blended, color.a * params.opacity);

    // Apply mask (using output UV for mask sampling)
    return apply_mask(with_opacity, output_uv);
}

// Simple passthrough fragment shader (for direct copy)
@fragment
fn fs_simple(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_input, s_input, in.uv);
}
