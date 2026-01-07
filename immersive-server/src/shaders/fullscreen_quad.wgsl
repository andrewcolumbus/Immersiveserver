// Fullscreen Quad Shader for Layer Display
//
// Renders a texture with full 2D transform support (position, scale, rotation).
// Used for compositing layers in the environment.

// Vertex output structure
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Uniforms for layer display parameters
struct LayerParams {
    // Video/layer size relative to environment (video_size / env_size)
    size_scale: vec2<f32>,
    // Position offset from center in normalized coordinates
    // (0,0) = centered, positive = right/down, negative = left/up
    position: vec2<f32>,
    // Scale factors for the transform
    scale: vec2<f32>,
    // Rotation in radians (clockwise)
    rotation: f32,
    // Environment aspect ratio (width / height) for correct rotation
    env_aspect: f32,
    // Anchor point for rotation/scaling (0-1, where 0.5,0.5 = center)
    anchor: vec2<f32>,
    // Opacity (0.0 - 1.0)
    opacity: f32,
    // Whether texture is BGRA (1.0) or RGBA (0.0) - swaps R and B channels
    is_bgra: f32,
}

@group(0) @binding(0) var t_video: texture_2d<f32>;
@group(0) @binding(1) var s_video: sampler;
@group(0) @binding(2) var<uniform> params: LayerParams;

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

// Fragment shader - samples video texture with full 2D transform
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Convert UV to center-relative coordinates (-0.5 to 0.5)
    // This makes (0,0) the center of the environment
    var uv = in.uv - 0.5;

    // INVERSE TRANSFORM: Transform UV coordinates to sample the texture correctly
    // The layer transform moves the layer visually, so we apply the inverse to UVs.

    // Step 1: Undo position translation (position is center-relative)
    uv = uv - params.position;

    // Convert anchor from 0-1 space to center-relative
    let anchor_offset = params.anchor - 0.5;

    // Step 2: Move to anchor-relative coordinates
    uv = uv - anchor_offset;

    // Step 3: Undo rotation (rotate by negative angle)
    // Use environment aspect ratio for proper rotation without distortion
    let safe_aspect = max(params.env_aspect, 0.0001);

    // Convert to aspect-corrected coordinates (square space)
    var rotated_uv = uv;
    rotated_uv.x = rotated_uv.x * safe_aspect;

    let cos_r = cos(-params.rotation);
    let sin_r = sin(-params.rotation);
    rotated_uv = vec2<f32>(
        rotated_uv.x * cos_r - rotated_uv.y * sin_r,
        rotated_uv.x * sin_r + rotated_uv.y * cos_r
    );

    // Convert back from square space
    rotated_uv.x = rotated_uv.x / safe_aspect;
    uv = rotated_uv;

    // Step 4: Undo scale (divide by scale factors)
    let safe_scale = max(params.scale, vec2<f32>(0.0001, 0.0001));
    uv = uv / safe_scale;

    // Step 5: Restore anchor offset
    uv = uv + anchor_offset;

    // Step 6: Apply size_scale for video centering and convert back to 0-1 space
    // Video is centered within the layer (which is environment-sized)
    uv = uv / params.size_scale + 0.5;

    // Bounds check - if UV is outside [0,1], return transparent
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Sample the video texture
    var color = textureSample(t_video, s_video, uv);

    // Swap R and B channels for BGRA textures (NDI provides BGRA)
    if (params.is_bgra > 0.5) {
        color = vec4<f32>(color.b, color.g, color.r, color.a);
    }

    // Apply opacity
    return vec4<f32>(color.rgb, color.a * params.opacity);
}

// Simple fragment shader without transforms (for basic use)
@fragment
fn fs_simple(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_video, s_video, in.uv);
}
