//! Previs renderer for 3D surface visualization
//!
//! Renders the environment texture onto 3D meshes (circle, walls, dome).

use bytemuck::{Pod, Zeroable};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use wgpu::util::DeviceExt;

use super::camera::OrbitCamera;
use super::mesh::{PrevisMesh, PrevisVertex};
use super::types::{PrevisSettings, SurfaceType};

/// Camera uniform buffer data
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct CameraUniforms {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 3],
    _padding: f32,
}

/// Hash key for mesh regeneration detection
fn hash_settings(settings: &PrevisSettings) -> u64 {
    let mut hasher = DefaultHasher::new();
    // Hash relevant fields
    std::mem::discriminant(&settings.surface_type).hash(&mut hasher);
    settings.circle_radius.to_bits().hash(&mut hasher);
    settings.circle_segments.hash(&mut hasher);
    // Individual wall settings
    settings.wall_front.enabled.hash(&mut hasher);
    settings.wall_front.width.to_bits().hash(&mut hasher);
    settings.wall_front.height.to_bits().hash(&mut hasher);
    settings.wall_back.enabled.hash(&mut hasher);
    settings.wall_back.width.to_bits().hash(&mut hasher);
    settings.wall_back.height.to_bits().hash(&mut hasher);
    settings.wall_left.enabled.hash(&mut hasher);
    settings.wall_left.width.to_bits().hash(&mut hasher);
    settings.wall_left.height.to_bits().hash(&mut hasher);
    settings.wall_right.enabled.hash(&mut hasher);
    settings.wall_right.width.to_bits().hash(&mut hasher);
    settings.wall_right.height.to_bits().hash(&mut hasher);
    settings.dome_radius.to_bits().hash(&mut hasher);
    settings.dome_segments_horizontal.hash(&mut hasher);
    settings.dome_segments_vertical.hash(&mut hasher);
    hasher.finish()
}

/// GPU renderer for 3D previs
pub struct PrevisRenderer {
    // Render pipeline
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,

    // Camera uniforms
    camera_buffer: wgpu::Buffer,

    // Mesh buffers (recreated when surface type changes)
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    index_count: u32,

    // Render target (panel's own texture)
    render_texture: Option<wgpu::Texture>,
    render_view: Option<wgpu::TextureView>,
    depth_texture: Option<wgpu::Texture>,
    depth_view: Option<wgpu::TextureView>,
    render_width: u32,
    render_height: u32,

    // Sampler for environment texture
    sampler: wgpu::Sampler,

    // Current surface settings hash (for mesh regeneration)
    current_settings_hash: u64,

    // Camera
    camera: OrbitCamera,
}

impl PrevisRenderer {
    /// Create a new previs renderer
    pub fn new(device: &wgpu::Device) -> Self {
        // Load 3D shader
        let shader_source = include_str!("../shaders/previs_3d.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Previs 3D Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Bind group layout: [0] camera uniforms, [1] environment texture, [2] sampler
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Previs Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Previs Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Render pipeline with depth testing
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Previs Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[PrevisVertex::buffer_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Disable culling so all walls are visible from any angle
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Camera uniform buffer
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Previs Camera Buffer"),
            size: std::mem::size_of::<CameraUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Previs Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            pipeline,
            bind_group_layout,
            camera_buffer,
            vertex_buffer: None,
            index_buffer: None,
            index_count: 0,
            render_texture: None,
            render_view: None,
            depth_texture: None,
            depth_view: None,
            render_width: 0,
            render_height: 0,
            sampler,
            current_settings_hash: 0,
            camera: OrbitCamera::new(),
        }
    }

    /// Update mesh if surface type or parameters changed
    pub fn update_mesh(&mut self, device: &wgpu::Device, settings: &PrevisSettings) {
        let settings_hash = hash_settings(settings);

        if self.current_settings_hash != settings_hash {
            let mesh = match settings.surface_type {
                SurfaceType::Circle => {
                    PrevisMesh::circle(settings.circle_radius, settings.circle_segments)
                }
                SurfaceType::Walls => PrevisMesh::walls_individual(
                    &settings.wall_front,
                    &settings.wall_back,
                    &settings.wall_left,
                    &settings.wall_right,
                ),
                SurfaceType::Dome => PrevisMesh::dome(
                    settings.dome_radius,
                    settings.dome_segments_horizontal,
                    settings.dome_segments_vertical,
                ),
            };

            self.vertex_buffer = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Previs Vertex Buffer"),
                contents: bytemuck::cast_slice(&mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }));

            self.index_buffer = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Previs Index Buffer"),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            }));

            self.index_count = mesh.indices.len() as u32;
            self.current_settings_hash = settings_hash;

            // Adjust camera target based on surface type
            match settings.surface_type {
                SurfaceType::Circle => {
                    self.camera.set_target(glam::Vec3::new(0.0, 0.0, 0.0));
                }
                SurfaceType::Walls => {
                    // Target center of walls (mid-height of tallest wall)
                    let max_height = settings
                        .wall_front
                        .height
                        .max(settings.wall_back.height)
                        .max(settings.wall_left.height)
                        .max(settings.wall_right.height);
                    self.camera
                        .set_target(glam::Vec3::new(0.0, max_height / 2.0, 0.0));
                }
                SurfaceType::Dome => {
                    // Target base center of dome
                    self.camera.set_target(glam::Vec3::new(0.0, 0.0, 0.0));
                }
            }
        }
    }

    /// Ensure render target exists with correct size
    pub fn ensure_render_target(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);

        if self.render_width != width || self.render_height != height {
            // Color target
            let render_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Previs Render Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

            // Depth target
            let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Previs Depth Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });

            self.render_view = Some(render_texture.create_view(&Default::default()));
            self.depth_view = Some(depth_texture.create_view(&Default::default()));
            self.render_texture = Some(render_texture);
            self.depth_texture = Some(depth_texture);
            self.render_width = width;
            self.render_height = height;

            self.camera.set_aspect(width as f32 / height as f32);
        }
    }

    /// Render 3D scene to internal texture
    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        env_texture_view: &wgpu::TextureView,
        settings: &PrevisSettings,
    ) {
        // Update mesh if needed
        self.update_mesh(device, settings);

        // Update camera uniforms
        let view_proj = self.camera.view_projection_matrix();
        let eye_pos = self.camera.eye_position();
        let uniforms = CameraUniforms {
            view_proj: view_proj.to_cols_array_2d(),
            camera_pos: eye_pos.into(),
            _padding: 0.0,
        };
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniforms));

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Previs Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(env_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        // Render pass
        let render_view = match &self.render_view {
            Some(v) => v,
            None => return, // No render target
        };
        let depth_view = match &self.depth_view {
            Some(v) => v,
            None => return,
        };

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Previs Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: render_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.1,
                        b: 0.15,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);

        if let (Some(vb), Some(ib)) = (&self.vertex_buffer, &self.index_buffer) {
            render_pass.set_vertex_buffer(0, vb.slice(..));
            render_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..self.index_count, 0, 0..1);
        }
    }

    /// Get rendered texture for egui display
    pub fn texture(&self) -> Option<&wgpu::Texture> {
        self.render_texture.as_ref()
    }

    /// Get rendered texture view for egui display
    pub fn texture_view(&self) -> Option<&wgpu::TextureView> {
        self.render_view.as_ref()
    }

    /// Get render target dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.render_width, self.render_height)
    }

    /// Camera access for input handling
    pub fn camera_mut(&mut self) -> &mut OrbitCamera {
        &mut self.camera
    }

    /// Camera access (immutable)
    pub fn camera(&self) -> &OrbitCamera {
        &self.camera
    }

    /// Load camera state from settings
    pub fn load_camera_state(&mut self, settings: &PrevisSettings) {
        self.camera
            .set_state(settings.camera_yaw, settings.camera_pitch, settings.camera_distance);
    }
}
