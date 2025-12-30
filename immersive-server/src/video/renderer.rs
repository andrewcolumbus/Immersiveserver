//! Video renderer for displaying video textures
//!
//! Provides a render pipeline and utilities for displaying video frames
//! using the fullscreen quad shader.

use std::collections::HashMap;

use super::VideoTexture;
use crate::compositor::{BlendMode, Layer, Transform2D};

/// Parameters for video display, matching the shader uniform (legacy)
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

    /// Create params for native pixel size rendering (allows spill-over for large videos)
    /// Videos larger than environment will spill over edges.
    /// Videos smaller than environment won't fill the canvas.
    pub fn native_size(video_width: u32, video_height: u32, env_width: u32, env_height: u32) -> Self {
        // Calculate scale such that the video renders at its native pixel size
        // relative to the environment.
        // scale = video_size / env_size
        // A video twice as wide as env will have scale_x = 2.0 (spills over)
        // A video half as wide as env will have scale_x = 0.5 (doesn't fill)
        let scale_x = video_width as f32 / env_width as f32;
        let scale_y = video_height as f32 / env_height as f32;

        Self {
            scale: [scale_x, scale_y],
            offset: [0.0, 0.0],
            opacity: 1.0,
            _padding: [0.0; 3],
        }
    }
}

/// Parameters for layer rendering with full 2D transforms.
///
/// This struct matches the LayerParams uniform in fullscreen_quad.wgsl.
/// It includes size scaling, position, rotation, opacity, and tiling.
///
/// IMPORTANT: WGSL struct alignment rules require vec2<f32> to be 8-byte aligned.
/// After `rotation: f32`, we must add 4 bytes of padding before `anchor: vec2<f32>`.
/// Total size: 56 bytes (14 floats × 4 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LayerParams {
    /// Video/layer size relative to environment (video_size / env_size)
    pub size_scale: [f32; 2],          // offset 0, size 8
    /// Position in normalized coordinates (0-1, where 0.5,0.5 = center)
    pub position: [f32; 2],            // offset 8, size 8
    /// Scale factors for the transform (1.0 = 100%)
    pub scale: [f32; 2],               // offset 16, size 8
    /// Rotation in radians (clockwise)
    pub rotation: f32,                 // offset 24, size 4
    /// Environment aspect ratio (width / height) for correct rotation
    pub env_aspect: f32,               // offset 28, size 4
    /// Anchor point for rotation/scaling (0-1, where 0.5,0.5 = center)
    pub anchor: [f32; 2],              // offset 32, size 8
    /// Opacity (0.0 - 1.0)
    pub opacity: f32,                  // offset 40, size 4
    /// Padding for WGSL vec2 alignment
    pub _pad_opacity: f32,             // offset 44, size 4
    /// Tiling factors (1.0 = no repeat, 2.0 = 2x2 grid, etc.)
    pub tile: [f32; 2],                // offset 48, size 8
}                                      // Total: 56 bytes

impl Default for LayerParams {
    fn default() -> Self {
        Self {
            size_scale: [1.0, 1.0],
            position: [0.0, 0.0],
            scale: [1.0, 1.0],
            rotation: 0.0,
            env_aspect: 1.0,  // Default to square aspect
            anchor: [0.5, 0.5],
            opacity: 1.0,
            _pad_opacity: 0.0,
            tile: [1.0, 1.0],
        }
    }
}

impl LayerParams {
    /// Create LayerParams from a Layer and video/environment dimensions.
    ///
    /// Converts pixel-based position to normalized coordinates and
    /// copies transform parameters from the layer.
    pub fn from_layer(
        layer: &Layer,
        video_width: u32,
        video_height: u32,
        env_width: u32,
        env_height: u32,
    ) -> Self {
        // Size scale: how big is the video relative to the environment
        let size_scale_x = video_width as f32 / env_width as f32;
        let size_scale_y = video_height as f32 / env_height as f32;

        // Convert pixel position to normalized coordinates (0-1)
        // Position (0,0) means layer anchor is at environment top-left
        // We need to account for the layer's size when positioning
        let pos_norm_x = layer.transform.position.0 / env_width as f32;
        let pos_norm_y = layer.transform.position.1 / env_height as f32;

        Self {
            size_scale: [size_scale_x, size_scale_y],
            position: [pos_norm_x, pos_norm_y],
            scale: [layer.transform.scale.0, layer.transform.scale.1],
            rotation: layer.transform.rotation,
            env_aspect: env_width as f32 / env_height.max(1) as f32,
            anchor: [layer.transform.anchor.0, layer.transform.anchor.1],
            opacity: layer.opacity,
            _pad_opacity: 0.0,
            tile: [layer.tile_x as f32, layer.tile_y as f32],
        }
    }

    /// Create LayerParams from a Transform2D and dimensions.
    pub fn from_transform(
        transform: &Transform2D,
        opacity: f32,
        video_width: u32,
        video_height: u32,
        env_width: u32,
        env_height: u32,
    ) -> Self {
        let size_scale_x = video_width as f32 / env_width as f32;
        let size_scale_y = video_height as f32 / env_height as f32;

        let pos_norm_x = transform.position.0 / env_width as f32;
        let pos_norm_y = transform.position.1 / env_height as f32;

        Self {
            size_scale: [size_scale_x, size_scale_y],
            position: [pos_norm_x, pos_norm_y],
            scale: [transform.scale.0, transform.scale.1],
            rotation: transform.rotation,
            env_aspect: env_width as f32 / env_height.max(1) as f32,
            anchor: [transform.anchor.0, transform.anchor.1],
            opacity,
            _pad_opacity: 0.0,
            tile: [1.0, 1.0],
        }
    }

    /// Create a simple identity LayerParams (no transform, full opacity)
    pub fn identity(video_width: u32, video_height: u32, env_width: u32, env_height: u32) -> Self {
        Self {
            size_scale: [
                video_width as f32 / env_width as f32,
                video_height as f32 / env_height as f32,
            ],
            position: [0.0, 0.0],
            scale: [1.0, 1.0],
            rotation: 0.0,
            env_aspect: env_width as f32 / env_height.max(1) as f32,
            anchor: [0.5, 0.5],
            opacity: 1.0,
            _pad_opacity: 0.0,
            tile: [1.0, 1.0],
        }
    }
}

/// Video renderer that displays video textures using a fullscreen quad.
///
/// Supports multiple blend modes by maintaining separate render pipelines
/// for each blend mode (Normal, Additive, Multiply, Screen).
///
/// With the `shader-hotreload` feature enabled, shaders can be reloaded
/// at runtime without restarting the application.
pub struct VideoRenderer {
    /// Render pipelines for each blend mode
    pipelines: HashMap<BlendMode, wgpu::RenderPipeline>,
    /// Bind group layout for video texture + sampler + params
    bind_group_layout: wgpu::BindGroupLayout,
    /// Pipeline layout (stored for hot-reload)
    pipeline_layout: wgpu::PipelineLayout,
    /// Output texture format (stored for hot-reload)
    output_format: wgpu::TextureFormat,
    /// Sampler for video texture filtering
    sampler: wgpu::Sampler,
    /// Uniform buffer for layer parameters (sized for LayerParams)
    params_buffer: wgpu::Buffer,
    /// Current layer parameters
    current_layer_params: LayerParams,
}

impl VideoRenderer {
    /// Create a new video renderer
    ///
    /// # Arguments
    /// * `device` - The wgpu device
    /// * `output_format` - The format of the render target (e.g., surface format)
    pub fn new(device: &wgpu::Device, output_format: wgpu::TextureFormat) -> Self {
        // Load shader source (from disk in dev mode, embedded in release)
        let shader_source = crate::shaders::load_fullscreen_quad_shader()
            .expect("Failed to load fullscreen quad shader");

        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Video Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
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

        // Create render pipelines for each blend mode
        let mut pipelines = HashMap::new();
        for blend_mode in BlendMode::all() {
            let pipeline = Self::create_pipeline(
                device,
                &shader,
                &pipeline_layout,
                output_format,
                *blend_mode,
            );
            pipelines.insert(*blend_mode, pipeline);
        }

        // Create uniform buffer for params (sized for LayerParams which is larger)
        let current_layer_params = LayerParams::default();
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Layer Params Buffer"),
            size: std::mem::size_of::<LayerParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipelines,
            bind_group_layout,
            pipeline_layout,
            output_format,
            sampler,
            params_buffer,
            current_layer_params,
        }
    }

    /// Create a render pipeline with a specific blend mode
    fn create_pipeline(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        pipeline_layout: &wgpu::PipelineLayout,
        output_format: wgpu::TextureFormat,
        blend_mode: BlendMode,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("Video Pipeline ({})", blend_mode.name())),
            layout: Some(pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
                    blend: Some(blend_mode.to_blend_state()),
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
        })
    }

    /// Rebuild all pipelines with new shader source (hot-reload)
    ///
    /// This method compiles the shader source and recreates all blend mode
    /// pipelines. If compilation fails, an error is returned and the existing
    /// pipelines remain unchanged.
    ///
    /// # Arguments
    /// * `device` - The wgpu device
    /// * `shader_source` - WGSL shader source code
    ///
    /// # Returns
    /// * `Ok(())` if pipelines were rebuilt successfully
    /// * `Err(String)` with error message if shader compilation failed
    pub fn rebuild_pipelines(&mut self, device: &wgpu::Device, shader_source: &str) -> Result<(), String> {
        // Try to compile the new shader
        // Note: wgpu doesn't provide detailed error info on creation failure,
        // but validation errors will cause a panic in debug or be logged
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Video Shader (hot-reload)"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Rebuild all pipelines with the new shader
        let mut new_pipelines = HashMap::new();
        for blend_mode in BlendMode::all() {
            let pipeline = Self::create_pipeline(
                device,
                &shader,
                &self.pipeline_layout,
                self.output_format,
                *blend_mode,
            );
            new_pipelines.insert(*blend_mode, pipeline);
        }

        // Replace old pipelines with new ones
        self.pipelines = new_pipelines;
        log::info!("✅ Shader hot-reload successful");

        Ok(())
    }

    /// Get the bind group layout for creating video textures
    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    /// Get the sampler for creating video textures
    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    /// Update layer display parameters (full transform support)
    pub fn set_layer_params(&mut self, queue: &wgpu::Queue, params: LayerParams) {
        self.current_layer_params = params;
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&self.current_layer_params));
    }

    /// Update video display parameters (legacy - converts to LayerParams)
    pub fn set_params(&mut self, queue: &wgpu::Queue, params: VideoParams) {
        // Convert VideoParams to LayerParams for backward compatibility
        let layer_params = LayerParams {
            size_scale: params.scale,
            position: params.offset,
            scale: [1.0, 1.0],
            rotation: 0.0,
            env_aspect: 1.0,  // Legacy - assume square aspect
            anchor: [0.5, 0.5],
            opacity: params.opacity,
            _pad_opacity: 0.0,
            tile: [1.0, 1.0],
        };
        self.set_layer_params(queue, layer_params);
    }

    /// Create a bind group for a video texture using the shared params buffer.
    /// 
    /// NOTE: This uses a shared params buffer which will be overwritten when
    /// rendering multiple layers. For multi-layer rendering, use
    /// `create_bind_group_with_buffer` with a per-layer params buffer instead.
    pub fn create_bind_group(&self, device: &wgpu::Device, video_texture: &VideoTexture) -> wgpu::BindGroup {
        self.create_bind_group_with_buffer(device, video_texture, &self.params_buffer)
    }

    /// Create a bind group for a video texture with a custom params buffer.
    ///
    /// This allows each layer to have its own params buffer, avoiding overwrites
    /// when rendering multiple layers in sequence.
    ///
    /// # Arguments
    /// * `device` - The wgpu device
    /// * `video_texture` - The video texture to bind
    /// * `params_buffer` - The per-layer params buffer for GPU uniforms
    pub fn create_bind_group_with_buffer(
        &self,
        device: &wgpu::Device,
        video_texture: &VideoTexture,
        params_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
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
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Create a new params buffer for a layer.
    ///
    /// Each layer should have its own params buffer to avoid overwrites
    /// when rendering multiple layers in sequence.
    pub fn create_params_buffer(&self, device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Layer Params Buffer"),
            size: std::mem::size_of::<LayerParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Write layer params to a specific buffer.
    ///
    /// Used to update per-layer params buffers during rendering.
    pub fn write_layer_params(&self, queue: &wgpu::Queue, buffer: &wgpu::Buffer, params: &LayerParams) {
        queue.write_buffer(buffer, 0, bytemuck::bytes_of(params));
    }

    /// Render video to the output texture
    /// Render video to the output texture with a specific blend mode
    ///
    /// # Arguments
    /// * `encoder` - Command encoder for recording render commands
    /// * `output_view` - The texture view to render to
    /// * `bind_group` - The bind group containing video texture and params
    /// * `blend_mode` - The blend mode to use for compositing
    /// * `clear` - Whether to clear the output before rendering
    pub fn render_with_blend(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        bind_group: &wgpu::BindGroup,
        blend_mode: BlendMode,
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

        // Select the pipeline for the requested blend mode
        let pipeline = self
            .pipelines
            .get(&blend_mode)
            .expect("Pipeline for blend mode should exist");

        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        // Draw fullscreen triangle (3 vertices, 1 instance)
        render_pass.draw(0..3, 0..1);
    }

    /// Render video to the output texture with default Normal blend mode
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
        self.render_with_blend(encoder, output_view, bind_group, BlendMode::Normal, clear);
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


