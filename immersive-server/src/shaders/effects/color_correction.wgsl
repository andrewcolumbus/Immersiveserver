// Color Correction Effect Shader
//
// Provides brightness, contrast, saturation, hue shift, and gamma controls.
//
// Parameters:
//   params[0] = brightness (-1 to 1, default 0)
//   params[1] = contrast (0 to 2, default 1)
//   params[2] = saturation (0 to 2, default 1)
//   params[3] = hue_shift (0 to 1, default 0)
//   params[4] = gamma (0.1 to 3, default 1)

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct EffectParams {
    time: f32,
    delta_time: f32,
    beat_phase: f32,
    bar_phase: f32,
    brightness: f32,    // params[0]
    contrast: f32,      // params[1]
    saturation: f32,    // params[2]
    hue_shift: f32,     // params[3]
    gamma: f32,         // params[4]
    _pad1: f32,         // Padding (scalars avoid vec3 alignment issues)
    _pad2: f32,
    _pad3: f32,
}

@group(0) @binding(0) var t_input: texture_2d<f32>;
@group(0) @binding(1) var s_input: sampler;
@group(0) @binding(2) var<uniform> params: EffectParams;

// ============================================================================
// Color Space Conversions (from common.wgsl)
// ============================================================================

fn rgb_to_hsv(rgb: vec3<f32>) -> vec3<f32> {
    let r = rgb.r;
    let g = rgb.g;
    let b = rgb.b;

    let max_c = max(max(r, g), b);
    let min_c = min(min(r, g), b);
    let delta = max_c - min_c;

    var h: f32 = 0.0;
    var s: f32 = 0.0;
    let v = max_c;

    if delta > 0.00001 {
        s = delta / max_c;

        if max_c == r {
            h = (g - b) / delta;
            if g < b {
                h += 6.0;
            }
        } else if max_c == g {
            h = (b - r) / delta + 2.0;
        } else {
            h = (r - g) / delta + 4.0;
        }
        h /= 6.0;
    }

    return vec3<f32>(h, s, v);
}

fn hsv_to_rgb(hsv: vec3<f32>) -> vec3<f32> {
    let h = hsv.x * 6.0;
    let s = hsv.y;
    let v = hsv.z;

    let i = floor(h);
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    let i_mod = i32(i) % 6;

    if i_mod == 0 {
        return vec3<f32>(v, t, p);
    } else if i_mod == 1 {
        return vec3<f32>(q, v, p);
    } else if i_mod == 2 {
        return vec3<f32>(p, v, t);
    } else if i_mod == 3 {
        return vec3<f32>(p, q, v);
    } else if i_mod == 4 {
        return vec3<f32>(t, p, v);
    } else {
        return vec3<f32>(v, p, q);
    }
}

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
    let color = textureSample(t_input, s_input, in.uv);
    var rgb = color.rgb;

    // 1. Brightness adjustment (additive)
    rgb = rgb + params.brightness;

    // 2. Contrast adjustment (around 0.5 midpoint)
    rgb = (rgb - 0.5) * params.contrast + 0.5;

    // 3. Saturation and Hue adjustment in HSV space
    var hsv = rgb_to_hsv(max(rgb, vec3<f32>(0.0)));

    // Hue shift (wrap around 0-1)
    hsv.x = fract(hsv.x + params.hue_shift);

    // Saturation multiplier
    hsv.y = hsv.y * params.saturation;

    rgb = hsv_to_rgb(hsv);

    // 4. Gamma correction
    let inv_gamma = 1.0 / max(params.gamma, 0.01);
    rgb = pow(max(rgb, vec3<f32>(0.0)), vec3<f32>(inv_gamma));

    // Clamp to valid range
    rgb = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(rgb, color.a);
}
