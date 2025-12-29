//! Application state holding wgpu graphics context
//!
//! This module contains the core graphics state including the wgpu device,
//! queue, surface, and configuration needed for rendering.
//!
//! Frame pacing is driven by the winit event loop (see `main.rs`), scheduling redraws
//! at `settings.target_fps` for stable pacing and low idle CPU.
//!
//! Video decoding runs on a background thread at the video's native frame rate; the
//! main thread picks up decoded frames for GPU upload without blocking.

use std::sync::Arc;
use std::time::Instant;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::compositor::{Environment, Viewport};
use crate::settings::EnvironmentSettings;
use crate::ui::MenuBar;
use crate::video::{VideoParams, VideoPlayer, VideoRenderer, VideoTexture};

/// Helper function to render egui pass
fn render_egui_pass(
    renderer: &egui_wgpu::Renderer,
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    paint_jobs: &[egui::ClippedPrimitive],
    screen_descriptor: &egui_wgpu::ScreenDescriptor,
) {
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

    // SAFETY: The render_pass is used only within this function and dropped
    // before the encoder is finished.
    let render_pass_static: &mut wgpu::RenderPass<'static> =
        unsafe { std::mem::transmute(&mut render_pass) };

    renderer.render(render_pass_static, paint_jobs, screen_descriptor);
}

/// Update FPS display every 1 second
const FPS_UPDATE_INTERVAL_SECS: f64 = 1.0;

/// Main application state holding all wgpu resources
pub struct App {
    /// Reference to the window
    window: Arc<Window>,
    /// The wgpu surface for presenting rendered frames
    surface: wgpu::Surface<'static>,
    /// The wgpu device for creating GPU resources
    device: wgpu::Device,
    /// The command queue for submitting GPU work
    queue: wgpu::Queue,
    /// Surface configuration (format, size, present mode)
    config: wgpu::SurfaceConfiguration,
    /// Current window size in physical pixels
    size: PhysicalSize<u32>,

    // Environment (fixed-resolution composition canvas)
    environment: Environment,
    
    // Viewport navigation (pan/zoom)
    viewport: Viewport,
    /// Current mouse position in window pixels
    cursor_position: (f32, f32),
    /// Last frame time for viewport animation
    last_frame_time: Instant,

    // Checkerboard background pipeline
    /// Render pipeline for checkerboard background
    checker_pipeline: wgpu::RenderPipeline,
    /// Uniform buffer for checker params (environment size)
    checker_params_buffer: wgpu::Buffer,
    /// Bind group for checker params
    checker_bind_group: wgpu::BindGroup,

    // Present pass (Environment -> WindowSurface)
    /// Bind group layout for presenting the environment to the window
    copy_bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group for presenting the environment to the window
    copy_bind_group: wgpu::BindGroup,
    /// Render pipeline for presenting the environment to the window
    copy_pipeline: wgpu::RenderPipeline,
    /// Sampler for presenting the environment texture
    sampler: wgpu::Sampler,
    /// Uniform buffer for present params (scale/offset)
    copy_params_buffer: wgpu::Buffer,

    // Frame timing
    /// UI frame count (for stats only)
    ui_frame_count: u64,
    /// Last time UI FPS was updated (once per second)
    last_ui_fps_update: Instant,
    /// UI frames since last update (for once-per-second FPS calculation)
    ui_frames_since_update: u64,
    /// UI FPS (frames per second, updated once per second)
    ui_fps: f64,

    // egui integration
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,

    // UI state
    pub menu_bar: MenuBar,

    // Settings
    pub settings: EnvironmentSettings,
    pub current_file: Option<std::path::PathBuf>,

    // Video playback (background-threaded)
    /// Video renderer for displaying video frames
    video_renderer: VideoRenderer,
    /// Background-threaded video player
    video_player: Option<VideoPlayer>,
    /// GPU texture for video frames
    video_texture: Option<VideoTexture>,
    /// Bind group for video rendering
    video_bind_group: Option<wgpu::BindGroup>,
}

impl App {
    /// Create a new App instance with initialized wgpu context
    pub async fn new(window: Arc<Window>, settings: EnvironmentSettings) -> Self {
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
                    label: Some("Immersive Server Device"),
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

        // Always use Fifo for presentation - we control frame rate ourselves
        // Fifo just queues frames, doesn't block rendering
        // Always use Immediate mode for manual FPS control
        // Fall back to Mailbox or Fifo if Immediate is not available
        let present_mode = if surface_caps.present_modes.contains(&wgpu::PresentMode::Immediate) {
            wgpu::PresentMode::Immediate
        } else if surface_caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
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
            desired_maximum_frame_latency: 1, // Minimize latency
        };

        surface.configure(&device, &config);

        // Create Environment (fixed-resolution composition canvas)
        let env_width = settings.environment_width.max(1);
        let env_height = settings.environment_height.max(1);
        let environment = Environment::new(&device, env_width, env_height, surface_format);

        // Create checkerboard background pipeline
        let (checker_pipeline, checker_params_buffer, checker_bind_group) =
            Self::create_checker_pipeline(&device, &queue, surface_format, env_width, env_height);

        // Create present pipeline (Environment -> WindowSurface)
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Copy Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let copy_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Copy Bind Group Layout"),
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

        let copy_params = VideoParams::fit_aspect_ratio(env_width, env_height, size.width, size.height);
        let copy_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Copy Params Buffer"),
            size: std::mem::size_of::<VideoParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&copy_params_buffer, 0, bytemuck::bytes_of(&copy_params));

        let copy_bind_group = Self::create_copy_bind_group(
            &device,
            &copy_bind_group_layout,
            environment.texture_view(),
            &sampler,
            &copy_params_buffer,
        );

        let copy_pipeline =
            Self::create_copy_pipeline(&device, &copy_bind_group_layout, surface_format);

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
        let menu_bar = MenuBar::new(&settings);

        // Initialize video renderer
        let video_renderer = VideoRenderer::new(&device, surface_format);

        let now = Instant::now();
        let initial_target_fps = settings.target_fps as f64;

        Self {
            window,
            surface,
            device,
            queue,
            config,
            size,
            environment,
            viewport: Viewport::new(),
            cursor_position: (0.0, 0.0),
            last_frame_time: now,
            checker_pipeline,
            checker_params_buffer,
            checker_bind_group,
            copy_bind_group_layout,
            copy_bind_group,
            copy_pipeline,
            sampler,
            copy_params_buffer,
            ui_frame_count: 0,
            last_ui_fps_update: now,
            ui_frames_since_update: 0,
            ui_fps: initial_target_fps, // Initialize to target so display isn't 0
            egui_ctx,
            egui_state,
            egui_renderer,
            menu_bar,
            settings,
            current_file: None,
            video_renderer,
            video_player: None,
            video_texture: None,
            video_bind_group: None,
        }
    }

    fn create_copy_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        texture_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
        params_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Copy Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        })
    }

    fn create_copy_pipeline(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Copy Shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
                struct VertexOutput {
                    @builtin(position) position: vec4<f32>,
                    @location(0) uv: vec2<f32>,
                }

                struct PresentParams {
                    scale: vec2<f32>,
                    offset: vec2<f32>,
                    opacity: f32,
                    _pad1: f32,
                    _pad2: f32,
                    _pad3: f32,
                }

                @vertex
                fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
                    var out: VertexOutput;
                    // Full screen triangle
                    let x = f32(i32(vertex_index & 1u) * 4 - 1);
                    let y = f32(i32(vertex_index >> 1u) * 4 - 1);
                    out.position = vec4<f32>(x, y, 0.0, 1.0);
                    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
                    return out;
                }

                @group(0) @binding(0) var t_texture: texture_2d<f32>;
                @group(0) @binding(1) var s_sampler: sampler;
                @group(0) @binding(2) var<uniform> params: PresentParams;

                @fragment
                fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
                    // Preserve environment aspect ratio when scaling into the window.
                    let adjusted_uv = (in.uv - 0.5) / params.scale + 0.5 + params.offset;

                    if (adjusted_uv.x < 0.0 || adjusted_uv.x > 1.0 || adjusted_uv.y < 0.0 || adjusted_uv.y > 1.0) {
                        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
                    }

                    let color = textureSample(t_texture, s_sampler, adjusted_uv);
                    return vec4<f32>(color.rgb, 1.0);
                }
                "#
                .into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Copy Pipeline Layout"),
            bind_group_layouts: &[bind_group_layout],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Copy Pipeline"),
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
                    format,
                    blend: None,
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
        })
    }

    /// Create checkerboard background pipeline
    fn create_checker_pipeline(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        env_width: u32,
        env_height: u32,
    ) -> (wgpu::RenderPipeline, wgpu::Buffer, wgpu::BindGroup) {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Checker Shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
                struct VertexOutput {
                    @builtin(position) position: vec4<f32>,
                    @location(0) uv: vec2<f32>,
                }

                struct CheckerParams {
                    env_size: vec2<f32>,
                    checker_size: f32,
                    _pad: f32,
                }

                @group(0) @binding(0) var<uniform> params: CheckerParams;

                @vertex
                fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
                    var out: VertexOutput;
                    let x = f32(i32(vertex_index & 1u) * 4 - 1);
                    let y = f32(i32(vertex_index >> 1u) * 4 - 1);
                    out.position = vec4<f32>(x, y, 0.0, 1.0);
                    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
                    return out;
                }

                @fragment
                fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
                    // Convert UV to pixel coordinates
                    let pixel = in.uv * params.env_size;
                    
                    // Calculate checker pattern
                    let checker_x = floor(pixel.x / params.checker_size);
                    let checker_y = floor(pixel.y / params.checker_size);
                    let is_light = (i32(checker_x) + i32(checker_y)) % 2 == 0;
                    
                    // Use subtle gray tones like Photoshop
                    let light_gray = vec3<f32>(0.35, 0.35, 0.35);
                    let dark_gray = vec3<f32>(0.25, 0.25, 0.25);
                    
                    let color = select(dark_gray, light_gray, is_light);
                    return vec4<f32>(color, 1.0);
                }
                "#
                .into(),
            ),
        });

        // Checker params: env_size (vec2), checker_size (f32), padding (f32)
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct CheckerParams {
            env_size: [f32; 2],
            checker_size: f32,
            _pad: f32,
        }

        let params = CheckerParams {
            env_size: [env_width as f32, env_height as f32],
            checker_size: 16.0, // 16 pixel checkers
            _pad: 0.0,
        };

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Checker Params Buffer"),
            size: std::mem::size_of::<CheckerParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        queue.write_buffer(&params_buffer, 0, bytemuck::bytes_of(&params));

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Checker Bind Group Layout"),
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Checker Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: params_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Checker Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Checker Pipeline"),
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
                    format,
                    blend: None,
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

        (pipeline, params_buffer, bind_group)
    }

    /// Update checkerboard params when environment size changes
    fn update_checker_params(&self) {
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct CheckerParams {
            env_size: [f32; 2],
            checker_size: f32,
            _pad: f32,
        }

        let params = CheckerParams {
            env_size: [self.environment.width() as f32, self.environment.height() as f32],
            checker_size: 16.0,
            _pad: 0.0,
        };

        self.queue
            .write_buffer(&self.checker_params_buffer, 0, bytemuck::bytes_of(&params));
    }

    /// Handle window resize events
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);

            // Update present params (Environment -> WindowSurface)
            let params = VideoParams::fit_aspect_ratio(
                self.environment.width(),
                self.environment.height(),
                new_size.width,
                new_size.height,
            );
            self.queue
                .write_buffer(&self.copy_params_buffer, 0, bytemuck::bytes_of(&params));

            log::debug!("Resized to {}x{}", new_size.width, new_size.height);
        }
    }

    /// Start a new frame (currently a no-op; redraw pacing is handled in `main.rs`)
    pub fn begin_frame(&mut self) {
        // Redraw pacing is handled by the winit event loop in `main.rs`.
    }

    /// Update frame timing statistics (once per second)
    fn update_frame_stats(&mut self) {
        self.ui_frame_count += 1;
        self.ui_frames_since_update += 1;

        let now = Instant::now();

        // Update FPS once per second
        let elapsed = now.duration_since(self.last_ui_fps_update).as_secs_f64();
        if elapsed >= FPS_UPDATE_INTERVAL_SECS {
            // Calculate UI FPS (frames per second over the interval)
            self.ui_fps = self.ui_frames_since_update as f64 / elapsed;
            
            // Reset counters
            self.last_ui_fps_update = now;
            self.ui_frames_since_update = 0;
        }
    }
    
    /// Handle winit window events for egui
    pub fn handle_window_event(&mut self, event: &winit::event::WindowEvent) -> bool {
        let response = self.egui_state.on_window_event(&self.window, event);
        response.consumed
    }

    fn update_present_params(&mut self) {
        let window_size = (self.size.width as f32, self.size.height as f32);
        let env_size = (self.environment.width() as f32, self.environment.height() as f32);
        
        let (scale_x, scale_y, offset_x, offset_y) = self.viewport.get_shader_params(window_size, env_size);
        
        let params = VideoParams {
            scale: [scale_x, scale_y],
            offset: [offset_x, offset_y],
            opacity: 1.0,
            _padding: [0.0; 3],
        };
        self.queue
            .write_buffer(&self.copy_params_buffer, 0, bytemuck::bytes_of(&params));
    }

    fn update_video_params_for_environment(&mut self) {
        let Some(player) = &self.video_player else {
            return;
        };

        let params = VideoParams::native_size(
            player.width(),
            player.height(),
            self.environment.width(),
            self.environment.height(),
        );
        self.video_renderer.set_params(&self.queue, params);
    }

    fn sync_environment_from_settings(&mut self) {
        let desired_width = self.settings.environment_width.max(1);
        let desired_height = self.settings.environment_height.max(1);

        if desired_width == self.environment.width() && desired_height == self.environment.height() {
            return;
        }

        self.environment
            .resize(&self.device, desired_width, desired_height);

        // Environment texture view changed, so recreate present bind group.
        self.copy_bind_group = Self::create_copy_bind_group(
            &self.device,
            &self.copy_bind_group_layout,
            self.environment.texture_view(),
            &self.sampler,
            &self.copy_params_buffer,
        );

        self.update_present_params();
        self.update_checker_params();
        self.update_video_params_for_environment();
    }

    /// Render a frame with egui UI
    pub fn render(&mut self) -> Result<bool, wgpu::SurfaceError> {
        // Begin egui frame
        let raw_input = self.egui_state.take_egui_input(&self.window);
        self.egui_ctx.begin_pass(raw_input);

        // Get FPS to display (updated once per second)
        let display_fps = self.ui_fps;
        let display_frame_time_ms = if display_fps > 0.0 {
            1000.0 / display_fps
        } else {
            0.0
        };

        // Render menu bar with appropriate FPS
        let settings_changed = self.menu_bar.render(
            &self.egui_ctx,
            &mut self.settings,
            &self.current_file,
            display_fps,
            display_frame_time_ms,
        );

        // Apply environment resolution changes (if any) before rendering.
        self.sync_environment_from_settings();

        let full_output = self.egui_ctx.end_pass();

        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);

        let paint_jobs = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        // Create command encoder
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Render to Environment texture (fixed-resolution canvas)
        // 1. Always render checkerboard background first
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Checker Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.environment.texture_view(),
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
            render_pass.set_pipeline(&self.checker_pipeline);
            render_pass.set_bind_group(0, &self.checker_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        // 2. Render video on top (with alpha blending, no clear)
        if let Some(bind_group) = &self.video_bind_group {
            self.video_renderer
                .render(&mut encoder, self.environment.texture_view(), bind_group, false);
        }

        // Update egui textures
        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, *id, image_delta);
        }

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        // Present Environment to the window surface
        let output = self.surface.get_current_texture()?;
        let surface_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Present Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_view,
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

            render_pass.set_pipeline(&self.copy_pipeline);
            render_pass.set_bind_group(0, &self.copy_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        // Render egui on top of the surface
        render_egui_pass(
            &self.egui_renderer,
            &mut encoder,
            &surface_view,
            &paint_jobs,
            &screen_descriptor,
        );

        // Free egui textures
        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(settings_changed)
    }

    /// Apply frame rate limiting - call after render()
    /// Uses pure sleep for responsive UI (accepts ~1% variance)
    pub fn end_frame(&mut self) {
        // Update UI timing stats.
        self.update_frame_stats();
    }

    // Getters
    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    pub fn fps(&self) -> f64 {
        self.ui_fps
    }

    pub fn frame_time_ms(&self) -> f64 {
        let fps = self.fps();
        if fps > 0.0 { 1000.0 / fps } else { 0.0 }
    }

    pub fn frame_count(&self) -> u64 {
        self.ui_frame_count
    }

    pub fn egui_wants_keyboard(&self) -> bool {
        self.egui_ctx.wants_keyboard_input()
    }

    pub fn egui_wants_pointer(&self) -> bool {
        self.egui_ctx.wants_pointer_input()
    }

    pub fn target_fps(&self) -> u32 {
        self.settings.target_fps
    }

    pub fn cursor_position(&self) -> (f32, f32) {
        self.cursor_position
    }

    // Video playback methods (background-threaded)

    /// Load a video file for playback (starts background decode thread)
    pub fn load_video(&mut self, path: &std::path::Path) -> Result<(), String> {
        // Open video player (starts background decode thread)
        let player = VideoPlayer::open(path)
            .map_err(|e| format!("Failed to open video: {}", e))?;

        log::info!(
            "Loaded video: {}x{} @ {:.2}fps, duration: {:.2}s",
            player.width(),
            player.height(),
            player.frame_rate(),
            player.duration()
        );

        // Create video texture
        let video_texture = VideoTexture::new(&self.device, player.width(), player.height());

        // Set up native size params (videos spill over if larger than environment)
        let params = VideoParams::native_size(
            player.width(),
            player.height(),
            self.environment.width(),
            self.environment.height(),
        );
        self.video_renderer.set_params(&self.queue, params);

        // Create bind group
        let video_bind_group = self.video_renderer.create_bind_group(&self.device, &video_texture);

        // Store player and texture
        self.video_player = Some(player);
        self.video_texture = Some(video_texture);
        self.video_bind_group = Some(video_bind_group);

        Ok(())
    }

    /// Update video playback - pick up decoded frames (non-blocking)
    pub fn update_video(&mut self) {
        let Some(player) = &self.video_player else {
            return;
        };
        let Some(texture) = &self.video_texture else {
            return;
        };

        // Pick up any new frame from background thread (non-blocking)
        if let Some(frame) = player.take_frame() {
            texture.upload(&self.queue, &frame);
        }
    }

    /// Toggle video pause state
    pub fn toggle_video_pause(&mut self) {
        if let Some(player) = &self.video_player {
            player.toggle_pause();
        }
    }

    /// Restart video from beginning
    pub fn restart_video(&mut self) {
        if let Some(player) = &self.video_player {
            player.restart();
        }
    }

    /// Check if video is currently loaded
    pub fn has_video(&self) -> bool {
        self.video_player.is_some()
    }

    /// Check if video is paused
    pub fn is_video_paused(&self) -> bool {
        self.video_player.as_ref().map(|p| p.is_paused()).unwrap_or(false)
    }
    
    /// Get the current video path if loaded
    pub fn current_video_path(&self) -> Option<&std::path::Path> {
        self.video_player.as_ref().map(|p| p.path())
    }

    // Viewport navigation methods

    /// Handle right mouse button press for panning
    /// Returns true if viewport was reset (double-click)
    pub fn on_right_mouse_down(&mut self, x: f32, y: f32) -> bool {
        let reset = self.viewport.on_right_mouse_down((x, y));
        if reset {
            self.update_present_params();
        }
        reset
    }

    /// Handle right mouse button release
    pub fn on_right_mouse_up(&mut self) {
        self.viewport.on_right_mouse_up();
    }

    /// Handle mouse movement
    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        self.cursor_position = (x, y);
        let window_size = (self.size.width as f32, self.size.height as f32);
        let env_size = (self.environment.width() as f32, self.environment.height() as f32);
        self.viewport.on_mouse_move((x, y), window_size, env_size);
        self.update_present_params();
    }

    /// Handle scroll wheel for zooming
    pub fn on_scroll(&mut self, delta: f32) {
        let window_size = (self.size.width as f32, self.size.height as f32);
        let env_size = (self.environment.width() as f32, self.environment.height() as f32);
        self.viewport.on_scroll(delta, self.cursor_position, window_size, env_size);
        self.update_present_params();
    }

    /// Handle keyboard zoom (+/- keys)
    pub fn on_keyboard_zoom(&mut self, zoom_in: bool) {
        let window_size = (self.size.width as f32, self.size.height as f32);
        let env_size = (self.environment.width() as f32, self.environment.height() as f32);
        self.viewport.on_keyboard_zoom(zoom_in, window_size, env_size);
        self.update_present_params();
    }

    /// Reset viewport to fit-to-window
    pub fn reset_viewport(&mut self) {
        self.viewport.reset();
        self.update_present_params();
    }

    /// Update viewport animation (rubber-band snap-back)
    pub fn update_viewport(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;

        if self.viewport.needs_update() {
            let window_size = (self.size.width as f32, self.size.height as f32);
            let env_size = (self.environment.width() as f32, self.environment.height() as f32);
            self.viewport.update(dt, window_size, env_size);
            self.update_present_params();
        }
    }

    /// Get current zoom level (for UI display)
    pub fn viewport_zoom(&self) -> f32 {
        self.viewport.zoom()
    }
}
