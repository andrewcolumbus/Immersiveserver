//! Render module for GPU rendering pipeline
//!
//! Handles wgpu rendering, compositing, and shader management.

#![allow(dead_code)]

mod compositor;
mod pipeline;

pub use compositor::Compositor;

/// Vertex format for rendering
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
}

impl Vertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: 8,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x2,
            },
        ],
    };

    pub fn new(position: [f32; 2], uv: [f32; 2]) -> Self {
        Self { position, uv }
    }
}

/// Uniform data for blend shader
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BlendUniforms {
    /// Left blend: [width, power, gamma, black_level]
    pub left_blend: [f32; 4],
    /// Right blend: [width, power, gamma, black_level]
    pub right_blend: [f32; 4],
    /// Top blend: [width, power, gamma, black_level]
    pub top_blend: [f32; 4],
    /// Bottom blend: [width, power, gamma, black_level]
    pub bottom_blend: [f32; 4],
    /// Screen resolution: [width, height, 0, 0]
    pub resolution: [f32; 4],
}

impl Default for BlendUniforms {
    fn default() -> Self {
        Self {
            left_blend: [0.0, 2.2, 1.0, 0.0],
            right_blend: [0.0, 2.2, 1.0, 0.0],
            top_blend: [0.0, 2.2, 1.0, 0.0],
            bottom_blend: [0.0, 2.2, 1.0, 0.0],
            resolution: [1920.0, 1080.0, 0.0, 0.0],
        }
    }
}

/// Create a fullscreen quad for rendering
pub fn fullscreen_quad() -> [Vertex; 6] {
    [
        Vertex::new([-1.0, -1.0], [0.0, 1.0]),
        Vertex::new([1.0, -1.0], [1.0, 1.0]),
        Vertex::new([1.0, 1.0], [1.0, 0.0]),
        Vertex::new([-1.0, -1.0], [0.0, 1.0]),
        Vertex::new([1.0, 1.0], [1.0, 0.0]),
        Vertex::new([-1.0, 1.0], [0.0, 0.0]),
    ]
}


