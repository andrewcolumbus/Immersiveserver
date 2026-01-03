//! Gray code pattern renderer using GPU shaders.

use crate::calibration::{PatternDirection, PatternSpec};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// Pattern type for shader.
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum PatternType {
    GrayCode = 0,
    White = 1,
    Black = 2,
}

/// Uniform buffer for pattern parameters.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct PatternParams {
    bit_index: u32,
    total_bits: u32,
    direction: u32,
    inverted: u32,
    proj_width: f32,
    proj_height: f32,
    pattern_type: u32,
    _padding: u32,
}

/// GPU-accelerated Gray code pattern renderer.
pub struct PatternRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    params_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    proj_width: u32,
    proj_height: u32,
    horizontal_bits: u32,
    vertical_bits: u32,
}

impl PatternRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, width: u32, height: u32) -> Self {
        let shader_source = include_str!("shaders/gray_code.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Gray Code Pattern Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Pattern Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pattern Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Pattern Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let horizontal_bits = (width as f32).log2().ceil() as u32;
        let vertical_bits = (height as f32).log2().ceil() as u32;

        let initial_params = PatternParams {
            bit_index: 0,
            total_bits: horizontal_bits,
            direction: 1, // Vertical
            inverted: 0,
            proj_width: width as f32,
            proj_height: height as f32,
            pattern_type: PatternType::GrayCode as u32,
            _padding: 0,
        };

        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Pattern Params Buffer"),
            contents: bytemuck::cast_slice(&[initial_params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Pattern Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: params_buffer.as_entire_binding(),
            }],
        });

        Self {
            pipeline,
            bind_group_layout,
            params_buffer,
            bind_group,
            proj_width: width,
            proj_height: height,
            horizontal_bits,
            vertical_bits,
        }
    }

    /// Update projector dimensions.
    pub fn set_dimensions(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.proj_width = width;
        self.proj_height = height;
        self.horizontal_bits = (width as f32).log2().ceil() as u32;
        self.vertical_bits = (height as f32).log2().ceil() as u32;

        // Recreate bind group with new buffer (dimensions changed)
        let initial_params = PatternParams {
            bit_index: 0,
            total_bits: self.horizontal_bits,
            direction: 1,
            inverted: 0,
            proj_width: width as f32,
            proj_height: height as f32,
            pattern_type: PatternType::GrayCode as u32,
            _padding: 0,
        };

        self.params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Pattern Params Buffer"),
            contents: bytemuck::cast_slice(&[initial_params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Pattern Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.params_buffer.as_entire_binding(),
            }],
        });
    }

    /// Render a Gray code pattern.
    pub fn render_pattern<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        spec: &PatternSpec,
    ) {
        let total_bits = match spec.direction {
            PatternDirection::Horizontal => self.vertical_bits,
            PatternDirection::Vertical => self.horizontal_bits,
        };

        let params = PatternParams {
            bit_index: spec.bit_index,
            total_bits,
            direction: match spec.direction {
                PatternDirection::Horizontal => 0,
                PatternDirection::Vertical => 1,
            },
            inverted: if spec.inverted { 1 } else { 0 },
            proj_width: self.proj_width as f32,
            proj_height: self.proj_height as f32,
            pattern_type: PatternType::GrayCode as u32,
            _padding: 0,
        };

        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[params]));

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    /// Render a white reference pattern.
    pub fn render_white<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue) {
        let params = PatternParams {
            bit_index: 0,
            total_bits: 0,
            direction: 0,
            inverted: 0,
            proj_width: self.proj_width as f32,
            proj_height: self.proj_height as f32,
            pattern_type: PatternType::White as u32,
            _padding: 0,
        };

        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[params]));

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    /// Render a black reference pattern.
    pub fn render_black<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue) {
        let params = PatternParams {
            bit_index: 0,
            total_bits: 0,
            direction: 0,
            inverted: 0,
            proj_width: self.proj_width as f32,
            proj_height: self.proj_height as f32,
            pattern_type: PatternType::Black as u32,
            _padding: 0,
        };

        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[params]));

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    pub fn horizontal_bits(&self) -> u32 {
        self.horizontal_bits
    }

    pub fn vertical_bits(&self) -> u32 {
        self.vertical_bits
    }
}
