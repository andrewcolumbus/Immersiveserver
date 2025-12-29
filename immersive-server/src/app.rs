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

    // Offscreen rendering (for display-independent frame rate)
    /// Offscreen render texture
    offscreen_texture: wgpu::Texture,
    /// View of the offscreen texture
    offscreen_view: wgpu::TextureView,
    /// Bind group layout for copying texture to screen
    copy_bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group for copying texture to screen
    copy_bind_group: wgpu::BindGroup,
    /// Render pipeline for copying to screen
    copy_pipeline: wgpu::RenderPipeline,
    /// Sampler for texture copy
    sampler: wgpu::Sampler,

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

        // Create offscreen render texture
        let (offscreen_texture, offscreen_view) =
            Self::create_offscreen_texture(&device, surface_format, size.width, size.height);

        // Create texture copy pipeline
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
                ],
            });

        let copy_bind_group = Self::create_copy_bind_group(
            &device,
            &copy_bind_group_layout,
            &offscreen_view,
            &sampler,
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
            offscreen_texture,
            offscreen_view,
            copy_bind_group_layout,
            copy_bind_group,
            copy_pipeline,
            sampler,
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

    fn create_offscreen_texture(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Offscreen Texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn create_copy_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        texture_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
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

                @fragment
                fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
                    return textureSample(t_texture, s_sampler, in.uv);
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

    /// Handle window resize events
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);

            // Recreate offscreen texture
            let (texture, view) = Self::create_offscreen_texture(
                &self.device,
                self.config.format,
                new_size.width,
                new_size.height,
            );
            self.offscreen_texture = texture;
            self.offscreen_view = view;

            // Recreate bind group with new texture view
            self.copy_bind_group = Self::create_copy_bind_group(
                &self.device,
                &self.copy_bind_group_layout,
                &self.offscreen_view,
                &self.sampler,
            );

            // Update video aspect ratio if video is loaded
            if let Some(player) = &self.video_player {
                let params = VideoParams::fit_aspect_ratio(
                    player.width(),
                    player.height(),
                    new_size.width,
                    new_size.height,
                );
                self.video_renderer.set_params(&self.queue, params);
            }

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

        // Render to OFFSCREEN texture (not surface) - this is key for decoupling
        // First, render video if loaded, otherwise clear
        if let Some(bind_group) = &self.video_bind_group {
            // Render video fullscreen
            self.video_renderer.render(&mut encoder, &self.offscreen_view, bind_group, true);
        } else {
            // No video, just clear
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.offscreen_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.02,
                            g: 0.02,
                            b: 0.04,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
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

        // Render egui to offscreen texture
        render_egui_pass(
            &self.egui_renderer,
            &mut encoder,
            &self.offscreen_view,
            &paint_jobs,
            &screen_descriptor,
        );

        // Free egui textures
        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        // Now copy offscreen texture to screen
        let output = self.surface.get_current_texture()?;
        let surface_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Copy to Screen Pass"),
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

        // Set up aspect ratio preserving params
        let params = VideoParams::fit_aspect_ratio(
            player.width(),
            player.height(),
            self.size.width,
            self.size.height,
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
}
