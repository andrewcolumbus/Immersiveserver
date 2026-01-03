//! Application state holding wgpu graphics context
//!
//! This module contains the core graphics state including the wgpu device,
//! queue, surface, and configuration needed for rendering.

use std::sync::Arc;
use std::time::Instant;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::window::Window;

use bytemuck::{Pod, Zeroable};

use crate::camera::CameraCapture;
use crate::effects::person_particles::{PersonParticlesEffect, Particle, ParticleParams, ParticleShape, ColorMode, MAX_PARTICLES};
use crate::effects::EffectType;
use crate::ml::MlInference;

/// Mask parameters for masked passthrough shader
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MaskParams {
    threshold: f32,
    fade_amount: f32,
    _pad: [f32; 2],
}

#[cfg(target_os = "macos")]
use crate::network::SyphonSharer;

/// Main application state
pub struct App {
    /// Reference to the window
    window: Arc<Window>,
    /// The wgpu surface for presenting rendered frames
    surface: wgpu::Surface<'static>,
    /// The wgpu device for creating GPU resources
    device: wgpu::Device,
    /// The command queue for submitting GPU work
    queue: wgpu::Queue,
    /// Surface configuration
    config: wgpu::SurfaceConfiguration,
    /// Current window size in physical pixels
    size: PhysicalSize<u32>,

    // Camera capture
    camera: Option<CameraCapture>,
    camera_texture: Option<wgpu::Texture>,
    camera_texture_view: Option<wgpu::TextureView>,
    last_camera_frame: u64,

    // ML inference
    ml_inference: Option<MlInference>,

    // Effects
    current_effect: EffectType,
    effect_enabled: bool,
    person_particles: PersonParticlesEffect,

    // Particle rendering resources
    particle_buffer: wgpu::Buffer,
    particle_pipeline: wgpu::RenderPipeline,
    particle_bind_group_layout: wgpu::BindGroupLayout,
    particle_params_buffer: wgpu::Buffer,
    /// Default white texture for particles when no camera
    default_texture: wgpu::Texture,
    default_texture_view: wgpu::TextureView,

    // Output texture (what gets sent to Syphon/Spout)
    output_texture: wgpu::Texture,
    output_texture_view: wgpu::TextureView,
    output_bind_group: wgpu::BindGroup,

    // Syphon/Spout output
    #[cfg(target_os = "macos")]
    syphon_sharer: Option<SyphonSharer>,
    #[cfg(target_os = "macos")]
    metal_command_queue: Option<metal::CommandQueue>,
    output_enabled: bool,

    // Passthrough pipeline (camera -> output when no effect)
    passthrough_pipeline: wgpu::RenderPipeline,
    passthrough_bind_group_layout: wgpu::BindGroupLayout,
    passthrough_bind_group: Option<wgpu::BindGroup>,
    sampler: wgpu::Sampler,

    // Masked passthrough pipeline (camera masked by segmentation)
    masked_pipeline: wgpu::RenderPipeline,
    masked_bind_group_layout: wgpu::BindGroupLayout,
    mask_texture: wgpu::Texture,
    mask_texture_view: wgpu::TextureView,
    mask_params_buffer: wgpu::Buffer,

    // egui integration
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,

    // Frame timing
    last_frame_time: Instant,
    frame_count: u64,
    fps: f64,
    last_fps_update: Instant,
    frames_since_update: u64,

    // Mouse position
    cursor_position: (f32, f32),
}

impl App {
    /// Create a new App instance with initialized wgpu context
    pub async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        // Create wgpu instance
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .expect("Failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find suitable GPU adapter");

        log::info!("Using GPU: {}", adapter.get_info().name);
        log::info!("Backend: {:?}", adapter.get_info().backend);

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Camera Effects Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: adapter.limits(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .expect("Failed to create device");

        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        log::info!("Surface format: {:?}", surface_format);

        let present_mode = if surface_caps
            .present_modes
            .contains(&wgpu::PresentMode::Immediate)
        {
            wgpu::PresentMode::Immediate
        } else if surface_caps
            .present_modes
            .contains(&wgpu::PresentMode::Mailbox)
        {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::Fifo
        };

        log::info!("Present mode: {:?}", present_mode);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };

        surface.configure(&device, &config);

        // Create output texture (1920x1080 default)
        let output_width = 1920u32;
        let output_height = 1080u32;
        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Output Texture"),
            size: wgpu::Extent3d {
                width: output_width,
                height: output_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[wgpu::TextureFormat::Bgra8Unorm],
        });
        let output_texture_view =
            output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create passthrough pipeline
        let passthrough_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Passthrough Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/passthrough.wgsl").into()),
        });

        let passthrough_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Passthrough Bind Group Layout"),
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

        let passthrough_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Passthrough Pipeline Layout"),
                bind_group_layouts: &[&passthrough_bind_group_layout],
                push_constant_ranges: &[],
            });

        let passthrough_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Passthrough Pipeline"),
            layout: Some(&passthrough_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &passthrough_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &passthrough_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create output bind group (cached, since output texture doesn't change)
        let output_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Output Bind Group"),
            layout: &passthrough_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&output_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Create masked passthrough pipeline (for rendering camera masked by segmentation)
        let masked_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Masked Passthrough Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/masked_passthrough.wgsl").into()),
        });

        let masked_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Masked Bind Group Layout"),
                entries: &[
                    // Camera texture
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
                    // Mask texture (R32Float is not filterable)
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Mask params
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
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

        let masked_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Masked Pipeline Layout"),
                bind_group_layouts: &[&masked_bind_group_layout],
                push_constant_ranges: &[],
            });

        let masked_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Masked Pipeline"),
            layout: Some(&masked_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &masked_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &masked_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
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
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create mask texture (256x256 to match segmentation model output)
        let mask_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Mask Texture"),
            size: wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let mask_texture_view = mask_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create mask params buffer
        let mask_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Mask Params Buffer"),
            size: std::mem::size_of::<MaskParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create particle buffer (storage buffer for particle data)
        let particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particle Buffer"),
            size: (MAX_PARTICLES * std::mem::size_of::<Particle>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create particle params uniform buffer
        let particle_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particle Params Buffer"),
            size: std::mem::size_of::<ParticleParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create particle bind group layout
        let particle_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Particle Bind Group Layout"),
                entries: &[
                    // Particle storage buffer
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Camera texture
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
                    // Sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Params uniform
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // Create particle pipeline
        let particle_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/particle.wgsl").into()),
        });

        let particle_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Particle Pipeline Layout"),
                bind_group_layouts: &[&particle_bind_group_layout],
                push_constant_ranges: &[],
            });

        // Create default white texture for particles when no camera
        let default_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Default Texture"),
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Fill with white pixels
        let white_pixels: Vec<u8> = vec![255u8; 4 * 4 * 4]; // 4x4 RGBA white
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &default_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &white_pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * 4),
                rows_per_image: Some(4),
            },
            wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
        );

        let default_texture_view = default_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let particle_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle Pipeline"),
            layout: Some(&particle_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &particle_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &particle_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb, // Match output texture format (sRGB)
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
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
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Initialize egui
        let egui_ctx = egui::Context::default();
        let mut style = (*egui_ctx.style()).clone();
        style.visuals.window_shadow = egui::epaint::Shadow::NONE;
        egui_ctx.set_style(style);

        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        let egui_renderer = egui_wgpu::Renderer::new(&device, surface_format, None, 1, false);

        // Initialize Syphon on macOS
        #[cfg(target_os = "macos")]
        let (syphon_sharer, metal_command_queue) = Self::init_syphon(&device, output_width, output_height);

        let now = Instant::now();

        Self {
            window,
            surface,
            device,
            queue,
            config,
            size,
            camera: None,
            camera_texture: None,
            camera_texture_view: None,
            last_camera_frame: 0,
            ml_inference: None,
            current_effect: EffectType::PersonParticles,
            effect_enabled: true,
            person_particles: PersonParticlesEffect::new(),
            particle_buffer,
            particle_pipeline,
            particle_bind_group_layout,
            particle_params_buffer,
            default_texture,
            default_texture_view,
            output_texture,
            output_texture_view,
            output_bind_group,
            #[cfg(target_os = "macos")]
            syphon_sharer,
            #[cfg(target_os = "macos")]
            metal_command_queue,
            output_enabled: true,
            passthrough_pipeline,
            passthrough_bind_group_layout,
            passthrough_bind_group: None,
            sampler,
            masked_pipeline,
            masked_bind_group_layout,
            mask_texture,
            mask_texture_view,
            mask_params_buffer,
            egui_ctx,
            egui_state,
            egui_renderer,
            last_frame_time: now,
            frame_count: 0,
            fps: 60.0,
            last_fps_update: now,
            frames_since_update: 0,
            cursor_position: (0.0, 0.0),
        }
    }

    #[cfg(target_os = "macos")]
    fn init_syphon(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (Option<SyphonSharer>, Option<metal::CommandQueue>) {
        use crate::network::texture_share::TextureSharer;
        use metal::foreign_types::ForeignType;
        use std::ffi::c_void;

        // Get Metal device from wgpu
        let metal_device: Option<metal::Device> = unsafe {
            device.as_hal::<wgpu_hal::api::Metal, _, _>(|hal_device| {
                hal_device.map(|d| {
                    // Clone the device reference
                    d.raw_device().lock().to_owned()
                })
            })
        };

        match metal_device {
            Some(mtl_device) => {
                log::info!("Got Metal device for Syphon");

                // Create command queue
                let command_queue = mtl_device.new_command_queue();

                // Create Syphon sharer
                let mut sharer = SyphonSharer::new();
                sharer.set_dimensions(width, height);

                // Set Metal handles
                unsafe {
                    sharer.set_metal_handles(
                        mtl_device.as_ptr() as *mut c_void,
                        command_queue.as_ptr() as *mut c_void,
                    );
                }

                // Start Syphon server
                match sharer.start("Camera Effects") {
                    Ok(_) => {
                        log::info!("Syphon server started");
                        (Some(sharer), Some(command_queue))
                    }
                    Err(e) => {
                        log::warn!("Failed to start Syphon: {}", e);
                        (None, None)
                    }
                }
            }
            None => {
                log::warn!("Could not get Metal device - Syphon disabled");
                (None, None)
            }
        }
    }

    /// Handle a window event, returning true if egui consumed it
    pub fn handle_window_event(&mut self, event: &WindowEvent) -> bool {
        let response = self.egui_state.on_window_event(&self.window, event);
        response.consumed
    }

    /// Resize the surface
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Get current size
    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    /// Handle mouse movement
    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        self.cursor_position = (x, y);
    }

    /// Toggle effect on/off
    pub fn toggle_effect(&mut self) {
        self.effect_enabled = !self.effect_enabled;
        log::info!("Effect enabled: {}", self.effect_enabled);
    }

    /// Select an effect by index
    pub fn select_effect(&mut self, index: usize) {
        self.current_effect = match index {
            0 => EffectType::PersonParticles,
            1 => EffectType::HandInteraction,
            2 => EffectType::PaintWarp,
            _ => EffectType::PersonParticles,
        };
        log::info!("Selected effect: {:?}", self.current_effect);
    }

    /// Connect to a camera
    pub fn connect_camera(&mut self, camera_index: u32) {
        log::info!("Connecting to camera {}", camera_index);

        // Default to 1280x720 (camera may use different resolution)
        let width = 1280u32;
        let height = 720u32;

        match CameraCapture::new(camera_index, width, height) {
            Ok(capture) => {
                log::info!("Camera capture started (requested: {}x{})", width, height);
                self.camera = Some(capture);
                // Texture will be created lazily in update_camera when first frame arrives
                self.camera_texture = None;
                self.camera_texture_view = None;
                self.passthrough_bind_group = None;
                self.last_camera_frame = 0;

                // Auto-initialize ML when camera connects
                if self.ml_inference.is_none() {
                    self.init_ml();
                }
            }
            Err(e) => {
                log::error!("Failed to connect camera: {}", e);
            }
        }
    }

    /// Disconnect current camera
    pub fn disconnect_camera(&mut self) {
        if let Some(mut camera) = self.camera.take() {
            camera.stop();
        }
        self.camera_texture = None;
        self.camera_texture_view = None;
        self.passthrough_bind_group = None;
        log::info!("Camera disconnected");
    }

    /// Update camera capture - poll for new frames and upload to GPU
    pub fn update_camera(&mut self) {
        let Some(camera) = &self.camera else { return };

        // Check if there's a new frame
        let Some(frame) = camera.latest_frame() else { return };

        // Only process if this is a new frame
        if frame.frame_number <= self.last_camera_frame {
            return;
        }
        self.last_camera_frame = frame.frame_number;

        // Check if we need to (re)create the texture for this frame size
        let needs_new_texture = match &self.camera_texture {
            None => true,
            Some(tex) => {
                let size = tex.size();
                size.width != frame.width || size.height != frame.height
            }
        };

        if needs_new_texture {
            log::info!("Creating camera texture: {}x{}", frame.width, frame.height);

            let camera_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Camera Texture"),
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

            let camera_texture_view =
                camera_texture.create_view(&wgpu::TextureViewDescriptor::default());

            // Create bind group for camera texture
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Camera Bind Group"),
                layout: &self.passthrough_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&camera_texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });

            self.camera_texture = Some(camera_texture);
            self.camera_texture_view = Some(camera_texture_view);
            self.passthrough_bind_group = Some(bind_group);
        }

        // Upload frame data to GPU texture
        if let Some(camera_texture) = &self.camera_texture {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: camera_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &frame.data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(frame.width * 4),
                    rows_per_image: Some(frame.height),
                },
                wgpu::Extent3d {
                    width: frame.width,
                    height: frame.height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    /// Initialize ML inference
    pub fn init_ml(&mut self) {
        if self.ml_inference.is_some() {
            return;
        }

        log::info!("Initializing ML inference...");
        match MlInference::new() {
            Ok(ml) => {
                self.ml_inference = Some(ml);
                log::info!("ML inference initialized");
            }
            Err(e) => {
                log::warn!("Failed to initialize ML: {}", e);
            }
        }
    }

    /// Update ML inference - send frame for processing
    pub fn update_ml(&mut self) {
        // Send current camera frame to ML thread if available
        if let (Some(camera), Some(ml)) = (&self.camera, &self.ml_inference) {
            if let Some(frame) = camera.latest_frame() {
                ml.process_frame(&frame.data, frame.width, frame.height, frame.frame_number);
            }
        }
    }

    /// Get latest ML result
    pub fn ml_result(&self) -> Option<crate::ml::MlResult> {
        self.ml_inference.as_ref().map(|ml| ml.latest_result())
    }

    /// Check if ML is ready
    pub fn is_ml_ready(&self) -> bool {
        self.ml_inference.as_ref().map(|ml| ml.is_ready()).unwrap_or(false)
    }

    /// Update effects
    pub fn update_effects(&mut self, delta_time: f32) {
        if !self.effect_enabled {
            return;
        }

        // Get ML result for segmentation
        let ml_result = self.ml_result();

        match self.current_effect {
            EffectType::PersonParticles => {
                let segmentation = ml_result.as_ref().and_then(|r| r.segmentation.as_ref());
                self.person_particles.update(delta_time, segmentation);
            }
            EffectType::HandInteraction => {
                // TODO: Implement hand interaction
            }
            EffectType::PaintWarp => {
                // TODO: Implement paint warp
            }
        }
    }

    /// Get particle count for UI
    pub fn particle_count(&self) -> usize {
        self.person_particles.particle_count()
    }

    /// Spawn test particles for debugging
    pub fn spawn_test_particles(&mut self, count: usize) {
        self.person_particles.spawn_test_particles(count);
        log::info!("Spawned {} test particles (total: {})", count, self.particle_count());
    }

    /// Render particles to the output texture
    fn render_particles(&self, encoder: &mut wgpu::CommandEncoder) {
        let particles = self.person_particles.particles();
        let particle_count = particles.len();

        if particle_count == 0 {
            return;
        }

        // Get texture view for particle color sampling (camera or default white)
        let texture_view = self
            .camera_texture_view
            .as_ref()
            .unwrap_or(&self.default_texture_view);

        // Upload particle data to GPU
        self.queue.write_buffer(
            &self.particle_buffer,
            0,
            bytemuck::cast_slice(particles),
        );

        // Upload params
        self.queue.write_buffer(
            &self.particle_params_buffer,
            0,
            bytemuck::bytes_of(self.person_particles.params()),
        );

        // Create bind group for this frame
        let particle_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Particle Bind Group"),
            layout: &self.particle_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.particle_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.particle_params_buffer.as_entire_binding(),
                },
            ],
        });

        // Render particles
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Particle Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.output_texture_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load, // Preserve camera image
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.particle_pipeline);
        render_pass.set_bind_group(0, &particle_bind_group, &[]);

        // Instanced rendering: 6 vertices per quad, one instance per particle
        render_pass.draw(0..6, 0..particle_count as u32);
    }

    /// Render a frame
    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // Check if we should use masked rendering (PersonParticles effect with ML)
        let use_masked = self.effect_enabled
            && matches!(self.current_effect, EffectType::PersonParticles)
            && self.camera_texture_view.is_some();

        // Upload segmentation mask if available
        let has_mask = if use_masked {
            if let Some(ml_result) = self.ml_result() {
                if let Some(ref seg) = ml_result.segmentation {
                    // Upload mask to GPU
                    self.queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &self.mask_texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        bytemuck::cast_slice(&seg.mask),
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(seg.width * 4), // R32Float = 4 bytes per pixel
                            rows_per_image: Some(seg.height),
                        },
                        wgpu::Extent3d {
                            width: seg.width,
                            height: seg.height,
                            depth_or_array_layers: 1,
                        },
                    );
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        // Clear output texture first
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.output_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        // Render camera (masked or unmasked)
        if use_masked && has_mask {
            // Use masked rendering - only show person, hide background
            if let Some(camera_texture_view) = &self.camera_texture_view {
                // Upload mask params
                let fade_person = self.person_particles.params().fade_person;
                let mask_params = MaskParams {
                    threshold: 0.5,
                    fade_amount: fade_person,
                    _pad: [0.0; 2],
                };
                self.queue.write_buffer(
                    &self.mask_params_buffer,
                    0,
                    bytemuck::bytes_of(&mask_params),
                );

                // Create masked bind group
                let masked_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Masked Bind Group"),
                    layout: &self.masked_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(camera_texture_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&self.mask_texture_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: self.mask_params_buffer.as_entire_binding(),
                        },
                    ],
                });

                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Masked Camera Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.output_texture_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                render_pass.set_pipeline(&self.masked_pipeline);
                render_pass.set_bind_group(0, &masked_bind_group, &[]);
                render_pass.draw(0..3, 0..1);
            }
        } else if let Some(camera_bind_group) = &self.passthrough_bind_group {
            // Render camera feed unmasked
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Camera to Output Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.output_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.passthrough_pipeline);
            render_pass.set_bind_group(0, camera_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        // Render particles if effect is enabled
        if self.effect_enabled && matches!(self.current_effect, EffectType::PersonParticles) {
            self.render_particles(&mut encoder);
        }

        // Render output to window (use cached bind group)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Present Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.passthrough_pipeline);
            render_pass.set_bind_group(0, &self.output_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        // Render egui UI
        self.render_ui(&mut encoder, &view);

        self.queue.submit(std::iter::once(encoder.finish()));

        // Publish to Syphon
        #[cfg(target_os = "macos")]
        self.publish_syphon();

        output.present();

        // Update FPS
        self.update_fps();

        Ok(())
    }

    fn render_ui(&mut self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let raw_input = self.egui_state.take_egui_input(&self.window);

        // Get UI state before running egui
        let current_effect = self.current_effect;
        let effect_enabled = self.effect_enabled;
        let fps = self.fps;
        let camera_connected = self.camera.is_some();
        let camera_frame_count = self.camera.as_ref().map(|c| c.frame_count()).unwrap_or(0);
        #[cfg(target_os = "macos")]
        let syphon_active = self.syphon_sharer.is_some();
        #[cfg(not(target_os = "macos"))]
        let syphon_active = false;

        // Get available cameras
        let available_cameras = crate::camera::CameraCapture::list_cameras();

        // Get ML state
        let ml_ready = self.is_ml_ready();
        let ml_initializing = self.ml_inference.is_some() && !ml_ready;
        let ml_result = self.ml_result();
        let particle_count = self.particle_count();

        // Get current particle settings
        let mut particle_shape = self.person_particles.shape();
        let mut particle_color_mode = self.person_particles.color_mode();
        let mut spawn_rate = self.person_particles.params().spawn_rate;
        let mut particle_size = self.person_particles.params().particle_size;
        let mut particle_lifetime = self.person_particles.params().particle_lifetime;
        let mut gravity_y = self.person_particles.params().gravity[1];
        let mut wind_x = self.person_particles.params().wind[0];
        let mut turbulence = self.person_particles.params().turbulence_strength;
        let mut spawn_inside = self.person_particles.params().spawn_inside != 0;
        let mut fade_person = self.person_particles.params().fade_person;
        let mut solid_color = self.person_particles.params().solid_color;
        let mut gradient_start = self.person_particles.params().gradient_start;
        let mut gradient_end = self.person_particles.params().gradient_end;

        // Run egui with a closure that doesn't borrow self
        let mut new_effect = None;
        let mut toggle_effect = false;
        let mut connect_camera_index: Option<u32> = None;
        let mut disconnect_camera = false;
        let mut init_ml = false;
        let mut spawn_test = false;
        let mut clear_particles = false;

        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Camera Effects");
                    ui.separator();
                    ui.label(format!("FPS: {:.1}", fps));
                    ui.separator();
                    if camera_connected {
                        ui.label(format!("Camera frames: {}", camera_frame_count));
                        ui.separator();
                    }
                    ui.label(format!("Effect: {:?}", current_effect));
                    if ui.button(if effect_enabled { "Disable" } else { "Enable" }).clicked() {
                        toggle_effect = true;
                    }
                });
            });

            egui::SidePanel::left("controls").show(ctx, |ui| {
                ui.heading("Camera");
                ui.separator();

                if camera_connected {
                    ui.label("Camera connected");
                    ui.label(format!("Frames: {}", camera_frame_count));
                    if ui.button("Disconnect").clicked() {
                        disconnect_camera = true;
                    }
                } else {
                    if available_cameras.is_empty() {
                        ui.label("No cameras found");
                        if ui.button("Refresh").clicked() {
                            // Will refresh on next frame
                        }
                    } else {
                        ui.label("Available cameras:");
                        for cam in &available_cameras {
                            if ui.button(format!("{}: {}", cam.index, cam.name)).clicked() {
                                connect_camera_index = Some(cam.index);
                            }
                        }
                    }
                }

                ui.separator();
                ui.heading("Effects");
                ui.separator();

                if ui.selectable_label(
                    matches!(current_effect, EffectType::PersonParticles),
                    format!("1. Person to Particles ({})", particle_count),
                ).clicked() {
                    new_effect = Some(0);
                }

                // Particle settings when PersonParticles is selected
                if matches!(current_effect, EffectType::PersonParticles) {
                    ui.horizontal(|ui| {
                        if ui.button("Test (T)").clicked() {
                            spawn_test = true;
                        }
                        if ui.button("Clear").clicked() {
                            clear_particles = true;
                        }
                    });

                    ui.separator();
                    ui.heading("Particle Settings");

                    // Shape selection
                    ui.label("Shape:");
                    ui.horizontal(|ui| {
                        if ui.selectable_label(particle_shape == ParticleShape::Circle, "Circle").clicked() {
                            particle_shape = ParticleShape::Circle;
                        }
                        if ui.selectable_label(particle_shape == ParticleShape::Square, "Square").clicked() {
                            particle_shape = ParticleShape::Square;
                        }
                        if ui.selectable_label(particle_shape == ParticleShape::Star, "Star").clicked() {
                            particle_shape = ParticleShape::Star;
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.selectable_label(particle_shape == ParticleShape::Heart, "Heart").clicked() {
                            particle_shape = ParticleShape::Heart;
                        }
                        if ui.selectable_label(particle_shape == ParticleShape::Diamond, "Diamond").clicked() {
                            particle_shape = ParticleShape::Diamond;
                        }
                    });

                    ui.add_space(4.0);

                    // Color mode selection
                    ui.label("Color Mode:");
                    ui.horizontal(|ui| {
                        if ui.selectable_label(particle_color_mode == ColorMode::Original, "Camera").clicked() {
                            particle_color_mode = ColorMode::Original;
                        }
                        if ui.selectable_label(particle_color_mode == ColorMode::Solid, "Solid").clicked() {
                            particle_color_mode = ColorMode::Solid;
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.selectable_label(particle_color_mode == ColorMode::Rainbow, "Rainbow").clicked() {
                            particle_color_mode = ColorMode::Rainbow;
                        }
                        if ui.selectable_label(particle_color_mode == ColorMode::Gradient, "Gradient").clicked() {
                            particle_color_mode = ColorMode::Gradient;
                        }
                    });

                    // Show color picker for solid color mode
                    if particle_color_mode == ColorMode::Solid {
                        ui.horizontal(|ui| {
                            ui.label("Color:");
                            let mut color = egui::Color32::from_rgba_unmultiplied(
                                (solid_color[0] * 255.0) as u8,
                                (solid_color[1] * 255.0) as u8,
                                (solid_color[2] * 255.0) as u8,
                                (solid_color[3] * 255.0) as u8,
                            );
                            if ui.color_edit_button_srgba(&mut color).changed() {
                                solid_color = [
                                    color.r() as f32 / 255.0,
                                    color.g() as f32 / 255.0,
                                    color.b() as f32 / 255.0,
                                    color.a() as f32 / 255.0,
                                ];
                            }
                        });
                    }

                    // Show gradient color pickers
                    if particle_color_mode == ColorMode::Gradient {
                        ui.horizontal(|ui| {
                            ui.label("Start:");
                            let mut start_color = egui::Color32::from_rgba_unmultiplied(
                                (gradient_start[0] * 255.0) as u8,
                                (gradient_start[1] * 255.0) as u8,
                                (gradient_start[2] * 255.0) as u8,
                                (gradient_start[3] * 255.0) as u8,
                            );
                            if ui.color_edit_button_srgba(&mut start_color).changed() {
                                gradient_start = [
                                    start_color.r() as f32 / 255.0,
                                    start_color.g() as f32 / 255.0,
                                    start_color.b() as f32 / 255.0,
                                    start_color.a() as f32 / 255.0,
                                ];
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("End:");
                            let mut end_color = egui::Color32::from_rgba_unmultiplied(
                                (gradient_end[0] * 255.0) as u8,
                                (gradient_end[1] * 255.0) as u8,
                                (gradient_end[2] * 255.0) as u8,
                                (gradient_end[3] * 255.0) as u8,
                            );
                            if ui.color_edit_button_srgba(&mut end_color).changed() {
                                gradient_end = [
                                    end_color.r() as f32 / 255.0,
                                    end_color.g() as f32 / 255.0,
                                    end_color.b() as f32 / 255.0,
                                    end_color.a() as f32 / 255.0,
                                ];
                            }
                        });
                    }

                    ui.add_space(4.0);

                    // Sliders for particle parameters
                    ui.add(egui::Slider::new(&mut spawn_rate, 100.0..=10000.0)
                        .text("Spawn Rate")
                        .logarithmic(true));

                    ui.add(egui::Slider::new(&mut particle_size, 0.005..=0.1)
                        .text("Size"));

                    ui.add(egui::Slider::new(&mut particle_lifetime, 0.5..=10.0)
                        .text("Lifetime"));

                    ui.add(egui::Slider::new(&mut gravity_y, -0.5..=0.5)
                        .text("Gravity"));

                    ui.add(egui::Slider::new(&mut wind_x, -0.2..=0.2)
                        .text("Wind"));

                    ui.add(egui::Slider::new(&mut turbulence, 0.0..=1.0)
                        .text("Turbulence"));

                    ui.add_space(4.0);
                    ui.label("Person Visibility:");
                    ui.add(egui::Slider::new(&mut fade_person, 0.0..=1.0)
                        .text("Fade"));

                    ui.checkbox(&mut spawn_inside, "Spawn inside silhouette");
                }

                ui.separator();

                if ui.selectable_label(
                    matches!(current_effect, EffectType::HandInteraction),
                    "2. Hand Interaction",
                ).clicked() {
                    new_effect = Some(1);
                }

                if ui.selectable_label(
                    matches!(current_effect, EffectType::PaintWarp),
                    "3. Paint Warp",
                ).clicked() {
                    new_effect = Some(2);
                }

                ui.separator();
                ui.heading("Output");

                #[cfg(target_os = "macos")]
                {
                    ui.label(format!("Syphon: {}", if syphon_active { "Active" } else { "Inactive" }));
                }

                #[cfg(target_os = "windows")]
                {
                    ui.label("Spout: Not implemented");
                }

                ui.separator();
                ui.heading("ML Status");
                if ml_ready {
                    ui.label("ML Ready");
                    if let Some(ref result) = ml_result {
                        if result.segmentation.is_some() {
                            ui.label(format!("Seg frame: {}", result.frame_number));
                        }
                    }
                } else if ml_initializing {
                    ui.label("Initializing...");
                } else {
                    ui.label("Not initialized");
                    if ui.button("Initialize ML").clicked() {
                        init_ml = true;
                    }
                }
            });
        });

        // Apply UI actions
        if toggle_effect {
            self.toggle_effect();
        }
        if let Some(idx) = new_effect {
            self.select_effect(idx);
        }
        if let Some(idx) = connect_camera_index {
            self.connect_camera(idx);
        }
        if disconnect_camera {
            self.disconnect_camera();
        }
        if init_ml {
            self.init_ml();
        }
        if spawn_test {
            self.spawn_test_particles(100);
        }
        if clear_particles {
            self.person_particles.clear();
        }

        // Apply particle settings
        self.person_particles.set_shape(particle_shape);
        self.person_particles.set_color_mode(particle_color_mode);
        self.person_particles.set_spawn_rate(spawn_rate);
        self.person_particles.set_particle_size(particle_size);
        self.person_particles.params_mut().particle_lifetime = particle_lifetime;
        self.person_particles.params_mut().gravity[1] = gravity_y;
        self.person_particles.params_mut().wind[0] = wind_x;
        self.person_particles.params_mut().turbulence_strength = turbulence;
        self.person_particles.set_spawn_inside(spawn_inside);
        self.person_particles.set_fade_person(fade_person);
        self.person_particles.set_solid_color(solid_color[0], solid_color[1], solid_color[2], solid_color[3]);
        self.person_particles.set_gradient(gradient_start, gradient_end);

        self.egui_state.handle_platform_output(&self.window, full_output.platform_output);

        let paint_jobs = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);

        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(&self.device, &self.queue, *id, image_delta);
        }

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            let render_pass_static: &mut wgpu::RenderPass<'static> =
                unsafe { std::mem::transmute(&mut render_pass) };

            self.egui_renderer.render(render_pass_static, &paint_jobs, &screen_descriptor);
        }

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }
    }

    #[cfg(target_os = "macos")]
    fn publish_syphon(&mut self) {
        if let (Some(sharer), Some(command_queue)) = (&self.syphon_sharer, &self.metal_command_queue) {
            unsafe {
                if let Err(e) = sharer.publish_wgpu_texture(&self.device, &self.output_texture, command_queue) {
                    log::warn!("Syphon publish error: {}", e);
                }
            }
        }
    }

    fn update_fps(&mut self) {
        self.frame_count += 1;
        self.frames_since_update += 1;

        let now = Instant::now();
        let elapsed = now.duration_since(self.last_fps_update).as_secs_f64();
        if elapsed >= 1.0 {
            self.fps = self.frames_since_update as f64 / elapsed;
            self.frames_since_update = 0;
            self.last_fps_update = now;
        }

        self.last_frame_time = now;
    }
}
