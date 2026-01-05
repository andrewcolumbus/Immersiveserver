// Slice Render Shader
//
// Renders a slice by sampling from the composition (or layer) texture
// with input rect cropping, output positioning, rotation, flipping, and color correction.

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Slice parameters uniform
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
}

@group(0) @binding(0) var t_input: texture_2d<f32>;
@group(0) @binding(1) var s_input: sampler;
@group(0) @binding(2) var<uniform> params: SliceParams;

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

    // Apply opacity
    return vec4<f32>(corrected, color.a * params.opacity);
}

// Simple passthrough fragment shader (for direct copy)
@fragment
fn fs_simple(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_input, s_input, in.uv);
}
