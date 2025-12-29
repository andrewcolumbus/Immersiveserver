//! Compositor for rendering composition output
//!
//! Renders layers with blend modes and transforms to an internal texture for display.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;
use wgpu;

// Import composition types - using super to avoid circular dependencies
// These will be passed in from the app layer
use crate::composition::{BlendMode, Composition, Layer};
use crate::video::HapFrame;

/// Uniforms for layer rendering
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LayerUniforms {
    /// Transform matrix row 0: [m00, m01, m02, 0]
    pub transform_row0: [f32; 4],
    /// Transform matrix row 1: [m10, m11, m12, 0]
    pub transform_row1: [f32; 4],
    /// Transform matrix row 2: [m20, m21, m22, 0]
    pub transform_row2: [f32; 4],
    /// Properties: [opacity, blend_mode, 0, 0]
    pub properties: [f32; 4],
}

impl Default for LayerUniforms {
    fn default() -> Self {
        Self {
            transform_row0: [1.0, 0.0, 0.0, 0.0],
            transform_row1: [0.0, 1.0, 0.0, 0.0],
            transform_row2: [0.0, 0.0, 1.0, 0.0],
            properties: [1.0, 0.0, 0.0, 0.0],
        }
    }
}

impl LayerUniforms {
    /// Create uniforms from a layer
    pub fn from_layer(layer: &Layer) -> Self {
        let matrix = layer.transform.to_matrix();
        Self {
            transform_row0: [matrix[0][0], matrix[0][1], matrix[0][2], 0.0],
            transform_row1: [matrix[1][0], matrix[1][1], matrix[1][2], 0.0],
            transform_row2: [matrix[2][0], matrix[2][1], matrix[2][2], 0.0],
            properties: [
                layer.effective_opacity(),
                layer.blend_mode.shader_index() as f32,
                0.0,
                0.0,
            ],
        }
    }
}

/// Compositor for rendering the composition
pub struct Compositor {
    device: Option<Arc<wgpu::Device>>,
    queue: Option<Arc<wgpu::Queue>>,
    /// Composition render target texture
    composition_texture: Option<wgpu::Texture>,
    composition_view: Option<wgpu::TextureView>,
    /// Scratch texture for multi-pass blending
    scratch_texture: Option<wgpu::Texture>,
    scratch_view: Option<wgpu::TextureView>,
    /// Composition dimensions
    width: u32,
    height: u32,
    /// Layer composite pipeline
    layer_pipeline: Option<wgpu::RenderPipeline>,
    /// Layer uniform bind group layout
    layer_uniform_layout: Option<wgpu::BindGroupLayout>,
    /// Texture bind group layout
    texture_bind_group_layout: Option<wgpu::BindGroupLayout>,
    /// Sampler for textures
    sampler: Option<wgpu::Sampler>,
    /// Layer uniform buffer
    layer_uniform_buffer: Option<wgpu::Buffer>,
    /// Fullscreen quad vertex buffer
    vertex_buffer: Option<wgpu::Buffer>,
}

impl Default for Compositor {
    fn default() -> Self {
        Self::new()
    }
}

impl Compositor {
    /// Create a new compositor
    pub fn new() -> Self {
        Self {
            device: None,
            queue: None,
            composition_texture: None,
            composition_view: None,
            scratch_texture: None,
            scratch_view: None,
            width: 1920,
            height: 1080,
            layer_pipeline: None,
            layer_uniform_layout: None,
            texture_bind_group_layout: None,
            sampler: None,
            layer_uniform_buffer: None,
            vertex_buffer: None,
        }
    }

    /// Initialize with GPU resources
    pub fn initialize(&mut self, device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) {
        self.device = Some(device.clone());
        self.queue = Some(queue);
        self.create_composition_texture();
        self.create_pipeline(&device);
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.device.is_some() && self.queue.is_some()
    }

    /// Create the composition render target texture
    fn create_composition_texture(&mut self) {
        let Some(device) = &self.device else { return };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Composition Texture"),
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create scratch texture for blending
        let scratch = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Scratch Texture"),
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let scratch_view = scratch.create_view(&wgpu::TextureViewDescriptor::default());

        self.composition_texture = Some(texture);
        self.composition_view = Some(view);
        self.scratch_texture = Some(scratch);
        self.scratch_view = Some(scratch_view);

        log::info!("Created composition texture {}x{}", self.width, self.height);
    }

    /// Create the layer compositing pipeline
    fn create_pipeline(&mut self, device: &wgpu::Device) {
        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Layer Composite Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/layer_composite.wgsl").into()),
        });

        // Create sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Layer Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create bind group layouts
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Layer Texture Bind Group Layout"),
                entries: &[
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let layer_uniform_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Layer Uniform Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Layer Composite Pipeline Layout"),
            bind_group_layouts: &[&texture_bind_group_layout, &layer_uniform_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline with alpha blending
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Layer Composite Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[super::Vertex::LAYOUT],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        // Create layer uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Layer Uniform Buffer"),
            size: std::mem::size_of::<LayerUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create vertex buffer
        let vertices = super::fullscreen_quad();
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Compositor Vertex Buffer"),
            size: (std::mem::size_of::<super::Vertex>() * vertices.len()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Write vertex data
        if let Some(queue) = &self.queue {
            queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        }

        self.layer_pipeline = Some(pipeline);
        self.layer_uniform_layout = Some(layer_uniform_layout);
        self.texture_bind_group_layout = Some(texture_bind_group_layout);
        self.sampler = Some(sampler);
        self.layer_uniform_buffer = Some(uniform_buffer);
        self.vertex_buffer = Some(vertex_buffer);
    }

    /// Resize the composition
    pub fn resize(&mut self, width: u32, height: u32) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.create_composition_texture();
        }
    }

    /// Get the composition texture view
    pub fn composition_view(&self) -> Option<&wgpu::TextureView> {
        self.composition_view.as_ref()
    }

    /// Get composition dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Render a composition to the output texture
    /// layer_textures: map from layer index to the layer's current texture view
    pub fn render_composition(
        &mut self,
        composition: &Composition,
        layer_textures: &HashMap<usize, wgpu::TextureView>,
    ) {
        let Some(device) = &self.device else { return };
        let Some(queue) = &self.queue else { return };
        let Some(view) = &self.composition_view else { return };

        let bg_color = composition.settings.background_color;

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Composition Encoder"),
        });

        // Clear pass with background color
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Composition Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: bg_color[0] as f64,
                            g: bg_color[1] as f64,
                            b: bg_color[2] as f64,
                            a: bg_color[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }

        // Submit the clear pass first
        queue.submit(std::iter::once(encoder.finish()));

        // Render visible layers from bottom to top
        // Layers are stored bottom-first, so we iterate in reverse to render top-last
        for (layer_idx, layer) in composition.layers.iter().enumerate().rev() {
            // Skip bypassed layers or layers that don't have visible content
            if layer.bypass || !layer.is_playing() {
                continue;
            }

            // Check if we have a texture for this layer
            if let Some(layer_texture) = layer_textures.get(&layer_idx) {
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some(&format!("Layer {} Encoder", layer_idx)),
                });
                
                self.render_layer(&mut encoder, layer, layer_texture);
                
                queue.submit(std::iter::once(encoder.finish()));
            }
        }
    }

    /// Upload a HapFrame to a GPU texture and return it
    pub fn upload_frame(&self, frame: &HapFrame) -> Option<wgpu::Texture> {
        let device = self.device.as_ref()?;
        let queue = self.queue.as_ref()?;

        // Create texture with RGBA8 format for egui compatibility
        // HAP frames are DXT compressed, but for simplicity we'll convert to RGBA8
        // In a production app, you'd use the native BC format directly
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Layer Frame Texture"),
            size: wgpu::Extent3d {
                width: frame.width,
                height: frame.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // For now, generate a simple test pattern based on frame data
        // In production, you'd decode the DXT data properly
        let pixel_count = (frame.width * frame.height) as usize;
        let mut rgba_data = vec![0u8; pixel_count * 4];
        
        // Generate a gradient pattern based on frame timestamp
        let t = (frame.timestamp * 2.0).sin() as f32 * 0.5 + 0.5;
        for y in 0..frame.height {
            for x in 0..frame.width {
                let idx = ((y * frame.width + x) * 4) as usize;
                let u = x as f32 / frame.width as f32;
                let v = y as f32 / frame.height as f32;
                
                rgba_data[idx] = ((u * t + 0.2) * 255.0) as u8;     // R
                rgba_data[idx + 1] = ((v * (1.0 - t) + 0.1) * 255.0) as u8; // G
                rgba_data[idx + 2] = (((1.0 - u) * t + 0.3) * 255.0) as u8; // B
                rgba_data[idx + 3] = 255; // A
            }
        }

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(frame.width * 4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: frame.width,
                height: frame.height,
                depth_or_array_layers: 1,
            },
        );

        Some(texture)
    }

    /// Render a single layer to the composition
    /// This is called for each visible layer in order
    fn render_layer(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        layer: &Layer,
        _layer_texture: &wgpu::TextureView,
    ) {
        let Some(device) = &self.device else { return };
        let Some(queue) = &self.queue else { return };
        let Some(view) = &self.composition_view else { return };
        let Some(pipeline) = &self.layer_pipeline else {
            return;
        };
        let Some(uniform_buffer) = &self.layer_uniform_buffer else {
            return;
        };
        let Some(vertex_buffer) = &self.vertex_buffer else {
            return;
        };
        let Some(texture_layout) = &self.texture_bind_group_layout else {
            return;
        };
        let Some(uniform_layout) = &self.layer_uniform_layout else {
            return;
        };
        let Some(sampler) = &self.sampler else { return };

        // Update layer uniforms
        let uniforms = LayerUniforms::from_layer(layer);
        queue.write_buffer(uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        // Create bind groups
        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Layer Texture Bind Group"),
            layout: texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(_layer_texture),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Layer Uniform Bind Group"),
            layout: uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Render pass
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Layer Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Preserve existing content
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(0, &texture_bind_group, &[]);
            render_pass.set_bind_group(1, &uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.draw(0..6, 0..1);
        }
    }

    /// Render a test pattern to the composition
    pub fn render_test_pattern(&mut self) {
        let Some(device) = &self.device else { return };
        let Some(queue) = &self.queue else { return };
        let Some(view) = &self.composition_view else { return };

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Test Pattern Encoder"),
        });

        // Clear to a gradient color
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Test Pattern Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.15,
                            b: 0.2,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }

        queue.submit(std::iter::once(encoder.finish()));
    }

    /// Clear the composition to a solid color
    pub fn clear(&mut self, color: [f32; 4]) {
        let Some(device) = &self.device else { return };
        let Some(queue) = &self.queue else { return };
        let Some(view) = &self.composition_view else { return };

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Clear Encoder"),
        });

        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: color[0] as f64,
                            g: color[1] as f64,
                            b: color[2] as f64,
                            a: color[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }

        queue.submit(std::iter::once(encoder.finish()));
    }

    /// Get blend state for a specific blend mode
    pub fn blend_state_for_mode(mode: BlendMode) -> wgpu::BlendState {
        match mode {
            BlendMode::Normal => wgpu::BlendState::ALPHA_BLENDING,
            BlendMode::Add => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            },
            BlendMode::Multiply => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Dst,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::OVER,
            },
            BlendMode::Screen => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrc,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::OVER,
            },
            BlendMode::Overlay => {
                // Overlay is complex and needs shader-based blending
                // Fall back to normal for hardware blend
                wgpu::BlendState::ALPHA_BLENDING
            }
        }
    }
}
