// Layer compositing shader with blend modes and transforms
//
// Blend modes: 0=Normal, 1=Add, 2=Multiply, 3=Screen, 4=Overlay

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct LayerUniforms {
    // Transform matrix (row-major 3x3 packed as vec4 + vec4 + vec4)
    transform_row0: vec4<f32>,  // [m00, m01, m02, 0]
    transform_row1: vec4<f32>,  // [m10, m11, m12, 0]
    transform_row2: vec4<f32>,  // [m20, m21, m22, 0]
    // Layer properties: [opacity, blend_mode, 0, 0]
    properties: vec4<f32>,
}

@group(0) @binding(0)
var t_layer: texture_2d<f32>;
@group(0) @binding(1)
var s_sampler: sampler;

@group(1) @binding(0)
var<uniform> uniforms: LayerUniforms;

// Optional: destination texture for blend modes that need it
@group(2) @binding(0)
var t_destination: texture_2d<f32>;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    
    // Apply 2D transform
    let uv = input.uv;
    let transform = mat3x3<f32>(
        uniforms.transform_row0.xyz,
        uniforms.transform_row1.xyz,
        uniforms.transform_row2.xyz
    );
    
    // Transform UV coordinates
    let transformed_uv = (transform * vec3<f32>(uv - 0.5, 1.0)).xy + 0.5;
    
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.uv = transformed_uv;
    return output;
}

// Normal blend (alpha compositing)
fn blend_normal(src: vec4<f32>, dst: vec4<f32>) -> vec4<f32> {
    let out_a = src.a + dst.a * (1.0 - src.a);
    if out_a == 0.0 {
        return vec4<f32>(0.0);
    }
    let out_rgb = (src.rgb * src.a + dst.rgb * dst.a * (1.0 - src.a)) / out_a;
    return vec4<f32>(out_rgb, out_a);
}

// Add blend (additive)
fn blend_add(src: vec4<f32>, dst: vec4<f32>) -> vec4<f32> {
    let rgb = min(src.rgb * src.a + dst.rgb, vec3<f32>(1.0));
    let a = src.a + dst.a * (1.0 - src.a);
    return vec4<f32>(rgb, a);
}

// Multiply blend (darkens)
fn blend_multiply(src: vec4<f32>, dst: vec4<f32>) -> vec4<f32> {
    let rgb = src.rgb * dst.rgb;
    let a = src.a + dst.a * (1.0 - src.a);
    return vec4<f32>(mix(dst.rgb, rgb, src.a), a);
}

// Screen blend (lightens)
fn blend_screen(src: vec4<f32>, dst: vec4<f32>) -> vec4<f32> {
    let rgb = 1.0 - (1.0 - src.rgb) * (1.0 - dst.rgb);
    let a = src.a + dst.a * (1.0 - src.a);
    return vec4<f32>(mix(dst.rgb, rgb, src.a), a);
}

// Overlay blend (contrast)
fn blend_overlay(src: vec4<f32>, dst: vec4<f32>) -> vec4<f32> {
    var rgb: vec3<f32>;
    for (var i = 0u; i < 3u; i = i + 1u) {
        if dst[i] < 0.5 {
            rgb[i] = 2.0 * src[i] * dst[i];
        } else {
            rgb[i] = 1.0 - 2.0 * (1.0 - src[i]) * (1.0 - dst[i]);
        }
    }
    let a = src.a + dst.a * (1.0 - src.a);
    return vec4<f32>(mix(dst.rgb, rgb, src.a), a);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let opacity = uniforms.properties.x;
    let blend_mode = i32(uniforms.properties.y);
    
    // Check UV bounds
    if input.uv.x < 0.0 || input.uv.x > 1.0 || input.uv.y < 0.0 || input.uv.y > 1.0 {
        discard;
    }
    
    var src = textureSample(t_layer, s_sampler, input.uv);
    src.a *= opacity;
    
    // For simple output without destination texture, just return premultiplied color
    return vec4<f32>(src.rgb * src.a, src.a);
}

// Fragment shader for blend modes that need destination texture
@fragment
fn fs_blend(input: VertexOutput) -> @location(0) vec4<f32> {
    let opacity = uniforms.properties.x;
    let blend_mode = i32(uniforms.properties.y);
    
    // Check UV bounds
    if input.uv.x < 0.0 || input.uv.x > 1.0 || input.uv.y < 0.0 || input.uv.y > 1.0 {
        discard;
    }
    
    var src = textureSample(t_layer, s_sampler, input.uv);
    src.a *= opacity;
    
    // Get destination color at same position
    let dst = textureSample(t_destination, s_sampler, input.uv);
    
    var result: vec4<f32>;
    switch blend_mode {
        case 0: { // Normal
            result = blend_normal(src, dst);
        }
        case 1: { // Add
            result = blend_add(src, dst);
        }
        case 2: { // Multiply
            result = blend_multiply(src, dst);
        }
        case 3: { // Screen
            result = blend_screen(src, dst);
        }
        case 4: { // Overlay
            result = blend_overlay(src, dst);
        }
        default: {
            result = blend_normal(src, dst);
        }
    }
    
    return result;
}


