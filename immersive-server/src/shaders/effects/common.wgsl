// Common utilities for effect shaders
//
// This file contains shared functions used across multiple effects.
// Include by copy-pasting into effect shaders (WGSL doesn't support #include).

// ============================================================================
// Color Space Conversions
// ============================================================================

// Convert RGB to HSV
// Input: RGB in [0, 1]
// Output: HSV where H in [0, 1], S in [0, 1], V in [0, 1]
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

// Convert HSV to RGB
// Input: HSV where H in [0, 1], S in [0, 1], V in [0, 1]
// Output: RGB in [0, 1]
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

// Convert RGB to HSL
// Input: RGB in [0, 1]
// Output: HSL where H in [0, 1], S in [0, 1], L in [0, 1]
fn rgb_to_hsl(rgb: vec3<f32>) -> vec3<f32> {
    let max_c = max(max(rgb.r, rgb.g), rgb.b);
    let min_c = min(min(rgb.r, rgb.g), rgb.b);
    let l = (max_c + min_c) * 0.5;

    var h: f32 = 0.0;
    var s: f32 = 0.0;

    if max_c != min_c {
        let d = max_c - min_c;
        s = select(d / (2.0 - max_c - min_c), d / (max_c + min_c), l > 0.5);

        if max_c == rgb.r {
            h = (rgb.g - rgb.b) / d + select(0.0, 6.0, rgb.g < rgb.b);
        } else if max_c == rgb.g {
            h = (rgb.b - rgb.r) / d + 2.0;
        } else {
            h = (rgb.r - rgb.g) / d + 4.0;
        }
        h /= 6.0;
    }

    return vec3<f32>(h, s, l);
}

// Helper for HSL to RGB conversion
fn hue_to_rgb(p: f32, q: f32, t: f32) -> f32 {
    var t_mod = t;
    if t_mod < 0.0 { t_mod += 1.0; }
    if t_mod > 1.0 { t_mod -= 1.0; }

    if t_mod < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t_mod;
    }
    if t_mod < 0.5 {
        return q;
    }
    if t_mod < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t_mod) * 6.0;
    }
    return p;
}

// Convert HSL to RGB
// Input: HSL where H in [0, 1], S in [0, 1], L in [0, 1]
// Output: RGB in [0, 1]
fn hsl_to_rgb(hsl: vec3<f32>) -> vec3<f32> {
    let h = hsl.x;
    let s = hsl.y;
    let l = hsl.z;

    if s == 0.0 {
        return vec3<f32>(l, l, l);
    }

    let q = select(l * (1.0 + s), l + s - l * s, l < 0.5);
    let p = 2.0 * l - q;

    return vec3<f32>(
        hue_to_rgb(p, q, h + 1.0 / 3.0),
        hue_to_rgb(p, q, h),
        hue_to_rgb(p, q, h - 1.0 / 3.0)
    );
}

// ============================================================================
// Common Effect Parameters Struct
// ============================================================================

// Standard effect parameters layout (matches EffectParams in Rust)
struct EffectParams {
    time: f32,
    delta_time: f32,
    beat_phase: f32,
    bar_phase: f32,
    // params[0..27] follow in individual effect structs
}

// ============================================================================
// Utility Functions
// ============================================================================

// Smoothstep for smooth transitions
fn smooth(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

// Mix two colors with an amount
fn mix_color(a: vec4<f32>, b: vec4<f32>, t: f32) -> vec4<f32> {
    return mix(a, b, t);
}

// Luminance (perceived brightness)
fn luminance(rgb: vec3<f32>) -> f32 {
    return dot(rgb, vec3<f32>(0.299, 0.587, 0.114));
}

// Clamp color to valid range
fn clamp_color(c: vec4<f32>) -> vec4<f32> {
    return clamp(c, vec4<f32>(0.0), vec4<f32>(1.0));
}
