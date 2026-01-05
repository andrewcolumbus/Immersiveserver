// Screen Composite Shader
//
// Composites a slice texture to the screen output with optional color correction.
// Used for the final output stage before sending to displays/streams.

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Screen-level color correction parameters
struct ScreenParams {
    // Color correction: brightness, contrast, gamma, saturation
    color_adjust: vec4<f32>,
    // RGB channel multipliers + padding
    color_rgb: vec4<f32>,
}

@group(0) @binding(0) var t_slice: texture_2d<f32>;
@group(0) @binding(1) var s_slice: sampler;
@group(0) @binding(2) var<uniform> params: ScreenParams;

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

// Convert RGB to HSL
fn rgb_to_hsl(rgb: vec3<f32>) -> vec3<f32> {
    let max_c = max(max(rgb.r, rgb.g), rgb.b);
    let min_c = min(min(rgb.r, rgb.g), rgb.b);
    let delta = max_c - min_c;

    let l = (max_c + min_c) * 0.5;

    var h = 0.0;
    var s = 0.0;

    if (delta > 0.0001) {
        s = delta / (1.0 - abs(2.0 * l - 1.0));

        if (max_c == rgb.r) {
            h = ((rgb.g - rgb.b) / delta) % 6.0;
        } else if (max_c == rgb.g) {
            h = (rgb.b - rgb.r) / delta + 2.0;
        } else {
            h = (rgb.r - rgb.g) / delta + 4.0;
        }
        h = h / 6.0;
        if (h < 0.0) {
            h = h + 1.0;
        }
    }

    return vec3<f32>(h, s, l);
}

// Convert HSL to RGB
fn hsl_to_rgb(hsl: vec3<f32>) -> vec3<f32> {
    let h = hsl.x;
    let s = hsl.y;
    let l = hsl.z;

    let c = (1.0 - abs(2.0 * l - 1.0)) * s;
    let x = c * (1.0 - abs((h * 6.0) % 2.0 - 1.0));
    let m = l - c * 0.5;

    var rgb = vec3<f32>(0.0);

    let h6 = h * 6.0;
    if (h6 < 1.0) {
        rgb = vec3<f32>(c, x, 0.0);
    } else if (h6 < 2.0) {
        rgb = vec3<f32>(x, c, 0.0);
    } else if (h6 < 3.0) {
        rgb = vec3<f32>(0.0, c, x);
    } else if (h6 < 4.0) {
        rgb = vec3<f32>(0.0, x, c);
    } else if (h6 < 5.0) {
        rgb = vec3<f32>(x, 0.0, c);
    } else {
        rgb = vec3<f32>(c, 0.0, x);
    }

    return rgb + vec3<f32>(m);
}

// Apply screen-level color correction
fn apply_screen_color_correction(color: vec3<f32>, params: ScreenParams) -> vec3<f32> {
    var c = color;

    // Extract adjustment values
    let brightness = params.color_adjust.x;  // -1.0 to 1.0
    let contrast = params.color_adjust.y;    // 0.0 to 2.0
    let gamma = params.color_adjust.z;       // 0.1 to 4.0
    let saturation = params.color_adjust.w;  // 0.0 to 2.0

    // Apply brightness
    c = c + vec3<f32>(brightness);

    // Apply contrast
    c = (c - 0.5) * contrast + 0.5;

    // Apply gamma
    c = pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / gamma));

    // Apply saturation via HSL
    if (abs(saturation - 1.0) > 0.001) {
        let hsl = rgb_to_hsl(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)));
        let adjusted_hsl = vec3<f32>(hsl.x, hsl.y * saturation, hsl.z);
        c = hsl_to_rgb(adjusted_hsl);
    }

    // Apply RGB channel multipliers
    c = c * params.color_rgb.xyz;

    return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
}

// Fragment shader with color correction
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_slice, s_slice, in.uv);

    // Check if color correction is needed (all values at identity)
    let is_identity =
        abs(params.color_adjust.x) < 0.001 &&      // brightness == 0
        abs(params.color_adjust.y - 1.0) < 0.001 && // contrast == 1
        abs(params.color_adjust.z - 1.0) < 0.001 && // gamma == 1
        abs(params.color_adjust.w - 1.0) < 0.001 && // saturation == 1
        abs(params.color_rgb.x - 1.0) < 0.001 &&   // red == 1
        abs(params.color_rgb.y - 1.0) < 0.001 &&   // green == 1
        abs(params.color_rgb.z - 1.0) < 0.001;     // blue == 1

    if (is_identity) {
        return color;
    }

    let corrected = apply_screen_color_correction(color.rgb, params);
    return vec4<f32>(corrected, color.a);
}

// Simple passthrough fragment shader (no color correction)
@fragment
fn fs_simple(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_slice, s_slice, in.uv);
}

// Fragment shader for clearing to black
@fragment
fn fs_clear(_in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
