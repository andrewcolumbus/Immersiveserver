// Test Pattern Shader for Immersive Server
// Professional calibration pattern with animated sweep line
//
// Features:
// - Checkerboard grid background
// - Rainbow color bar (left)
// - Grayscale gradient (right)
// - Center crosshairs and circle
// - Color calibration bars (bottom)
// - Resolution and time display
// - Animated diagonal sweep line (10-second cycle)

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct TestPatternParams {
    env_size: vec2<f32>,    // Environment dimensions in pixels
    time: f32,              // Elapsed time in seconds
    _pad: f32,
    logo_size: vec2<f32>,   // Logo texture dimensions in pixels
    _pad2: vec2<f32>,
}

@group(0) @binding(0) var<uniform> params: TestPatternParams;
@group(0) @binding(1) var logo_texture: texture_2d<f32>;
@group(0) @binding(2) var logo_sampler: sampler;

// ============================================================================
// VERTEX SHADER - Fullscreen triangle
// ============================================================================

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Generate fullscreen triangle vertices
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index >> 1u) * 4 - 1);
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// ============================================================================
// COLOR UTILITIES
// ============================================================================

// Convert HSV to RGB
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> vec3<f32> {
    let c = v * s;
    let x = c * (1.0 - abs(fract(h * 6.0) * 2.0 - 1.0));
    let m = v - c;

    var rgb: vec3<f32>;
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

// ============================================================================
// PATTERN DRAWING FUNCTIONS
// ============================================================================

// Draw checkerboard background
fn draw_checkerboard(pixel: vec2<f32>) -> vec3<f32> {
    let checker_size = 40.0; // Pixels per checker square
    let checker_x = floor(pixel.x / checker_size);
    let checker_y = floor(pixel.y / checker_size);
    let is_light = (i32(checker_x) + i32(checker_y)) % 2 == 0;

    // Subtle gray tones
    let light_gray = vec3<f32>(0.45, 0.45, 0.45);
    let dark_gray = vec3<f32>(0.35, 0.35, 0.35);

    return select(dark_gray, light_gray, is_light);
}

// Draw rainbow gradient bar on left side
fn draw_rainbow_bar(pixel: vec2<f32>, env_size: vec2<f32>) -> vec4<f32> {
    let bar_width = env_size.x * 0.10; // 10% of width
    let bar_start_x = env_size.x * 0.05; // 5% margin from left edge
    let bar_start_y = env_size.y * 0.10;
    let bar_end_y = env_size.y * 0.90;

    if (pixel.x > bar_start_x && pixel.x < bar_start_x + bar_width && pixel.y > bar_start_y && pixel.y < bar_end_y) {
        // Calculate hue based on vertical position
        let t = (pixel.y - bar_start_y) / (bar_end_y - bar_start_y);
        let hue = t; // 0 (red) to 1 (red again, full spectrum)
        let rgb = hsv_to_rgb(hue, 1.0, 1.0);

        // Add grid overlay to rainbow
        let grid_size = 30.0;
        let gx = floor(pixel.x / grid_size);
        let gy = floor(pixel.y / grid_size);
        let grid_line_x = abs(fract(pixel.x / grid_size) - 0.5) > 0.45;
        let grid_line_y = abs(fract(pixel.y / grid_size) - 0.5) > 0.45;
        let on_grid = grid_line_x || grid_line_y;

        if (on_grid) {
            return vec4<f32>(rgb * 0.7, 1.0);
        }
        return vec4<f32>(rgb, 1.0);
    }
    return vec4<f32>(0.0);
}

// Draw grayscale gradient bar on right side
fn draw_grayscale_bar(pixel: vec2<f32>, env_size: vec2<f32>) -> vec4<f32> {
    let bar_width = env_size.x * 0.10; // 10% of width
    let bar_start_x = env_size.x * 0.85; // 5% margin from right edge
    let bar_start_y = env_size.y * 0.10;
    let bar_end_y = env_size.y * 0.90;

    if (pixel.x > bar_start_x && pixel.y > bar_start_y && pixel.y < bar_end_y) {
        // Gradient from black (top) to white (bottom)
        let t = (pixel.y - bar_start_y) / (bar_end_y - bar_start_y);
        let gray = t;
        return vec4<f32>(vec3<f32>(gray), 1.0);
    }
    return vec4<f32>(0.0);
}

// Draw center crosshairs
fn draw_crosshairs(pixel: vec2<f32>, env_size: vec2<f32>) -> f32 {
    let center = env_size * 0.5;
    let line_width = 1.5;
    let dash_length = 15.0;
    let gap_length = 10.0;

    // Horizontal line
    let h_dist = abs(pixel.y - center.y);
    let h_dash = fract(pixel.x / (dash_length + gap_length)) < (dash_length / (dash_length + gap_length));
    let on_h_line = h_dist < line_width && h_dash;

    // Vertical line
    let v_dist = abs(pixel.x - center.x);
    let v_dash = fract(pixel.y / (dash_length + gap_length)) < (dash_length / (dash_length + gap_length));
    let on_v_line = v_dist < line_width && v_dash;

    return select(0.0, 1.0, on_h_line || on_v_line);
}

// Draw center circle for aspect ratio
fn draw_center_circle(pixel: vec2<f32>, env_size: vec2<f32>) -> f32 {
    let center = env_size * 0.5;
    let radius = min(env_size.x, env_size.y) * 0.35;
    let line_width = 2.0;

    let dist = length(pixel - center);
    let on_circle = abs(dist - radius) < line_width;

    // Also draw a smaller inner circle
    let inner_radius = radius * 0.5;
    let on_inner = abs(dist - inner_radius) < line_width;

    return select(0.0, 1.0, on_circle || on_inner);
}

// Draw diagonal lines through corners
fn draw_diagonal_lines(pixel: vec2<f32>, env_size: vec2<f32>) -> f32 {
    let line_width = 1.5;

    // Top-left to bottom-right diagonal
    let d1 = abs(pixel.x / env_size.x - pixel.y / env_size.y) * min(env_size.x, env_size.y);

    // Top-right to bottom-left diagonal
    let d2 = abs((env_size.x - pixel.x) / env_size.x - pixel.y / env_size.y) * min(env_size.x, env_size.y);

    return select(0.0, 1.0, d1 < line_width || d2 < line_width);
}

// Draw color calibration bars at bottom
fn draw_color_bars(pixel: vec2<f32>, env_size: vec2<f32>) -> vec4<f32> {
    let bar_height = env_size.y * 0.06;
    let bar_start_y = env_size.y * 0.88;
    let bar_start_x = env_size.x * 0.25;
    let bar_end_x = env_size.x * 0.75;

    if (pixel.y > bar_start_y && pixel.y < bar_start_y + bar_height &&
        pixel.x > bar_start_x && pixel.x < bar_end_x) {

        let t = (pixel.x - bar_start_x) / (bar_end_x - bar_start_x);
        let bar_index = u32(t * 6.0);

        // Standard color bars: Red, Yellow, Green, Cyan, Blue, Magenta
        var color: vec3<f32>;
        switch (bar_index) {
            case 0u: { color = vec3<f32>(1.0, 0.0, 0.0); } // Red
            case 1u: { color = vec3<f32>(1.0, 1.0, 0.0); } // Yellow
            case 2u: { color = vec3<f32>(0.0, 1.0, 0.0); } // Green
            case 3u: { color = vec3<f32>(0.0, 1.0, 1.0); } // Cyan
            case 4u: { color = vec3<f32>(0.0, 0.0, 1.0); } // Blue
            default: { color = vec3<f32>(1.0, 0.0, 1.0); } // Magenta
        }
        return vec4<f32>(color, 1.0);
    }
    return vec4<f32>(0.0);
}

// Draw frequency/detail test bars
fn draw_frequency_bars(pixel: vec2<f32>, env_size: vec2<f32>) -> vec4<f32> {
    let bar_height = env_size.y * 0.04;
    let bar_start_y = env_size.y * 0.82;
    let bar_start_x = env_size.x * 0.35;
    let bar_end_x = env_size.x * 0.65;

    if (pixel.y > bar_start_y && pixel.y < bar_start_y + bar_height &&
        pixel.x > bar_start_x && pixel.x < bar_end_x) {

        let t = (pixel.x - bar_start_x) / (bar_end_x - bar_start_x);
        // Increasing frequency stripes
        let freq = 2.0 + t * 30.0;
        let stripe = sin(pixel.x * freq) > 0.0;
        let gray = select(0.0, 1.0, stripe);
        return vec4<f32>(vec3<f32>(gray), 1.0);
    }
    return vec4<f32>(0.0);
}

// Draw animated diagonal sweep line
fn draw_sweep_line(pixel: vec2<f32>, env_size: vec2<f32>, time: f32) -> f32 {
    let cycle_time = 10.0; // 10-second cycle
    let phase = fract(time / cycle_time);

    // Diagonal line sweeping from top-left to bottom-right
    let total_distance = env_size.x + env_size.y;
    let offset = -env_size.y + phase * total_distance;

    // Distance from pixel to line x - y = offset
    let dist = abs(pixel.x - pixel.y - offset) / 1.414;

    // Line width with soft edges
    let line_width = 3.0;
    let alpha = 1.0 - smoothstep(0.0, line_width, dist);

    return alpha;
}

// ============================================================================
// 7-SEGMENT DIGIT RENDERING
// ============================================================================

// SDF for a horizontal segment
fn sdf_h_segment(p: vec2<f32>, cy: f32, w: f32, h: f32) -> f32 {
    let d = abs(p - vec2<f32>(0.5, cy)) - vec2<f32>(w * 0.5, h * 0.5);
    return max(d.x, d.y);
}

// SDF for a vertical segment
fn sdf_v_segment(p: vec2<f32>, cx: f32, cy: f32, w: f32, h: f32) -> f32 {
    let d = abs(p - vec2<f32>(cx, cy)) - vec2<f32>(w * 0.5, h * 0.5);
    return max(d.x, d.y);
}

// Draw a single 7-segment digit (0-9)
// uv is in 0-1 range within digit bounds
fn draw_digit(uv: vec2<f32>, digit: u32) -> f32 {
    // Segment dimensions (normalized)
    let seg_w = 0.6;  // width of horizontal segments
    let seg_h = 0.12; // height of segments
    let seg_vw = 0.15; // width of vertical segments
    let seg_vh = 0.35; // height of vertical segments

    // Segment positions
    // Layout:
    //   0 (top)
    // 1   2
    //   3 (middle)
    // 4   5
    //   6 (bottom)

    // Which segments are on for each digit (encoded as bits)
    // Segments: 0=top, 1=top-left, 2=top-right, 3=middle, 4=bottom-left, 5=bottom-right, 6=bottom
    let patterns = array<u32, 10>(
        0x77u, // 0: all except middle
        0x24u, // 1: right side only
        0x5Du, // 2: top, top-right, middle, bottom-left, bottom
        0x6Du, // 3: top, top-right, middle, bottom-right, bottom
        0x2Eu, // 4: top-left, top-right, middle, bottom-right
        0x6Bu, // 5: top, top-left, middle, bottom-right, bottom
        0x7Bu, // 6: all except top-right
        0x25u, // 7: top, top-right, bottom-right
        0x7Fu, // 8: all segments
        0x6Fu  // 9: all except bottom-left
    );

    let pattern = patterns[digit % 10u];
    var d: f32 = 999.0;

    // Top segment (0)
    if ((pattern & 0x01u) != 0u) {
        d = min(d, sdf_h_segment(uv, 0.9, seg_w, seg_h));
    }
    // Top-left segment (1)
    if ((pattern & 0x02u) != 0u) {
        d = min(d, sdf_v_segment(uv, 0.15, 0.7, seg_vw, seg_vh));
    }
    // Top-right segment (2)
    if ((pattern & 0x04u) != 0u) {
        d = min(d, sdf_v_segment(uv, 0.85, 0.7, seg_vw, seg_vh));
    }
    // Middle segment (3)
    if ((pattern & 0x08u) != 0u) {
        d = min(d, sdf_h_segment(uv, 0.5, seg_w, seg_h));
    }
    // Bottom-left segment (4)
    if ((pattern & 0x10u) != 0u) {
        d = min(d, sdf_v_segment(uv, 0.15, 0.3, seg_vw, seg_vh));
    }
    // Bottom-right segment (5)
    if ((pattern & 0x20u) != 0u) {
        d = min(d, sdf_v_segment(uv, 0.85, 0.3, seg_vw, seg_vh));
    }
    // Bottom segment (6)
    if ((pattern & 0x40u) != 0u) {
        d = min(d, sdf_h_segment(uv, 0.1, seg_w, seg_h));
    }

    return 1.0 - smoothstep(-0.02, 0.02, d);
}

// Draw resolution text (e.g., "1920 x 1080")
fn draw_resolution_text(pixel: vec2<f32>, env_size: vec2<f32>) -> f32 {
    let digit_width = 30.0;
    let digit_height = 50.0;
    let spacing = 35.0;

    // Center position for resolution text
    let center_x = env_size.x * 0.5;
    let center_y = env_size.y * 0.5 + 30.0;

    // Calculate digits for width and height
    let w = u32(env_size.x);
    let h = u32(env_size.y);

    // Width digits (up to 4 digits)
    let w1 = (w / 1000u) % 10u;
    let w2 = (w / 100u) % 10u;
    let w3 = (w / 10u) % 10u;
    let w4 = w % 10u;

    // Height digits (up to 4 digits)
    let h1 = (h / 1000u) % 10u;
    let h2 = (h / 100u) % 10u;
    let h3 = (h / 10u) % 10u;
    let h4 = h % 10u;

    // Total width: 4 digits + space + "x" + space + 4 digits
    let total_width = spacing * 9.0;
    let start_x = center_x - total_width * 0.5;

    var alpha = 0.0;

    // Draw width digits
    for (var i = 0u; i < 4u; i = i + 1u) {
        let digit_x = start_x + f32(i) * spacing;
        if (pixel.x >= digit_x && pixel.x < digit_x + digit_width &&
            pixel.y >= center_y && pixel.y < center_y + digit_height) {
            let local_uv = vec2<f32>(
                (pixel.x - digit_x) / digit_width,
                1.0 - (pixel.y - center_y) / digit_height
            );
            var d: u32;
            switch (i) {
                case 0u: { d = w1; }
                case 1u: { d = w2; }
                case 2u: { d = w3; }
                default: { d = w4; }
            }
            // Skip leading zeros for width
            if (i == 0u && d == 0u && w < 1000u) { continue; }
            if (i == 1u && d == 0u && w < 100u) { continue; }
            if (i == 2u && d == 0u && w < 10u) { continue; }
            alpha = max(alpha, draw_digit(local_uv, d));
        }
    }

    // Draw "x" separator (simple cross)
    let sep_x = start_x + 4.0 * spacing + spacing * 0.3;
    let sep_y = center_y + digit_height * 0.5;
    let sep_dist = length(pixel - vec2<f32>(sep_x, sep_y));
    if (sep_dist < 8.0) {
        let local = pixel - vec2<f32>(sep_x, sep_y);
        let on_x = abs(abs(local.x) - abs(local.y)) < 3.0;
        if (on_x) {
            alpha = max(alpha, 1.0);
        }
    }

    // Draw height digits
    for (var i = 0u; i < 4u; i = i + 1u) {
        let digit_x = start_x + (5.0 + f32(i)) * spacing;
        if (pixel.x >= digit_x && pixel.x < digit_x + digit_width &&
            pixel.y >= center_y && pixel.y < center_y + digit_height) {
            let local_uv = vec2<f32>(
                (pixel.x - digit_x) / digit_width,
                1.0 - (pixel.y - center_y) / digit_height
            );
            var d: u32;
            switch (i) {
                case 0u: { d = h1; }
                case 1u: { d = h2; }
                case 2u: { d = h3; }
                default: { d = h4; }
            }
            // Skip leading zeros for height
            if (i == 0u && d == 0u && h < 1000u) { continue; }
            if (i == 1u && d == 0u && h < 100u) { continue; }
            if (i == 2u && d == 0u && h < 10u) { continue; }
            alpha = max(alpha, draw_digit(local_uv, d));
        }
    }

    return alpha;
}

// Draw time display (HH:MM:SS)
fn draw_time_text(pixel: vec2<f32>, env_size: vec2<f32>, time: f32) -> f32 {
    let digit_width = 20.0;
    let digit_height = 35.0;
    let spacing = 24.0;
    let colon_spacing = 12.0;

    // Position below resolution
    let center_x = env_size.x * 0.5;
    let center_y = env_size.y * 0.5 + 100.0;

    // Calculate time components
    let total_secs = u32(time);
    let hours = (total_secs / 3600u) % 24u;
    let minutes = (total_secs / 60u) % 60u;
    let seconds = total_secs % 60u;

    // Total width: HH:MM:SS = 6 digits + 2 colons
    let total_width = spacing * 6.0 + colon_spacing * 2.0;
    let start_x = center_x - total_width * 0.5;

    var alpha = 0.0;

    // Draw hours (2 digits)
    for (var i = 0u; i < 2u; i = i + 1u) {
        let digit_x = start_x + f32(i) * spacing;
        if (pixel.x >= digit_x && pixel.x < digit_x + digit_width &&
            pixel.y >= center_y && pixel.y < center_y + digit_height) {
            let local_uv = vec2<f32>(
                (pixel.x - digit_x) / digit_width,
                1.0 - (pixel.y - center_y) / digit_height
            );
            let d = select(hours % 10u, hours / 10u, i == 0u);
            alpha = max(alpha, draw_digit(local_uv, d));
        }
    }

    // Draw first colon
    let colon1_x = start_x + 2.0 * spacing + colon_spacing * 0.5;
    if (abs(pixel.x - colon1_x) < 3.0) {
        let dy1 = abs(pixel.y - (center_y + digit_height * 0.3));
        let dy2 = abs(pixel.y - (center_y + digit_height * 0.7));
        if (dy1 < 4.0 || dy2 < 4.0) {
            alpha = max(alpha, 1.0);
        }
    }

    // Draw minutes (2 digits)
    let mins_start = start_x + 2.0 * spacing + colon_spacing;
    for (var i = 0u; i < 2u; i = i + 1u) {
        let digit_x = mins_start + f32(i) * spacing;
        if (pixel.x >= digit_x && pixel.x < digit_x + digit_width &&
            pixel.y >= center_y && pixel.y < center_y + digit_height) {
            let local_uv = vec2<f32>(
                (pixel.x - digit_x) / digit_width,
                1.0 - (pixel.y - center_y) / digit_height
            );
            let d = select(minutes % 10u, minutes / 10u, i == 0u);
            alpha = max(alpha, draw_digit(local_uv, d));
        }
    }

    // Draw second colon
    let colon2_x = mins_start + 2.0 * spacing + colon_spacing * 0.5;
    if (abs(pixel.x - colon2_x) < 3.0) {
        let dy1 = abs(pixel.y - (center_y + digit_height * 0.3));
        let dy2 = abs(pixel.y - (center_y + digit_height * 0.7));
        if (dy1 < 4.0 || dy2 < 4.0) {
            alpha = max(alpha, 1.0);
        }
    }

    // Draw seconds (2 digits)
    let secs_start = mins_start + 2.0 * spacing + colon_spacing;
    for (var i = 0u; i < 2u; i = i + 1u) {
        let digit_x = secs_start + f32(i) * spacing;
        if (pixel.x >= digit_x && pixel.x < digit_x + digit_width &&
            pixel.y >= center_y && pixel.y < center_y + digit_height) {
            let local_uv = vec2<f32>(
                (pixel.x - digit_x) / digit_width,
                1.0 - (pixel.y - center_y) / digit_height
            );
            let d = select(seconds % 10u, seconds / 10u, i == 0u);
            alpha = max(alpha, draw_digit(local_uv, d));
        }
    }

    return alpha;
}

// Draw logo from texture (centered above middle)
fn draw_branding(pixel: vec2<f32>, env_size: vec2<f32>) -> vec4<f32> {
    // Position above center - scale logo to fit nicely
    let center_x = env_size.x * 0.5;
    let center_y = env_size.y * 0.5 - 80.0;

    // Target logo width (30% of environment width, capped at 400px)
    let target_width = min(env_size.x * 0.30, 400.0);
    let aspect_ratio = params.logo_size.x / params.logo_size.y;
    let logo_width = target_width;
    let logo_height = target_width / aspect_ratio;

    // Logo bounds
    let start_x = center_x - logo_width * 0.5;
    let start_y = center_y - logo_height * 0.5;

    // Check if we're in the logo region
    if (pixel.x >= start_x && pixel.x < start_x + logo_width &&
        pixel.y >= start_y && pixel.y < start_y + logo_height) {
        // Calculate UV coordinates for texture sampling
        let uv = vec2<f32>(
            (pixel.x - start_x) / logo_width,
            (pixel.y - start_y) / logo_height
        );
        // Sample the logo texture
        return textureSample(logo_texture, logo_sampler, uv);
    }
    return vec4<f32>(0.0);
}

// ============================================================================
// FRAGMENT SHADER - Composite all elements
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let pixel = in.uv * params.env_size;

    // Start with checkerboard background
    var color = draw_checkerboard(pixel);

    // Draw static diagonal lines (corner to corner)
    let diag = draw_diagonal_lines(pixel, params.env_size);
    color = mix(color, vec3<f32>(1.0), diag * 0.6);

    // Layer rainbow bar (left side)
    let rainbow = draw_rainbow_bar(pixel, params.env_size);
    if (rainbow.a > 0.0) {
        color = rainbow.rgb;
    }

    // Layer grayscale bar (right side)
    let grayscale = draw_grayscale_bar(pixel, params.env_size);
    if (grayscale.a > 0.0) {
        color = grayscale.rgb;
    }

    // Layer frequency test bars
    let freq_bars = draw_frequency_bars(pixel, params.env_size);
    if (freq_bars.a > 0.0) {
        color = freq_bars.rgb;
    }

    // Layer color bars (bottom)
    let color_bars = draw_color_bars(pixel, params.env_size);
    if (color_bars.a > 0.0) {
        color = color_bars.rgb;
    }

    // Draw center crosshairs
    let crosshairs = draw_crosshairs(pixel, params.env_size);
    color = mix(color, vec3<f32>(1.0), crosshairs * 0.8);

    // Draw center circle
    let circle = draw_center_circle(pixel, params.env_size);
    color = mix(color, vec3<f32>(1.0), circle * 0.8);

    // Draw logo branding (alpha-blended)
    let branding = draw_branding(pixel, params.env_size);
    color = mix(color, branding.rgb, branding.a);

    // Draw resolution text
    let resolution = draw_resolution_text(pixel, params.env_size);
    color = mix(color, vec3<f32>(1.0), resolution);

    // Draw time text
    let time_text = draw_time_text(pixel, params.env_size, params.time);
    color = mix(color, vec3<f32>(1.0), time_text);

    // Draw animated sweep line (on top of everything)
    let sweep = draw_sweep_line(pixel, params.env_size, params.time);
    color = mix(color, vec3<f32>(1.0), sweep);

    return vec4<f32>(color, 1.0);
}
