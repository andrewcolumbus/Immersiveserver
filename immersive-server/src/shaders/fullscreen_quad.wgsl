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
    // Position in normalized coordinates (0-1)
    position: vec2<f32>,
    // Scale factors for the transform
    scale: vec2<f32>,
    // Rotation in radians (clockwise)
    rotation: f32,
    // Anchor point for rotation/scaling (0-1, where 0.5,0.5 = center)
    anchor: vec2<f32>,
    // Opacity (0.0 - 1.0)
    opacity: f32,
    // Padding for 16-byte alignment
    _pad: vec2<f32>,
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
    // We need to apply the INVERSE transform to the UV coordinates.
    // The layer transform moves the layer in screen space, so we need
    // to do the opposite to the sampling coordinates.
    //
    // Transform order (visual, what the layer does):
    // 1. Start at origin
    // 2. Scale around anchor
    // 3. Rotate around anchor  
    // 4. Translate to position
    //
    // Inverse transform order (what we do to UVs):
    // 1. Subtract position (undo translation)
    // 2. Subtract anchor (move to anchor-relative coords)
    // 3. Rotate by -angle (undo rotation)
    // 4. Divide by scale (undo scale)
    // 5. Add anchor (restore anchor offset)
    // 6. Apply size_scale for video-to-environment mapping
    
    var uv = in.uv;
    
    // Step 1: Undo position translation
    uv = uv - params.position;
    
    // Calculate the layer's center in UV space (accounting for size)
    // The anchor point is relative to the layer's own size
    let layer_center = params.anchor * params.size_scale;
    
    // Step 2: Move to anchor-relative coordinates
    uv = uv - layer_center;
    
    // Step 3: Undo rotation (rotate by negative angle)
    let cos_r = cos(-params.rotation);
    let sin_r = sin(-params.rotation);
    let rotated_uv = vec2<f32>(
        uv.x * cos_r - uv.y * sin_r,
        uv.x * sin_r + uv.y * cos_r
    );
    uv = rotated_uv;
    
    // Step 4: Undo scale (divide by scale factors)
    // Protect against division by zero
    let safe_scale = max(params.scale, vec2<f32>(0.0001, 0.0001));
    uv = uv / safe_scale;
    
    // Step 5: Restore anchor offset
    uv = uv + layer_center;
    
    // Step 6: Apply size_scale for video-to-environment mapping
    // Convert from environment UV space to video UV space
    uv = (uv - 0.5) / params.size_scale + 0.5;
    
    // Bounds check - if UV is outside [0,1], return transparent
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    
    // Sample the video texture
    let color = textureSample(t_video, s_video, uv);
    
    // Apply opacity
    return vec4<f32>(color.rgb, color.a * params.opacity);
}

// Simple fragment shader without transforms (for basic use)
@fragment
fn fs_simple(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_video, s_video, in.uv);
}
