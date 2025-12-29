// Fullscreen Quad Shader for Video Display
//
// Renders a texture to fill the screen using a fullscreen triangle.
// Supports aspect ratio preservation via uniforms.

// Vertex output structure
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Uniforms for video display parameters
struct VideoParams {
    // Scale factor for aspect ratio (1.0 = fill, adjusted for letterbox/pillarbox)
    scale: vec2<f32>,
    // Offset for centering (0.0 = centered)
    offset: vec2<f32>,
    // Opacity (0.0 - 1.0)
    opacity: f32,
    // Padding for 16-byte alignment (3 floats)
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
}

@group(0) @binding(0) var t_video: texture_2d<f32>;
@group(0) @binding(1) var s_video: sampler;
@group(0) @binding(2) var<uniform> params: VideoParams;

// Vertex shader - generates fullscreen triangle from vertex index
// No vertex buffer needed - uses vertex_index to generate positions
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    
    // Generate fullscreen triangle coordinates
    // Vertex 0: (-1, -1), Vertex 1: (3, -1), Vertex 2: (-1, 3)
    // This covers the entire screen with a single triangle
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index >> 1u) * 4 - 1);
    
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    
    // UV coordinates (0,0 at top-left, 1,1 at bottom-right)
    // Flip Y for correct orientation
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    
    return out;
}

// Fragment shader - samples video texture with aspect ratio correction
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Apply scale and offset for aspect ratio preservation
    let adjusted_uv = (in.uv - 0.5) / params.scale + 0.5 + params.offset;
    
    // Check if UV is within texture bounds (for letterboxing)
    if (adjusted_uv.x < 0.0 || adjusted_uv.x > 1.0 || adjusted_uv.y < 0.0 || adjusted_uv.y > 1.0) {
        // Outside texture bounds - render black bars
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    
    // Sample the video texture
    let color = textureSample(t_video, s_video, adjusted_uv);
    
    // Apply opacity
    return vec4<f32>(color.rgb, color.a * params.opacity);
}

// Simple fragment shader without aspect ratio correction (for basic use)
@fragment
fn fs_simple(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_video, s_video, in.uv);
}

