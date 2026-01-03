// Gray code pattern generation shader for structured light calibration.
//
// Generates binary stripe patterns for Gray code encoding.
// Each pattern encodes one bit of the projector coordinate.

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
};

struct PatternParams {
    // Pattern parameters
    bit_index: u32,        // Which bit we're encoding (0 = MSB)
    total_bits: u32,       // Total bits for this direction
    direction: u32,        // 0 = horizontal (stripes along X, encode Y), 1 = vertical
    inverted: u32,         // 0 = normal, 1 = inverted
    // Projector dimensions
    proj_width: f32,
    proj_height: f32,
    // Pattern type: 0 = gray code, 1 = white, 2 = black
    pattern_type: u32,
    _padding: u32,
};

@group(0) @binding(0)
var<uniform> params: PatternParams;

// Full-screen triangle vertex shader
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;

    // Generate full-screen triangle
    let x = f32(i32(vertex_index) - 1);
    let y = f32(i32(vertex_index & 1u) * 2 - 1);

    output.position = vec4<f32>(x, y, 0.0, 1.0);
    output.tex_coord = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);

    return output;
}

// Convert binary coordinate to Gray code
fn binary_to_gray(binary: u32) -> u32 {
    return binary ^ (binary >> 1u);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Reference patterns
    if (params.pattern_type == 1u) {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0); // White
    }
    if (params.pattern_type == 2u) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0); // Black
    }

    // Calculate pixel coordinate in projector space
    let px = u32(input.tex_coord.x * params.proj_width);
    let py = u32(input.tex_coord.y * params.proj_height);

    // Get the coordinate we're encoding (X for vertical stripes, Y for horizontal)
    var coord: u32;
    if (params.direction == 0u) {
        coord = py; // Horizontal stripes encode Y coordinate
    } else {
        coord = px; // Vertical stripes encode X coordinate
    }

    // Convert to Gray code
    let gray = binary_to_gray(coord);

    // Extract the bit we're interested in (MSB first)
    let bit_position = params.total_bits - 1u - params.bit_index;
    let bit_value = (gray >> bit_position) & 1u;

    // Apply inversion if needed
    var value: u32;
    if (params.inverted == 1u) {
        value = 1u - bit_value;
    } else {
        value = bit_value;
    }

    // Output white or black
    let intensity = f32(value);
    return vec4<f32>(intensity, intensity, intensity, 1.0);
}
