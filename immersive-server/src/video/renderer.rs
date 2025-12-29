//! Video renderer for displaying video textures
//!
//! Provides a render pipeline and utilities for displaying video frames
//! using the fullscreen quad shader.

use super::VideoTexture;

/// Parameters for video display, matching the shader uniform
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VideoParams {
    /// Scale factor for aspect ratio (1.0, 1.0 = fill screen)
    pub scale: [f32; 2],
    /// Offset for centering
    pub offset: [f32; 2],
    /// Opacity (0.0 - 1.0)
    pub opacity: f32,
    /// Padding for 16-byte alignment
    pub _padding: [f32; 3],
}

impl Default for VideoParams {
    fn default() -> Self {
        Self {
            scale: [1.0, 1.0],
            offset: [0.0, 0.0],
            opacity: 1.0,
            _padding: [0.0; 3],
        }
    }
}

impl VideoParams {
    /// Create params that preserve aspect ratio (letterbox/pillarbox)
    pub fn fit_aspect_ratio(video_width: u32, video_height: u32, screen_width: u32, screen_height: u32) -> Self {
        let video_aspect = video_width as f32 / video_height as f32;
        let screen_aspect = screen_width as f32 / screen_height as f32;

        let (scale_x, scale_y) = if video_aspect > screen_aspect {
            // Video is wider than screen - pillarbox (black bars on sides)
            (1.0, screen_aspect / video_aspect)
        } else {
            // Video is taller than screen - letterbox (black bars top/bottom)
            (video_aspect / screen_aspect, 1.0)
        };

        Self {
            scale: [scale_x, scale_y],
            offset: [0.0, 0.0],
            opacity: 1.0,
            _padding: [0.0; 3],
        }
    }

    /// Create params that stretch to fill (no aspect ratio preservation)
    pub fn stretch() -> Self {
        Self::default()
    }
}

/// Video renderer that displays video textures using a fullscreen quad
pub struct VideoRenderer {
    /// Render pipeline for video display
    pipeline: wgpu::RenderPipeline,
    /// Bind group layout for video texture + sampler + params
    bind_group_layout: wgpu::BindGroupLayout,
    /// Sampler for video texture filtering
    sampler: wgpu::Sampler,
    /// Uniform buffer for video parameters
    params_buffer: wgpu::Buffer,
    /// Current video parameters
    current_params: VideoParams,
}

impl VideoRenderer {
    /// Create a new video renderer
    ///
    /// # Arguments
    /// * `device` - The wgpu device
    /// * `output_format` - The format of the render target (e.g., surface format)
    pub fn new(device: &wgpu::Device, output_format: wgpu::TextureFormat) -> Self {
        // Create shader module from embedded WGSL
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Video Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/fullscreen_quad.wgsl").into()),
        });

        // Create sampler for video texture
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Video Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Video Bind Group Layout"),
            entries: &[
                // Texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Uniform buffer for params
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Video Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Video Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
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

        // Create uniform buffer for params
        let current_params = VideoParams::default();
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Video Params Buffer"),
            size: std::mem::size_of::<VideoParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            params_buffer,
            current_params,
        }
    }

    /// Get the bind group layout for creating video textures
    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    /// Get the sampler for creating video textures
    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    /// Update video display parameters
    pub fn set_params(&mut self, queue: &wgpu::Queue, params: VideoParams) {
        self.current_params = params;
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&self.current_params));
    }

    /// Create a bind group for a video texture
    pub fn create_bind_group(&self, device: &wgpu::Device, video_texture: &VideoTexture) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Video Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(video_texture.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Render video to the output texture
    ///
    /// # Arguments
    /// * `encoder` - Command encoder for recording render commands
    /// * `output_view` - The texture view to render to
    /// * `bind_group` - The bind group containing video texture and params
    /// * `clear` - Whether to clear the output before rendering
    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        bind_group: &wgpu::BindGroup,
        clear: bool,
    ) {
        let load_op = if clear {
            wgpu::LoadOp::Clear(wgpu::Color::BLACK)
        } else {
            wgpu::LoadOp::Load
        };

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Video Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: load_op,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        // Draw fullscreen triangle (3 vertices, 1 instance)
        render_pass.draw(0..3, 0..1);
    }

    /// Convenience method to render a video texture directly
    ///
    /// Creates a temporary bind group and renders. For frequent rendering,
    /// prefer creating a bind group once and reusing it.
    pub fn render_texture(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        video_texture: &VideoTexture,
        clear: bool,
    ) {
        let bind_group = self.create_bind_group(device, video_texture);
        self.render(encoder, output_view, &bind_group, clear);
    }
}


