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

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::compositor::{Environment, LayerSource, Viewport};
use crate::layer_runtime::LayerRuntime;
use crate::settings::EnvironmentSettings;
use crate::ui::MenuBar;
use crate::video::{LayerParams, VideoParams, VideoPlayer, VideoRenderer, VideoTexture};

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
    /// Whether BC texture compression is supported (for HAP/DXV)
    bc_texture_supported: bool,

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
    /// Clip grid panel for triggering clips
    pub clip_grid_panel: crate::ui::ClipGridPanel,
    /// Docking manager for detachable/resizable panels
    pub dock_manager: crate::ui::DockManager,
    /// Properties panel (Environment/Layer/Clip tabs)
    pub properties_panel: crate::ui::PropertiesPanel,
    /// Sources panel for drag-and-drop of OMT/NDI sources
    pub sources_panel: crate::ui::SourcesPanel,
    /// Effects browser panel for drag-and-drop of effects
    pub effects_browser_panel: crate::ui::EffectsBrowserPanel,
    /// File browser panel for drag-and-drop of video files
    pub file_browser_panel: crate::ui::FileBrowserPanel,
    /// Preview monitor panel for previewing clips before triggering
    pub preview_monitor_panel: crate::ui::PreviewMonitorPanel,
    /// Preview player for clip preview playback
    preview_player: crate::preview_player::PreviewPlayer,
    /// Thumbnail cache for video previews in clip grid
    pub thumbnail_cache: crate::ui::ThumbnailCache,
    /// HAP Converter window
    pub converter_window: crate::converter::ConverterWindow,

    // Settings
    pub settings: EnvironmentSettings,
    pub current_file: Option<std::path::PathBuf>,

    // Layer rendering
    /// Video renderer for displaying video frames (shared across all layers)
    video_renderer: VideoRenderer,
    /// Runtime state for each layer (GPU resources, video players)
    /// Key is layer ID, matching Environment.layers[].id
    layer_runtimes: HashMap<u32, LayerRuntime>,
    /// Pending runtimes being loaded (waiting for first frame before swap)
    /// When a new clip is loaded, it goes here until has_frame=true, then swaps in
    pending_runtimes: HashMap<u32, LayerRuntime>,
    /// Pending transitions for layers (stored when clip is triggered, applied when ready)
    pending_transition: HashMap<u32, crate::compositor::ClipTransition>,
    /// Last layer ID that had a texture uploaded (for round-robin rate limiting)
    last_upload_layer: u32,

    // Shader hot-reload
    /// Watches shader files for changes and triggers recompilation
    shader_watcher: Option<crate::shaders::ShaderWatcher>,

    // OMT (Open Media Transport) networking
    /// OMT source discovery service
    omt_discovery: Option<crate::network::SourceDiscovery>,
    /// OMT sender for broadcasting the environment
    omt_sender: Option<crate::network::OmtSender>,
    /// OMT capture for GPU texture readback
    omt_capture: Option<crate::network::OmtCapture>,
    /// Whether OMT sender is enabled (broadcasts environment)
    omt_broadcast_enabled: bool,
    /// Tokio runtime for async OMT operations
    tokio_runtime: Option<tokio::runtime::Runtime>,
    /// Pending OMT sender from background start (received when ready)
    pending_omt_sender: Option<std::sync::mpsc::Receiver<Result<crate::network::OmtSender, String>>>,

    // Syphon/Spout texture sharing
    /// Texture sharer for Syphon (macOS) or Spout (Windows)
    #[cfg(target_os = "macos")]
    texture_sharer: Option<crate::network::SyphonSharer>,
    #[cfg(target_os = "windows")]
    spout_capture: Option<crate::network::SpoutCapture>,
    /// Whether texture sharing is enabled
    texture_share_enabled: bool,
    /// Metal command queue for Syphon (macOS only)
    #[cfg(target_os = "macos")]
    metal_command_queue: Option<metal::CommandQueue>,

    // Effects system
    /// Effect manager for processing effects on layers and environment
    effect_manager: crate::effects::EffectManager,
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

        // Request BC texture compression for GPU-native codecs (HAP/DXV)
        let bc_texture_supported = adapter.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let mut required_features = wgpu::Features::empty();
        if bc_texture_supported {
            required_features |= wgpu::Features::TEXTURE_COMPRESSION_BC;
            log::info!("BC texture compression enabled (for HAP/DXV support)");
        } else {
            log::warn!("BC texture compression not available - HAP/DXV will use software decode");
        }
        
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Immersive Server Device"),
                    required_features,
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
        let mut environment = Environment::new(&device, env_width, env_height, surface_format);

        // Add default layers if none exist (so clip grid is immediately usable)
        if environment.layer_count() == 0 {
            let clip_count = settings.global_clip_count;
            for i in 1..=4 {
                let mut layer = crate::compositor::Layer::new(i, format!("Layer {}", i));
                layer.clips = vec![None; clip_count];
                environment.add_existing_layer(layer);
            }
            log::info!("Created 4 default layers with {} clip slots each", clip_count);
        }

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

        // Initialize shader hot-reload watcher
        let shader_watcher = match crate::shaders::ShaderWatcher::new() {
            Ok(watcher) => Some(watcher),
            Err(e) => {
                log::warn!("Failed to initialize shader watcher: {:?}", e);
                None
            }
        };

        let now = Instant::now();
        let initial_target_fps = settings.target_fps as f64;
        let texture_share_enabled = settings.texture_share_enabled;

        Self {
            window,
            surface,
            device,
            queue,
            config,
            size,
            bc_texture_supported,
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
            clip_grid_panel: crate::ui::ClipGridPanel::new(),
            dock_manager: {
                let mut dm = crate::ui::DockManager::new();
                // Register the standard panels with their default dock zones
                dm.register_panel(crate::ui::DockablePanel::new(
                    crate::ui::dock::panel_ids::CLIP_GRID,
                    "Clip Grid",
                    crate::ui::DockZone::Right,
                ));
                dm.register_panel(crate::ui::DockablePanel::new(
                    crate::ui::dock::panel_ids::PROPERTIES,
                    "Properties",
                    crate::ui::DockZone::Left,
                ));
                dm.register_panel(crate::ui::DockablePanel::new(
                    crate::ui::dock::panel_ids::SOURCES,
                    "Sources",
                    crate::ui::DockZone::Left,
                ));
                dm.register_panel(crate::ui::DockablePanel::new(
                    crate::ui::dock::panel_ids::EFFECTS_BROWSER,
                    "Effects",
                    crate::ui::DockZone::Left,
                ));
                dm.register_panel(crate::ui::DockablePanel::new(
                    crate::ui::dock::panel_ids::FILES,
                    "Files",
                    crate::ui::DockZone::Left,
                ));
                dm.register_panel(crate::ui::DockablePanel::new(
                    crate::ui::dock::panel_ids::PREVIEW_MONITOR,
                    "Preview Monitor",
                    crate::ui::DockZone::Floating,
                ));
                dm
            },
            properties_panel: crate::ui::PropertiesPanel::new(),
            sources_panel: crate::ui::SourcesPanel::new(),
            effects_browser_panel: crate::ui::EffectsBrowserPanel::new(),
            file_browser_panel: crate::ui::FileBrowserPanel::new(),
            preview_monitor_panel: crate::ui::PreviewMonitorPanel::new(),
            preview_player: crate::preview_player::PreviewPlayer::new(bc_texture_supported),
            thumbnail_cache: crate::ui::ThumbnailCache::new(),
            converter_window: crate::converter::ConverterWindow::new(),
            settings,
            current_file: None,
            video_renderer,
            layer_runtimes: HashMap::new(),
            pending_runtimes: HashMap::new(),
            pending_transition: HashMap::new(),
            last_upload_layer: 0,
            shader_watcher,

            // Initialize OMT networking
            omt_discovery: Self::create_omt_discovery(),
            omt_sender: None,
            omt_capture: None,
            omt_broadcast_enabled: false, // Disabled by default - enable via menu
            tokio_runtime: Self::create_tokio_runtime(),
            pending_omt_sender: None,

            // Syphon/Spout texture sharing
            #[cfg(target_os = "macos")]
            texture_sharer: None,
            #[cfg(target_os = "windows")]
            spout_capture: None,
            texture_share_enabled,
            #[cfg(target_os = "macos")]
            metal_command_queue: None,

            // Effects
            effect_manager: crate::effects::EffectManager::new(),
        }
    }

    /// Create the Tokio runtime for async OMT operations
    fn create_tokio_runtime() -> Option<tokio::runtime::Runtime> {
        match tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
        {
            Ok(rt) => {
                log::info!("OMT: Tokio runtime initialized");
                Some(rt)
            }
            Err(e) => {
                log::warn!("OMT: Failed to create Tokio runtime: {}", e);
                None
            }
        }
    }

    /// Create the OMT discovery service
    fn create_omt_discovery() -> Option<crate::network::SourceDiscovery> {
        match crate::network::SourceDiscovery::new() {
            Ok(mut discovery) => {
                // Start browsing for sources
                if let Err(e) = discovery.start_browsing() {
                    log::warn!("OMT: Failed to start source discovery: {}", e);
                } else {
                    log::info!("OMT: Source discovery started");
                }
                Some(discovery)
            }
            Err(e) => {
                log::warn!("OMT: Discovery service unavailable: {}", e);
                None
            }
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

    fn update_layer_params_for_environment(&mut self) {
        // When environment resizes, we need to update layer params
        // This is handled per-layer during rendering now
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

        // Resize OMT capture buffers if broadcasting
        if let Some(capture) = &mut self.omt_capture {
            capture.resize(&self.device, desired_width, desired_height);
        }

        self.update_present_params();
        self.update_checker_params();
        self.update_layer_params_for_environment();
    }

    /// Sync layers and effects from environment to settings (for saving)
    pub fn sync_layers_to_settings(&mut self) {
        let layers: Vec<_> = self.environment.layers().to_vec();
        self.settings.set_layers(&layers);
        // Also sync environment effects
        self.settings.effects = self.environment.effects().clone();
    }

    /// Restore layers from settings (after loading)
    pub fn restore_layers_from_settings(&mut self) {
        // Clear existing layers
        self.environment.clear_layers();
        self.layer_runtimes.clear();

        // Add layers from settings
        for mut layer in self.settings.layers.clone() {
            let layer_id = layer.id;
            let active_clip = layer.active_clip;

            // Clean up invalid clips (empty paths from deserialization)
            for clip_slot in layer.clips.iter_mut() {
                if let Some(cell) = clip_slot {
                    if !cell.is_valid() {
                        *clip_slot = None;
                    }
                }
            }

            // Get valid clips for checking active clip
            let clips = layer.clips.clone();

            // Add the layer to the environment
            self.environment.add_existing_layer(layer);

            // If the layer has an active clip, try to load it (only if valid)
            if let Some(slot) = active_clip {
                if let Some(Some(cell)) = clips.get(slot) {
                    if cell.is_valid() {
                        let path = cell.source_path.clone();
                        
                        // Try to load the video (errors are logged but don't stop restore)
                        if let Err(e) = self.load_layer_video(layer_id, &path) {
                            log::warn!("Failed to restore clip for layer {}: {}", layer_id, e);
                        }
                    }
                }
            }
        }

        // If no layers were restored, create 4 default layers
        if self.environment.layer_count() == 0 {
            let clip_count = self.settings.global_clip_count;
            for i in 1..=4 {
                let mut layer = crate::compositor::Layer::new(i, format!("Layer {}", i));
                layer.clips = vec![None; clip_count];
                self.environment.add_existing_layer(layer);
            }
            log::info!("No saved layers, created 4 default layers with {} clip slots each", clip_count);
        } else {
            log::info!("Restored {} layers from settings", self.environment.layer_count());
        }

        // Restore environment effects from settings
        *self.environment.effects_mut() = self.settings.effects.clone();
        if !self.settings.effects.is_empty() {
            log::info!("Restored {} master effects from settings", self.settings.effects.len());
        }
    }

    /// Sync OMT broadcast state from settings (after loading)
    pub fn sync_omt_broadcast_from_settings(&mut self) {
        let should_broadcast = self.settings.omt_broadcast_enabled;
        let is_broadcasting = self.is_omt_broadcasting();

        if should_broadcast && !is_broadcasting {
            self.omt_broadcast_enabled = true;
            self.start_omt_broadcast("Immersive Server", 5970);
        } else if !should_broadcast && is_broadcasting {
            self.omt_broadcast_enabled = false;
            self.stop_omt_broadcast();
        } else {
            self.omt_broadcast_enabled = should_broadcast;
        }
    }

    /// Render a frame with egui UI
    pub fn render(&mut self) -> Result<bool, wgpu::SurfaceError> {
        // Update effect manager timing (BPM clock, frame time)
        self.effect_manager.update();

        // Poll for shader hot-reload (no-op in release builds)
        self.poll_shader_reload();

        // Poll for pending OMT sender start
        self.poll_pending_omt_sender();

        // Poll for completed thumbnail generations
        self.thumbnail_cache.poll(&self.egui_ctx);

        // Sync thumbnail mode from settings (clears cache if changed)
        self.thumbnail_cache.set_mode(self.settings.thumbnail_mode);

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

        // Build panel states for View menu
        let panel_states: Vec<(&str, &str, bool)> = vec![
            (
                crate::ui::dock::panel_ids::PROPERTIES,
                "Properties",
                self.dock_manager.get_panel(crate::ui::dock::panel_ids::PROPERTIES)
                    .map(|p| p.open).unwrap_or(false),
            ),
            (
                crate::ui::dock::panel_ids::CLIP_GRID,
                "Clip Grid",
                self.dock_manager.get_panel(crate::ui::dock::panel_ids::CLIP_GRID)
                    .map(|p| p.open).unwrap_or(false),
            ),
            (
                crate::ui::dock::panel_ids::SOURCES,
                "Sources",
                self.dock_manager.get_panel(crate::ui::dock::panel_ids::SOURCES)
                    .map(|p| p.open).unwrap_or(false),
            ),
            (
                crate::ui::dock::panel_ids::EFFECTS_BROWSER,
                "Effects",
                self.dock_manager.get_panel(crate::ui::dock::panel_ids::EFFECTS_BROWSER)
                    .map(|p| p.open).unwrap_or(false),
            ),
            (
                crate::ui::dock::panel_ids::FILES,
                "Files",
                self.dock_manager.get_panel(crate::ui::dock::panel_ids::FILES)
                    .map(|p| p.open).unwrap_or(false),
            ),
            (
                crate::ui::dock::panel_ids::PREVIEW_MONITOR,
                "Preview Monitor",
                self.dock_manager.get_panel(crate::ui::dock::panel_ids::PREVIEW_MONITOR)
                    .map(|p| p.open).unwrap_or(false),
            ),
        ];

        // Render menu bar with appropriate FPS
        let settings_changed = self.menu_bar.render(
            &self.egui_ctx,
            &mut self.settings,
            &self.current_file,
            display_fps,
            display_frame_time_ms,
            &panel_states,
        );

        // Handle menu actions (e.g., toggle panel)
        if let Some(action) = self.menu_bar.take_menu_action() {
            match action {
                crate::ui::menu_bar::MenuAction::TogglePanel { panel_id } => {
                    self.dock_manager.toggle_panel(&panel_id);
                }
                crate::ui::menu_bar::MenuAction::OpenHAPConverter => {
                    self.converter_window.open();
                }
            }
        }

        // Render dock zones overlay during drag operations
        self.dock_manager.render_dock_zones(&self.egui_ctx);
        
        // Render HAP Converter window
        self.converter_window.show(&self.egui_ctx);
        
        // Get layer list for property panels
        let layers: Vec<_> = self.environment.layers().to_vec();
        let omt_broadcasting = self.is_omt_broadcasting();

        // Render properties panel (left panel or floating)
        let prop_actions = if let Some(panel) = self.dock_manager.get_panel(crate::ui::dock::panel_ids::PROPERTIES) {
            if panel.open {
                let zone = panel.zone;
                let floating_pos = panel.floating_pos;
                let floating_size = panel.floating_size;
                let dock_width = panel.dock_width;
                
                match zone {
                    crate::ui::DockZone::Left => {
                        let mut actions = Vec::new();
                        egui::SidePanel::left("properties_panel")
                            .default_width(dock_width)
                            .resizable(true)
                            .show(&self.egui_ctx, |ui| {
                                // Panel header with undock button
                                ui.horizontal(|ui| {
                                    ui.heading("Properties");
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button("⊞").on_hover_text("Undock panel").clicked() {
                                            if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::PROPERTIES) {
                                                p.zone = crate::ui::DockZone::Floating;
                                                p.floating_pos = Some((100.0, 100.0));
                                                p.floating_size = Some((300.0, 400.0));
                                            }
                                        }
                                    });
                                });
                                ui.separator();
                                actions = self.properties_panel.render(ui, &self.environment, &layers, &self.settings, omt_broadcasting, self.texture_share_enabled, self.effect_manager.registry());
                            });
                        actions
                    }
                    crate::ui::DockZone::Floating => {
                        let mut actions = Vec::new();
                        let pos = floating_pos.unwrap_or((100.0, 100.0));
                        let size = floating_size.unwrap_or((300.0, 400.0));
                        let mut open = true;
                        
                        // Check if there's a pending snap to apply
                        let snap_pos = self.dock_manager.take_pending_snap(crate::ui::dock::panel_ids::PROPERTIES);
                        
                        let mut window = egui::Window::new("Properties")
                            .id(egui::Id::new("properties_window"))
                            .default_pos(egui::pos2(pos.0, pos.1))
                            .default_size(egui::vec2(size.0, size.1))
                            .resizable(true)
                            .collapsible(true);
                        
                        // Apply snap position if pending (overrides egui's tracked position)
                        if let Some(snap) = snap_pos {
                            window = window.current_pos(egui::pos2(snap.0, snap.1));
                        }
                        
                        let window_response = window
                            .open(&mut open)
                            .show(&self.egui_ctx, |ui| {
                                // Dock button in header
                                ui.horizontal(|ui| {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.small_button("⊟").on_hover_text("Dock to left").clicked() {
                                            if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::PROPERTIES) {
                                                p.zone = crate::ui::DockZone::Left;
                                            }
                                        }
                                    });
                                });
                                ui.separator();
                                actions = self.properties_panel.render(ui, &self.environment, &layers, &self.settings, omt_broadcasting, self.texture_share_enabled, self.effect_manager.registry());
                            });

                        // Track window dragging for dock zone snapping
                        if let Some(resp) = &window_response {
                            if resp.response.drag_started() {
                                let cursor_pos = self.egui_ctx.input(|i| i.pointer.hover_pos().unwrap_or_default());
                                self.dock_manager.start_drag(crate::ui::dock::panel_ids::PROPERTIES, (cursor_pos.x, cursor_pos.y));
                            }
                            if resp.response.dragged() {
                                let cursor_pos = self.egui_ctx.input(|i| i.pointer.hover_pos().unwrap_or_default());
                                let screen_rect = self.egui_ctx.screen_rect();
                                self.dock_manager.update_drag((cursor_pos.x, cursor_pos.y), (screen_rect.width(), screen_rect.height()));
                            }
                            if resp.response.drag_stopped() {
                                // Get actual window rect from egui
                                let window_rect = resp.response.rect;
                                let window_pos = (window_rect.left(), window_rect.top());
                                let window_size = (window_rect.width(), window_rect.height());
                                
                                // End drag with window rect for proper snapping
                                self.dock_manager.end_drag_with_rect(window_pos, window_size);
                            }
                        }
                        
                        if !open {
                            if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::PROPERTIES) {
                                p.open = false;
                            }
                        }
                        actions
                    }
                    _ => {
                        // For other zones (Right, Top, Bottom), render as appropriate panel type
                        Vec::new()
                    }
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        
        // Handle properties actions
        for action in prop_actions {
            self.handle_properties_action(action);
        }
        
        // Render clip grid panel (right panel or floating)
        let clip_actions = if let Some(panel) = self.dock_manager.get_panel(crate::ui::dock::panel_ids::CLIP_GRID) {
            if panel.open {
                let zone = panel.zone;
                let floating_pos = panel.floating_pos;
                let floating_size = panel.floating_size;
                let dock_width = panel.dock_width;
                
                match zone {
                    crate::ui::DockZone::Right => {
                        let mut actions = Vec::new();
                        egui::SidePanel::right("clip_grid_side_panel")
                            .default_width(dock_width)
                            .resizable(true)
                            .show(&self.egui_ctx, |ui| {
                                // Panel header with undock button
                                ui.horizontal(|ui| {
                                    ui.heading("Clip Grid");
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button("⊞").on_hover_text("Undock panel").clicked() {
                                            if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::CLIP_GRID) {
                                                p.zone = crate::ui::DockZone::Floating;
                                                p.floating_pos = Some((400.0, 100.0));
                                                p.floating_size = Some((400.0, 300.0));
                                            }
                                        }
                                    });
                                });
                                ui.separator();
                                actions = self.clip_grid_panel.render_contents(ui, &layers, &mut self.thumbnail_cache);
                            });
                        actions
                    }
                    crate::ui::DockZone::Floating => {
                        let mut actions = Vec::new();
                        let pos = floating_pos.unwrap_or((400.0, 100.0));
                        let size = floating_size.unwrap_or((400.0, 300.0));
                        let mut open = true;
                        
                        // Check if there's a pending snap to apply
                        let snap_pos = self.dock_manager.take_pending_snap(crate::ui::dock::panel_ids::CLIP_GRID);
                        
                        let mut window = egui::Window::new("Clip Grid")
                            .id(egui::Id::new("clip_grid_window"))
                            .default_pos(egui::pos2(pos.0, pos.1))
                            .default_size(egui::vec2(size.0, size.1))
                            .resizable(true)
                            .collapsible(true);
                        
                        // Apply snap position if pending (overrides egui's tracked position)
                        if let Some(snap) = snap_pos {
                            window = window.current_pos(egui::pos2(snap.0, snap.1));
                        }
                        
                        let window_response = window
                            .open(&mut open)
                            .show(&self.egui_ctx, |ui| {
                                // Dock button in header
                                ui.horizontal(|ui| {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.small_button("⊟").on_hover_text("Dock to right").clicked() {
                                            if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::CLIP_GRID) {
                                                p.zone = crate::ui::DockZone::Right;
                                            }
                                        }
                                    });
                                });
                                ui.separator();
                                actions = self.clip_grid_panel.render_contents(ui, &layers, &mut self.thumbnail_cache);
                            });

                        // Track window dragging for dock zone snapping
                        if let Some(resp) = &window_response {
                            if resp.response.drag_started() {
                                let cursor_pos = self.egui_ctx.input(|i| i.pointer.hover_pos().unwrap_or_default());
                                self.dock_manager.start_drag(crate::ui::dock::panel_ids::CLIP_GRID, (cursor_pos.x, cursor_pos.y));
                            }
                            if resp.response.dragged() {
                                let cursor_pos = self.egui_ctx.input(|i| i.pointer.hover_pos().unwrap_or_default());
                                let screen_rect = self.egui_ctx.screen_rect();
                                self.dock_manager.update_drag((cursor_pos.x, cursor_pos.y), (screen_rect.width(), screen_rect.height()));
                            }
                            if resp.response.drag_stopped() {
                                // Get actual window rect from egui
                                let window_rect = resp.response.rect;
                                let window_pos = (window_rect.left(), window_rect.top());
                                let window_size = (window_rect.width(), window_rect.height());
                                
                                // End drag with window rect for proper snapping
                                self.dock_manager.end_drag_with_rect(window_pos, window_size);
                            }
                        }
                        
                        if !open {
                            if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::CLIP_GRID) {
                                p.open = false;
                            }
                        }
                        actions
                    }
                    _ => {
                        // For other zones, render in default position
                        self.clip_grid_panel.render(&self.egui_ctx, &layers, &mut self.thumbnail_cache)
                    }
                }
            } else {
                Vec::new()
            }
        } else {
            // Fallback if dock manager doesn't have panel registered yet
            self.clip_grid_panel.render(&self.egui_ctx, &layers, &mut self.thumbnail_cache)
        };

        // Render sources panel (floating window for now)
        let sources_actions = if let Some(panel) = self.dock_manager.get_panel(crate::ui::dock::panel_ids::SOURCES) {
            if panel.open {
                let floating_pos = panel.floating_pos;
                let floating_size = panel.floating_size;
                let mut actions = Vec::new();
                let pos = floating_pos.unwrap_or((100.0, 300.0));
                let size = floating_size.unwrap_or((250.0, 300.0));
                let mut open = true;
                
                let window_response = egui::Window::new("Sources")
                    .id(egui::Id::new("sources_window"))
                    .default_pos(egui::pos2(pos.0, pos.1))
                    .default_size(egui::vec2(size.0, size.1))
                    .resizable(true)
                    .collapsible(true)
                    .open(&mut open)
                    .show(&self.egui_ctx, |ui| {
                        actions = self.sources_panel.render_contents(ui);
                    });
                
                // Update window position for persistence
                if let Some(resp) = &window_response {
                    let rect = resp.response.rect;
                    if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::SOURCES) {
                        p.floating_pos = Some((rect.left(), rect.top()));
                        p.floating_size = Some((rect.width(), rect.height()));
                    }
                }
                
                if !open {
                    if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::SOURCES) {
                        p.open = false;
                    }
                }
                actions
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Render effects browser panel (floating window)
        let _effects_actions = if let Some(panel) = self.dock_manager.get_panel(crate::ui::dock::panel_ids::EFFECTS_BROWSER) {
            if panel.open {
                let floating_pos = panel.floating_pos;
                let floating_size = panel.floating_size;
                let mut actions = Vec::new();
                let pos = floating_pos.unwrap_or((100.0, 400.0));
                let size = floating_size.unwrap_or((250.0, 350.0));
                let mut open = true;

                let window_response = egui::Window::new("Effects")
                    .id(egui::Id::new("effects_browser_window"))
                    .default_pos(egui::pos2(pos.0, pos.1))
                    .default_size(egui::vec2(size.0, size.1))
                    .resizable(true)
                    .collapsible(true)
                    .open(&mut open)
                    .show(&self.egui_ctx, |ui| {
                        actions = self.effects_browser_panel.render_contents(ui, self.effect_manager.registry());
                    });

                // Update window position for persistence
                if let Some(resp) = &window_response {
                    let rect = resp.response.rect;
                    if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::EFFECTS_BROWSER) {
                        p.floating_pos = Some((rect.left(), rect.top()));
                        p.floating_size = Some((rect.width(), rect.height()));
                    }
                }

                if !open {
                    if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::EFFECTS_BROWSER) {
                        p.open = false;
                    }
                }
                actions
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Render file browser panel (floating window)
        if let Some(panel) = self.dock_manager.get_panel(crate::ui::dock::panel_ids::FILES) {
            if panel.open {
                let floating_pos = panel.floating_pos;
                let floating_size = panel.floating_size;
                let pos = floating_pos.unwrap_or((350.0, 100.0));
                let size = floating_size.unwrap_or((280.0, 400.0));
                let mut open = true;

                let window_response = egui::Window::new("Files")
                    .id(egui::Id::new("files_window"))
                    .default_pos(egui::pos2(pos.0, pos.1))
                    .default_size(egui::vec2(size.0, size.1))
                    .resizable(true)
                    .collapsible(true)
                    .open(&mut open)
                    .show(&self.egui_ctx, |ui| {
                        self.file_browser_panel.render_contents(ui);
                    });

                // Update window position for persistence
                if let Some(resp) = &window_response {
                    let rect = resp.response.rect;
                    if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::FILES) {
                        p.floating_pos = Some((rect.left(), rect.top()));
                        p.floating_size = Some((rect.width(), rect.height()));
                    }
                }

                if !open {
                    if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::FILES) {
                        p.open = false;
                    }
                }
            }
        }

        // Render preview monitor panel (floating window)
        let preview_actions = if let Some(panel) = self.dock_manager.get_panel(crate::ui::dock::panel_ids::PREVIEW_MONITOR) {
            if panel.open {
                let floating_pos = panel.floating_pos;
                let floating_size = panel.floating_size;
                let mut actions = Vec::new();
                let pos = floating_pos.unwrap_or((600.0, 100.0));
                let size = floating_size.unwrap_or((320.0, 280.0));
                let mut open = true;

                let has_frame = self.preview_player.has_frame();
                let is_playing = !self.preview_player.is_paused();
                let video_info = self.preview_player.video_info();

                let window_response = egui::Window::new("Preview Monitor")
                    .id(egui::Id::new("preview_monitor_window"))
                    .default_pos(egui::pos2(pos.0, pos.1))
                    .default_size(egui::vec2(size.0, size.1))
                    .resizable(true)
                    .collapsible(true)
                    .open(&mut open)
                    .show(&self.egui_ctx, |ui| {
                        actions = self.preview_monitor_panel.render_contents(
                            ui,
                            has_frame,
                            is_playing,
                            video_info,
                            |ui, rect| {
                                // Render the preview texture into the given rect
                                if let (Some(bind_group), Some(params_buffer)) =
                                    (self.preview_player.bind_group(), self.preview_player.params_buffer())
                                {
                                    // For now, just draw a placeholder rectangle
                                    // Full GPU rendering would require more integration
                                    ui.painter().rect_filled(
                                        rect,
                                        4.0,
                                        egui::Color32::from_rgb(30, 60, 30),
                                    );
                                    ui.painter().text(
                                        rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "Preview Active",
                                        egui::FontId::proportional(11.0),
                                        egui::Color32::WHITE,
                                    );
                                }
                            },
                        );
                    });

                // Update window position for persistence
                if let Some(resp) = &window_response {
                    let rect = resp.response.rect;
                    if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::PREVIEW_MONITOR) {
                        p.floating_pos = Some((rect.left(), rect.top()));
                        p.floating_size = Some((rect.width(), rect.height()));
                    }
                }

                if !open {
                    if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::PREVIEW_MONITOR) {
                        p.open = false;
                    }
                }
                actions
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Apply environment resolution changes (if any) before rendering.
        self.sync_environment_from_settings();

        let full_output = self.egui_ctx.end_pass();

        // Process clip grid actions (after egui pass ends)
        for action in clip_actions {
            self.handle_clip_action(action);
        }
        
        // Process sources panel actions
        for action in sources_actions {
            self.handle_sources_action(action);
        }

        // Process preview monitor actions
        for action in preview_actions {
            self.handle_preview_action(action);
        }

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

        // 2. Render layers back-to-front (index 0 = back, last = front)
        for layer in self.environment.layers() {
            // Skip invisible layers or fully transparent layers
            if !layer.visible || layer.opacity <= 0.0 {
                continue;
            }

            // Get runtime resources for this layer
            if let Some(runtime) = self.layer_runtimes.get(&layer.id) {
                // Check if we're in a transition
                let transition_progress = runtime.transition_progress();
                let in_transition = runtime.transition_active && transition_progress < 1.0;
                
                // For crossfade: render old content first at (1 - progress) opacity
                if in_transition && runtime.transition_type.needs_old_content() {
                    if let Some(old_bind_group) = &runtime.old_bind_group {
                        if let Some(old_params_buffer) = &runtime.old_params_buffer {
                            let old_opacity = layer.opacity * (1.0 - transition_progress);
                            if old_opacity > 0.0 {
                                let mut params = LayerParams::from_layer(
                                    layer,
                                    runtime.old_video_width,
                                    runtime.old_video_height,
                                    self.environment.width(),
                                    self.environment.height(),
                                );
                                params.opacity = old_opacity;
                                // Write to old layer's params buffer (not shared)
                                self.video_renderer.write_layer_params(&self.queue, old_params_buffer, &params);
                                
                                self.video_renderer.render_with_blend(
                                    &mut encoder,
                                    self.environment.texture_view(),
                                    old_bind_group,
                                    layer.blend_mode,
                                    false,
                                );
                            }
                        }
                    }
                }

                // Only render if we have a bind_group AND at least one frame has been uploaded
                if let Some(bind_group) = &runtime.bind_group {
                    if let Some(params_buffer) = &runtime.params_buffer {
                        if runtime.has_frame {
                            // Calculate opacity with transition and fade-out
                            let effective_opacity = if runtime.fade_out_active {
                                // Fading out: opacity goes from layer.opacity to 0
                                layer.opacity * (1.0 - runtime.fade_out_progress())
                            } else if in_transition {
                                layer.opacity * transition_progress
                            } else {
                                layer.opacity
                            };

                            // Skip rendering if fully transparent
                            if effective_opacity > 0.0 {
                                // Get clip effects for the active clip
                                let (clip_slot, clip_effects) = layer
                                    .active_clip
                                    .and_then(|slot| layer.get_clip(slot).map(|c| (slot, c)))
                                    .map(|(slot, clip)| (Some(slot), Some(&clip.effects)))
                                    .unwrap_or((None, None));

                                let clip_active_effect_count = clip_effects
                                    .map(|e| e.active_effects().count())
                                    .unwrap_or(0);

                                // Check if layer has active effects
                                let layer_active_effect_count = layer.effects.active_effects().count();

                                // Determine what effects path to take
                                let has_clip_effects = clip_active_effect_count > 0;
                                let has_layer_effects = layer_active_effect_count > 0;
                                let has_any_effects = has_clip_effects || has_layer_effects;

                                if has_any_effects {
                                    // --- EFFECT PROCESSING PATH ---
                                    // Effect textures are VIDEO-SIZED so effects process at native resolution

                                    // Track what texture to use as input for the next stage
                                    let mut current_input_is_clip_output = false;

                                    // ========== CLIP EFFECTS ==========
                                    if has_clip_effects {
                                        if let Some(slot) = clip_slot {
                                            // 1. Ensure clip effect runtime exists
                                            self.effect_manager.ensure_clip_runtime(
                                                layer.id,
                                                slot,
                                                &self.device,
                                                runtime.video_width,
                                                runtime.video_height,
                                                self.config.format,
                                            );

                                            // 2. Sync clip effect runtimes
                                            if let Some(clip_effect_stack) = clip_effects {
                                                self.effect_manager.sync_clip_effects(
                                                    layer.id,
                                                    slot,
                                                    clip_effect_stack,
                                                    &self.device,
                                                    &self.queue,
                                                    self.config.format,
                                                );
                                            }

                                            // 3. Copy video texture to clip effect input
                                            if let Some(video_texture) = &runtime.texture {
                                                if let Some(clip_runtime) = self.effect_manager.get_clip_runtime(layer.id, slot) {
                                                    clip_runtime.copy_input_texture(
                                                        &mut encoder,
                                                        &self.device,
                                                        video_texture.view(),
                                                    );
                                                }
                                            }

                                            // 4. Process clip effects
                                            let effect_params = self.effect_manager.build_params();
                                            if let Some(clip_runtime) = self.effect_manager.get_clip_runtime_mut(layer.id, slot) {
                                                if let (Some(input_view), Some(output_view)) = (
                                                    clip_runtime.input_view().map(|v| v as *const _),
                                                    clip_runtime.output_view(clip_active_effect_count).map(|v| v as *const _),
                                                ) {
                                                    if let Some(clip_effect_stack) = clip_effects {
                                                        unsafe {
                                                            clip_runtime.process(
                                                                &mut encoder,
                                                                &self.queue,
                                                                &self.device,
                                                                &*input_view,
                                                                &*output_view,
                                                                clip_effect_stack,
                                                                &effect_params,
                                                            );
                                                        }
                                                    }
                                                }
                                            }
                                            current_input_is_clip_output = true;
                                        }
                                    }

                                    // ========== LAYER EFFECTS ==========
                                    if has_layer_effects {
                                        // 1. Ensure layer effect runtime exists
                                        self.effect_manager.ensure_layer_runtime(
                                            layer.id,
                                            &self.device,
                                            runtime.video_width,
                                            runtime.video_height,
                                            self.config.format,
                                        );

                                        // 2. Sync layer effect runtimes
                                        self.effect_manager.sync_layer_effects(
                                            layer.id,
                                            &layer.effects,
                                            &self.device,
                                            &self.queue,
                                            self.config.format,
                                        );

                                        // 3. Copy input to layer effect input
                                        // Input is either clip effect output or video texture
                                        if current_input_is_clip_output {
                                            // Use clip effect output as input
                                            if let Some(slot) = clip_slot {
                                                if let Some(clip_runtime) = self.effect_manager.get_clip_runtime(layer.id, slot) {
                                                    if let Some(clip_output) = clip_runtime.output_view(clip_active_effect_count) {
                                                        if let Some(layer_runtime) = self.effect_manager.get_layer_runtime(layer.id) {
                                                            layer_runtime.copy_input_texture(
                                                                &mut encoder,
                                                                &self.device,
                                                                clip_output,
                                                            );
                                                        }
                                                    }
                                                }
                                            }
                                        } else {
                                            // Use video texture directly
                                            if let Some(video_texture) = &runtime.texture {
                                                if let Some(layer_runtime) = self.effect_manager.get_layer_runtime(layer.id) {
                                                    layer_runtime.copy_input_texture(
                                                        &mut encoder,
                                                        &self.device,
                                                        video_texture.view(),
                                                    );
                                                }
                                            }
                                        }

                                        // 4. Process layer effects
                                        let effect_params = self.effect_manager.build_params();
                                        if let Some(layer_runtime) = self.effect_manager.get_layer_runtime_mut(layer.id) {
                                            if let (Some(input_view), Some(output_view)) = (
                                                layer_runtime.input_view().map(|v| v as *const _),
                                                layer_runtime.output_view(layer_active_effect_count).map(|v| v as *const _),
                                            ) {
                                                unsafe {
                                                    layer_runtime.process(
                                                        &mut encoder,
                                                        &self.queue,
                                                        &self.device,
                                                        &*input_view,
                                                        &*output_view,
                                                        &layer.effects,
                                                        &effect_params,
                                                    );
                                                }
                                            }
                                        }
                                    }

                                    // ========== COMPOSITE TO ENVIRONMENT ==========
                                    // Determine which output to use: layer effects output or clip effects output
                                    let final_output_view = if has_layer_effects {
                                        self.effect_manager.get_layer_runtime(layer.id)
                                            .and_then(|r| r.output_view(layer_active_effect_count))
                                    } else if has_clip_effects {
                                        clip_slot.and_then(|slot| {
                                            self.effect_manager.get_clip_runtime(layer.id, slot)
                                                .and_then(|r| r.output_view(clip_active_effect_count))
                                        })
                                    } else {
                                        None
                                    };

                                    if let Some(effect_output_view) = final_output_view {
                                        let mut composite_params = LayerParams::from_layer(
                                            layer,
                                            runtime.video_width,
                                            runtime.video_height,
                                            self.environment.width(),
                                            self.environment.height(),
                                        );
                                        composite_params.opacity = effective_opacity;
                                        self.video_renderer.write_layer_params(&self.queue, params_buffer, &composite_params);

                                        let effect_bind_group = self.video_renderer.create_bind_group_with_view(
                                            &self.device,
                                            effect_output_view,
                                            params_buffer,
                                        );

                                        self.video_renderer.render_with_blend(
                                            &mut encoder,
                                            self.environment.texture_view(),
                                            &effect_bind_group,
                                            layer.blend_mode,
                                            false,
                                        );
                                    }
                                } else {
                                    // --- NO EFFECTS - DIRECT RENDERING ---
                                    let mut params = LayerParams::from_layer(
                                        layer,
                                        runtime.video_width,
                                        runtime.video_height,
                                        self.environment.width(),
                                        self.environment.height(),
                                    );
                                    params.opacity = effective_opacity;
                                    self.video_renderer.write_layer_params(&self.queue, params_buffer, &params);

                                    self.video_renderer.render_with_blend(
                                        &mut encoder,
                                        self.environment.texture_view(),
                                        bind_group,
                                        layer.blend_mode,
                                        false,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // ========== ENVIRONMENT EFFECTS (Master Post-Processing) ==========
        // Process environment effects AFTER all layers composited, BEFORE capture/output
        {
            let env_effects = self.environment.effects();
            let env_active_effect_count = env_effects.active_effects().count();

            if env_active_effect_count > 0 {
                // 1. Ensure environment effect runtime exists at ENVIRONMENT resolution
                self.effect_manager.init_environment_effects(
                    &self.device,
                    self.environment.width(),
                    self.environment.height(),
                    self.config.format,
                );

                // 2. Sync environment effect runtimes
                self.effect_manager.sync_environment_effects(
                    env_effects,
                    &self.device,
                    &self.queue,
                    self.config.format,
                );

                // 3. Process environment effects in-place on the environment texture
                self.effect_manager.process_environment_effects(
                    &mut encoder,
                    &self.device,
                    &self.queue,
                    self.environment.texture_view(),
                    env_effects,
                );
            }
        }

        // Capture environment texture for OMT output (before we move on to present)
        if self.omt_broadcast_enabled {
            if let Some(capture) = &mut self.omt_capture {
                capture.capture_frame(&mut encoder, self.environment.texture());
            }
        }

        // Publish environment texture via Syphon (macOS)
        #[cfg(target_os = "macos")]
        if self.texture_share_enabled {
            if let (Some(ref sharer), Some(ref queue)) =
                (&self.texture_sharer, &self.metal_command_queue)
            {
                unsafe {
                    if let Err(e) =
                        sharer.publish_wgpu_texture(&self.device, self.environment.texture(), queue)
                    {
                        log::warn!("Syphon: Failed to publish frame: {}", e);
                    }
                }
            }
        }

        // Capture environment texture for Spout output (Windows)
        #[cfg(target_os = "windows")]
        if self.texture_share_enabled {
            if let Some(capture) = &mut self.spout_capture {
                capture.capture_frame(&mut encoder, self.environment.texture());
            }
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

        // Process OMT capture pipeline (non-blocking)
        if self.omt_broadcast_enabled {
            if let Some(capture) = &mut self.omt_capture {
                capture.process(&self.device);
            }
        }

        // Process Spout capture pipeline (Windows, non-blocking)
        #[cfg(target_os = "windows")]
        if self.texture_share_enabled {
            if let Some(capture) = &mut self.spout_capture {
                capture.process(&self.device);
            }
        }

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

    // Layer management methods

    /// Add a new layer with a video source.
    /// Returns the layer ID on success.
    pub fn add_layer_with_video(
        &mut self,
        name: impl Into<String>,
        path: &std::path::Path,
    ) -> Result<u32, String> {
        // Create the layer in the environment
        let layer_id = self.environment.add_layer(name);

        // Set the layer's source to the video path
        if let Some(layer) = self.environment.get_layer_mut(layer_id) {
            layer.source = LayerSource::Video(path.to_path_buf());
        }

        // Create runtime resources
        self.load_layer_video(layer_id, path)?;

        log::info!("Added layer {} with video: {:?}", layer_id, path);
        Ok(layer_id)
    }

    /// Load a video for an existing layer
    fn load_layer_video(&mut self, layer_id: u32, path: &std::path::Path) -> Result<(), String> {
        let old_runtime_exists = self.layer_runtimes.contains_key(&layer_id);

        // Open video player (starts background decode thread)
        let player =
            VideoPlayer::open(path).map_err(|e| format!("Failed to open video: {}", e))?;

        log::info!(
            "Layer {}: Loaded video {}x{} @ {:.2}fps, duration: {:.2}s, gpu_native: {}",
            layer_id,
            player.width(),
            player.height(),
            player.frame_rate(),
            player.duration(),
            player.is_gpu_native()
        );

        // Create video texture with appropriate format
        // HAP uses raw DXT extraction for GPU-native upload
        // DXV v4 (most common) requires FFmpeg decode to RGBA due to proprietary compression
        let use_gpu_native = player.is_hap() && self.bc_texture_supported;
        
        let video_texture = if use_gpu_native {
            log::info!("Using GPU-native BC texture for layer {} (HAP fast path)", layer_id);
            VideoTexture::new_gpu_native(&self.device, player.width(), player.height(), player.is_bc3())
        } else {
            VideoTexture::new(&self.device, player.width(), player.height())
        };

        // Create per-layer params buffer to avoid overwrites during multi-layer rendering
        let params_buffer = self.video_renderer.create_params_buffer(&self.device);

        // Create bind group with per-layer params buffer
        let bind_group = self
            .video_renderer
            .create_bind_group_with_buffer(&self.device, &video_texture, &params_buffer);

        // Store runtime
        let runtime = LayerRuntime {
            layer_id,
            video_width: player.width(),
            video_height: player.height(),
            player: Some(player),
            texture: Some(video_texture),
            bind_group: Some(bind_group),
            has_frame: false, // Will be set to true when first frame is uploaded
            // Transition state (initialized empty)
            transition_active: false,
            transition_start: None,
            transition_duration: std::time::Duration::ZERO,
            transition_type: crate::compositor::ClipTransition::Cut,
            old_bind_group: None,
            old_video_width: 0,
            old_video_height: 0,
            old_params_buffer: None,
            params_buffer: Some(params_buffer),
            // Fade-out state (initialized empty)
            fade_out_active: false,
            fade_out_start: None,
            fade_out_duration: std::time::Duration::ZERO,
        };

        if old_runtime_exists {
            // Put in pending - old runtime continues to render until new one has a frame
            self.pending_runtimes.insert(layer_id, runtime);
        } else {
            // No old runtime - insert directly
            self.layer_runtimes.insert(layer_id, runtime);
        }

        Ok(())
    }

    /// Remove a layer by ID
    pub fn remove_layer(&mut self, layer_id: u32) -> bool {
        // Remove from environment
        let removed = self.environment.remove_layer(layer_id).is_some();

        // Clean up runtime resources
        self.layer_runtimes.remove(&layer_id);
        self.pending_runtimes.remove(&layer_id);

        if removed {
            log::info!("Removed layer {}", layer_id);
        }

        removed
    }

    /// Add a new layer to the environment
    pub fn add_layer(&mut self) {
        // Find the next available layer ID
        let next_id = self.environment.layers()
            .iter()
            .map(|l| l.id)
            .max()
            .map(|id| id + 1)
            .unwrap_or(1);

        // Create a new layer with the current global clip count
        let clip_count = self.settings.global_clip_count;
        let mut layer = crate::compositor::Layer::new(next_id, format!("Layer {}", next_id));
        layer.clips = vec![None; clip_count];

        self.environment.add_existing_layer(layer);
        log::info!("Added layer {} with {} clip slots", next_id, clip_count);
        self.menu_bar.set_status(format!("Added Layer {}", next_id));
    }

    /// Delete a layer by ID
    pub fn delete_layer(&mut self, layer_id: u32) {
        // Don't allow deleting the last layer
        if self.environment.layer_count() <= 1 {
            log::warn!("Cannot delete the last layer");
            self.menu_bar.set_status("Cannot delete the last layer".to_string());
            return;
        }

        if self.remove_layer(layer_id) {
            self.menu_bar.set_status(format!("Deleted layer {}", layer_id));
        }
    }

    /// Add a new column (clip slot) to all layers
    pub fn add_column(&mut self) {
        self.settings.global_clip_count += 1;
        let new_count = self.settings.global_clip_count;

        // Add a None slot to each layer
        for layer in self.environment.layers_mut() {
            layer.clips.push(None);
        }

        log::info!("Added column - now {} clip slots", new_count);
        self.menu_bar.set_status(format!("Added column {}", new_count));
    }

    /// Delete a column (clip slot) from all layers
    pub fn delete_column(&mut self, column_index: usize) {
        // Don't allow deleting the last column
        if self.settings.global_clip_count <= 1 {
            log::warn!("Cannot delete the last column");
            self.menu_bar.set_status("Cannot delete the last column".to_string());
            return;
        }

        // Check if the column index is valid
        if column_index >= self.settings.global_clip_count {
            log::warn!("Invalid column index: {}", column_index);
            return;
        }

        // Collect layer IDs that have clips playing in this column
        let layers_to_stop: Vec<u32> = self.environment.layers()
            .iter()
            .filter(|layer| layer.active_clip == Some(column_index))
            .map(|layer| layer.id)
            .collect();

        // Stop any clips playing in this column
        for layer_id in layers_to_stop {
            self.stop_clip(layer_id);
        }

        // Remove the slot from each layer
        for layer in self.environment.layers_mut() {
            if column_index < layer.clips.len() {
                layer.clips.remove(column_index);
                // Adjust active_clip if needed
                if let Some(active) = layer.active_clip {
                    if active > column_index {
                        layer.active_clip = Some(active - 1);
                    }
                }
            }
        }

        self.settings.global_clip_count -= 1;
        log::info!("Deleted column {} - now {} clip slots", column_index + 1, self.settings.global_clip_count);
        self.menu_bar.set_status(format!("Deleted column {}", column_index + 1));
    }

    /// Update all layer videos - pick up decoded frames (non-blocking)
    /// 
    /// Uploads ALL available frames each update cycle.
    /// The previous rate limiting (1 upload/frame) was too aggressive and caused video starvation.
    pub fn update_videos(&mut self) {
        // Collect layer IDs for round-robin iteration
        let mut layer_ids: Vec<u32> = self.layer_runtimes.keys().copied().collect();
        layer_ids.sort(); // Ensure consistent ordering
        
        // Find starting position in round-robin order
        let start_idx = layer_ids.iter()
            .position(|&id| id > self.last_upload_layer)
            .unwrap_or(0);
        
        // Collect layers that have completed fade-out (need to be cleared after iteration)
        let mut fade_out_complete: Vec<u32> = Vec::new();

        // Iterate in round-robin order starting after last uploaded layer
        for i in 0..layer_ids.len() {
            let idx = (start_idx + i) % layer_ids.len();
            let layer_id = layer_ids[idx];
            
            if let Some(runtime) = self.layer_runtimes.get_mut(&layer_id) {
                // Check if transition is complete
                if runtime.transition_active && runtime.is_transition_complete() {
                    runtime.end_transition();
                }

                // Check if fade-out is complete
                if runtime.is_fade_out_complete() {
                    fade_out_complete.push(layer_id);
                }
                
                // Upload all available frames - no rate limiting
                if runtime.try_update_texture(&self.queue) {
                    self.last_upload_layer = layer_id;
                }
            }
        }

        // Clear runtimes that have completed fade-out
        for layer_id in fade_out_complete {
            if let Some(runtime) = self.layer_runtimes.get_mut(&layer_id) {
                runtime.clear();
            }
            // Clear the active clip in the layer
            if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                layer.active_clip = None;
                layer.source = crate::compositor::LayerSource::None;
            }
            log::info!("⏹️ Fade-out complete, stopped clip on layer {}", layer_id);
        }
        
        // Update pending runtimes - these get priority since user is waiting
        for runtime in self.pending_runtimes.values_mut() {
            let _ = runtime.try_update_texture(&self.queue);
        }

        // Update preview player
        self.preview_player.update(&self.queue);

        // Swap pending runtimes into active once they have a frame
        let ready_layers: Vec<u32> = self.pending_runtimes
            .iter()
            .filter(|(_, runtime)| runtime.has_frame)
            .map(|(id, _)| *id)
            .collect();

        for layer_id in ready_layers {
            if let Some(mut new_runtime) = self.pending_runtimes.remove(&layer_id) {
                // Get the pending transition for this layer
                let transition = self.pending_transition.remove(&layer_id)
                    .unwrap_or(crate::compositor::ClipTransition::Cut);
                
                // For fade transition, transfer the old content from the old runtime
                if transition.needs_old_content() {
                    if let Some(old_runtime) = self.layer_runtimes.get_mut(&layer_id) {
                        // Move old bind group and params buffer to new runtime
                        new_runtime.old_bind_group = old_runtime.bind_group.take();
                        new_runtime.old_video_width = old_runtime.video_width;
                        new_runtime.old_video_height = old_runtime.video_height;
                        new_runtime.old_params_buffer = old_runtime.params_buffer.take();
                    }
                }
                
                // Start the transition
                if transition.duration_ms() > 0 {
                    new_runtime.start_transition(transition);
                }
                
                // Replace old runtime with new one that has a frame ready
                self.layer_runtimes.insert(layer_id, new_runtime);
            }
        }
    }

    /// Poll for shader changes and hot-reload if needed
    pub fn poll_shader_reload(&mut self) {
        if let Some(ref mut watcher) = self.shader_watcher {
            if watcher.poll().is_some() {
                // A shader file changed, reload it
                match crate::shaders::load_fullscreen_quad_shader() {
                    Ok(source) => {
                        if let Err(e) = self.video_renderer.rebuild_pipelines(&self.device, &source) {
                            log::error!("❌ Shader reload failed: {}", e);
                        }
                    }
                    Err(e) => {
                        log::error!("❌ Failed to read shader file: {}", e);
                    }
                }
            }
        }
    }

    /// Toggle pause state for a specific layer
    pub fn toggle_layer_pause(&self, layer_id: u32) {
        if let Some(runtime) = self.layer_runtimes.get(&layer_id) {
            runtime.toggle_pause();
        }
    }

    /// Restart video for a specific layer
    pub fn restart_layer_video(&self, layer_id: u32) {
        if let Some(runtime) = self.layer_runtimes.get(&layer_id) {
            runtime.restart();
        }
    }

    /// Toggle pause for all layers
    pub fn toggle_all_pause(&self) {
        for runtime in self.layer_runtimes.values() {
            runtime.toggle_pause();
        }
    }

    /// Restart all layer videos
    pub fn restart_all_videos(&self) {
        for runtime in self.layer_runtimes.values() {
            runtime.restart();
        }
    }

    /// Check if any layer has video
    pub fn has_video(&self) -> bool {
        self.layer_runtimes.values().any(|r| r.has_video())
    }

    /// Check if any video is paused (returns true if any layer is paused)
    pub fn is_any_video_paused(&self) -> bool {
        self.layer_runtimes.values().any(|r| r.is_paused())
    }

    /// Get number of layers
    pub fn layer_count(&self) -> usize {
        self.environment.layer_count()
    }

    // =========================================================================
    // Clip Grid Methods
    // =========================================================================

    /// Trigger a clip on a layer at the specified slot
    ///
    /// Loads and plays the video from the clip cell. Stops any currently
    /// playing clip on this layer first.
    ///
    /// Returns `Ok(())` if successful, or an error message if the clip
    /// couldn't be loaded.
    pub fn trigger_clip(&mut self, layer_id: u32, slot: usize) -> Result<(), String> {
        // Get the clip source and layer transition
        let (clip_source, transition) = {
            let layer = self.environment.get_layer(layer_id)
                .ok_or_else(|| format!("Layer {} not found", layer_id))?;

            let cell = layer.get_clip(slot)
                .ok_or_else(|| format!("No clip at slot {}", slot))?;

            (cell.source.clone(), layer.transition)
        };

        // Handle different source types
        match &clip_source {
            crate::compositor::ClipSource::File { path } => {
                // Check if this is a replay of the same clip (same path)
                let is_same_clip = if let Some(runtime) = self.layer_runtimes.get(&layer_id) {
                    if let Some(player) = &runtime.player {
                        player.path() == path.as_path()
                    } else {
                        false
                    }
                } else {
                    false
                };

                if is_same_clip {
                    // Same clip - just restart playback (no flash!)
                    log::info!("🔄 Restarting clip {} on layer {}", slot, layer_id);
                    self.restart_layer_video(layer_id);
                } else {
                    // Different clip - need to load it
                    log::info!("🎬 Loading clip {} on layer {} with {:?} transition: {:?}",
                        slot, layer_id, transition.name(), path.display());

                    // Store the transition type for when the new clip is ready
                    self.pending_transition.insert(layer_id, transition);

                    self.load_layer_video(layer_id, path)?;
                }

                // Update the active clip slot in the layer
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    layer.active_clip = Some(slot);
                    layer.source = crate::compositor::LayerSource::Video(path.clone());
                }
            }
            crate::compositor::ClipSource::Omt { address, name } => {
                // OMT source playback not yet implemented
                log::warn!("📡 OMT playback not yet implemented: {} ({})", name, address);
                return Err(format!("OMT playback not yet implemented. Source: {} ({})", name, address));
            }
        }

        Ok(())
    }

    /// Stop the currently playing clip on a layer
    ///
    /// Clears the video player and resets the active clip indicator.
    pub fn stop_clip(&mut self, layer_id: u32) {
        // Clear the runtime video resources
        if let Some(runtime) = self.layer_runtimes.get_mut(&layer_id) {
            runtime.clear();
        }

        // Clear the active clip in the layer
        if let Some(layer) = self.environment.get_layer_mut(layer_id) {
            layer.active_clip = None;
            layer.source = crate::compositor::LayerSource::None;
        }

        log::info!("⏹️ Stopped clip on layer {}", layer_id);
    }

    /// Stop the currently playing clip on a layer with a fade-out transition
    ///
    /// Starts a fade-out animation; the actual clear happens when fade completes.
    pub fn stop_clip_with_fade(&mut self, layer_id: u32) {
        // Get the transition duration from the layer
        let fade_duration = self.environment
            .get_layer(layer_id)
            .map(|l| l.transition.duration_ms())
            .unwrap_or(0);

        if fade_duration == 0 {
            // No fade, just stop immediately
            self.stop_clip(layer_id);
            return;
        }

        // Start the fade-out on the runtime
        if let Some(runtime) = self.layer_runtimes.get_mut(&layer_id) {
            if runtime.has_frame && !runtime.fade_out_active {
                runtime.start_fade_out(std::time::Duration::from_millis(fade_duration as u64));
                log::info!("⏹️ Starting fade-out on layer {} ({}ms)", layer_id, fade_duration);
            } else {
                // No frame or already fading, just stop immediately
                self.stop_clip(layer_id);
            }
        } else {
            // No runtime, nothing to fade
            self.stop_clip(layer_id);
        }
    }

    /// Set a clip in a layer's clip slots
    ///
    /// Assigns a video path to a slot in the layer's clips.
    pub fn set_layer_clip(
        &mut self,
        layer_id: u32,
        slot: usize,
        path: std::path::PathBuf,
        label: Option<String>,
    ) -> bool {
        if let Some(layer) = self.environment.get_layer_mut(layer_id) {
            let cell = if let Some(lbl) = label {
                crate::compositor::ClipCell::with_label(path, lbl)
            } else {
                crate::compositor::ClipCell::new(path)
            };
            layer.set_clip(slot, cell)
        } else {
            false
        }
    }

    /// Clear a clip from a layer's clips
    pub fn clear_layer_clip(&mut self, layer_id: u32, slot: usize) -> bool {
        if let Some(layer) = self.environment.get_layer_mut(layer_id) {
            // If this is the active clip, stop it first
            if layer.active_clip == Some(slot) {
                self.stop_clip(layer_id);
            }
            if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                return layer.clear_clip(slot);
            }
        }
        false
    }

    // =========================================================================
    // OMT (Open Media Transport) Methods
    // =========================================================================

    /// Set an OMT source as a clip in a layer's clip slots
    pub fn set_layer_omt_clip(
        &mut self,
        layer_id: u32,
        slot: usize,
        address: String,
        name: String,
    ) -> bool {
        if let Some(layer) = self.environment.get_layer_mut(layer_id) {
            let cell = crate::compositor::ClipCell::from_omt(&address, &name);
            if layer.set_clip(slot, cell) {
                log::info!(
                    "📡 Assigned OMT source '{}' ({}) to layer {} slot {}",
                    name, address, layer_id, slot
                );
                self.menu_bar.set_status(format!("Assigned OMT source: {}", name));
                return true;
            }
        }
        false
    }

    /// Refresh the list of discovered OMT sources
    pub fn refresh_omt_sources(&mut self) {
        // Get sources from discovery service
        let sources: Vec<(String, String, String)> = if let Some(discovery) = &self.omt_discovery {
            discovery.get_sources()
                .into_iter()
                .map(|s| {
                    let address = s.address(); // Call before moving name
                    (s.id, s.name, address)
                })
                .collect()
        } else {
            Vec::new()
        };

        let count = sources.len();
        self.sources_panel.set_omt_sources(sources);
        log::info!("📡 OMT: Refreshed source list, found {} sources", count);
        self.menu_bar.set_status(format!("Found {} OMT sources", count));
    }

    /// Update the UI with the current OMT sources (called periodically)
    pub fn update_omt_sources_in_ui(&mut self) {
        if let Some(discovery) = &self.omt_discovery {
            let sources: Vec<(String, String, String)> = discovery.get_sources()
                .into_iter()
                .map(|s| {
                    let address = s.address(); // Call before moving name
                    (s.id, s.name, address)
                })
                .collect();
            self.sources_panel.set_omt_sources(sources);
        }
    }

    /// Start OMT broadcast of the environment
    ///
    /// This announces the server as an OMT source so other applications
    /// can receive the environment's output. Non-blocking - spawns background thread.
    pub fn start_omt_broadcast(&mut self, name: &str, port: u16) {
        if self.omt_capture.is_some() || self.pending_omt_sender.is_some() {
            log::info!("OMT: Broadcast already active or starting");
            return;
        }

        if let Some(rt) = &self.tokio_runtime {
            let (tx, rx) = std::sync::mpsc::channel();
            let name = name.to_string();
            let runtime_handle = rt.handle().clone();

            // Spawn background thread to start sender (avoids blocking UI)
            std::thread::spawn(move || {
                let mut sender = crate::network::OmtSender::new(name.clone(), port);
                let result = runtime_handle.block_on(async {
                    sender.start().await
                });

                match result {
                    Ok(()) => {
                        log::info!("📡 OMT: Started sender as '{}' on port {}", name, port);
                        let _ = tx.send(Ok(sender));
                    }
                    Err(e) => {
                        log::error!("OMT: Failed to start broadcast: {}", e);
                        let _ = tx.send(Err(format!("{}", e)));
                    }
                }
            });

            self.pending_omt_sender = Some(rx);
            self.menu_bar.set_status("Starting OMT broadcast...");
        } else {
            log::warn!("OMT: Cannot start broadcast - no Tokio runtime");
        }
    }

    /// Poll for pending OMT sender and complete setup if ready.
    /// Call this each frame from the render loop.
    fn poll_pending_omt_sender(&mut self) {
        let rx = match self.pending_omt_sender.take() {
            Some(rx) => rx,
            None => return,
        };

        match rx.try_recv() {
            Ok(Ok(sender)) => {
                // Sender is ready, create capture and start streaming
                if let Some(rt) = &self.tokio_runtime {
                    let w = self.environment.width();
                    let h = self.environment.height();
                    log::info!("📡 OMT: Creating capture pipeline for {}x{}", w, h);

                    let mut capture = crate::network::OmtCapture::new(&self.device, w, h);
                    capture.set_target_fps(self.settings.omt_capture_fps);
                    capture.start_sender_thread(sender, rt.handle().clone());
                    self.omt_capture = Some(capture);
                    self.omt_broadcast_enabled = true;
                    self.menu_bar.set_status(format!("OMT broadcast started ({}x{})", w, h));
                    log::info!("📡 OMT: Capture pipeline started, broadcasting {}x{} on port 5970", w, h);
                }
            }
            Ok(Err(e)) => {
                // Sender failed to start
                self.omt_broadcast_enabled = false;
                self.settings.omt_broadcast_enabled = false;
                self.menu_bar.set_status(format!("OMT broadcast failed: {}", e));
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                // Still waiting, put the receiver back
                self.pending_omt_sender = Some(rx);
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                // Thread crashed or was dropped
                self.omt_broadcast_enabled = false;
                self.settings.omt_broadcast_enabled = false;
                self.menu_bar.set_status("OMT broadcast failed");
            }
        }
    }

    /// Stop OMT broadcast
    pub fn stop_omt_broadcast(&mut self) {
        if self.omt_capture.is_some() {
            // Drop capture - this stops the sender thread
            self.omt_capture = None;
            self.omt_sender = None;
            self.omt_broadcast_enabled = false;
            log::info!("📡 OMT: Stopped broadcast");
            self.menu_bar.set_status("OMT broadcast stopped");
        }
    }

    /// Check if OMT broadcast is active
    pub fn is_omt_broadcasting(&self) -> bool {
        self.omt_capture.as_ref().map(|c| c.is_sender_running()).unwrap_or(false)
    }

    /// Check if OMT broadcast is starting (pending)
    pub fn is_omt_starting(&self) -> bool {
        self.pending_omt_sender.is_some()
    }

    // =========================================
    // Syphon/Spout Texture Sharing
    // =========================================

    /// Start texture sharing via Syphon (macOS) or Spout (Windows)
    #[cfg(target_os = "macos")]
    pub fn start_texture_sharing(&mut self) {
        use crate::network::TextureSharer;
        use metal::foreign_types::ForeignType;
        use std::ffi::c_void;

        if self.texture_sharer.is_some() {
            log::info!("Syphon: Already sharing");
            return;
        }

        // Extract Metal device and create command queue from wgpu
        let metal_result: Option<(*mut c_void, metal::CommandQueue)> = unsafe {
            self.device
                .as_hal::<wgpu_hal::api::Metal, _, _>(|metal_device| {
                    metal_device.map(|dev| {
                        // Get the raw Metal device
                        let raw_device = dev.raw_device();
                        let device_guard = raw_device.lock();
                        let device_ptr = device_guard.as_ptr() as *mut c_void;

                        // Create a command queue for Syphon
                        let queue = device_guard.new_command_queue();
                        (device_ptr, queue)
                    })
                })
        };

        let (metal_device_ptr, command_queue) = match metal_result {
            Some((ptr, queue)) => (ptr, queue),
            None => {
                log::error!("Syphon: Failed to get Metal device from wgpu");
                self.menu_bar
                    .set_status("Syphon error: Not running on Metal backend");
                return;
            }
        };

        // Create the Syphon sharer
        let mut sharer = crate::network::SyphonSharer::new();

        // Set the texture dimensions to share
        sharer.set_dimensions(self.environment.width(), self.environment.height());

        // Set the Metal device handle
        unsafe {
            sharer.set_metal_handles(metal_device_ptr, std::ptr::null_mut());
        }

        // Start sharing with a server name
        let name = format!(
            "Immersive Server - {}",
            self.current_file
                .as_ref()
                .and_then(|p| p.file_stem())
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "Untitled".to_string())
        );

        match sharer.start(&name) {
            Ok(()) => {
                self.texture_sharer = Some(sharer);
                self.metal_command_queue = Some(command_queue);
                self.texture_share_enabled = true;
                log::info!("Syphon: Started sharing as '{}'", name);
                self.menu_bar
                    .set_status(&format!("Syphon: Sharing as '{}'", name));
            }
            Err(e) => {
                log::error!("Syphon: Failed to start: {}", e);
                self.menu_bar.set_status(&format!("Syphon error: {}", e));
            }
        }
    }

    #[cfg(target_os = "windows")]
    pub fn start_texture_sharing(&mut self) {
        if self.spout_capture.is_some() {
            log::info!("Spout: Already sharing");
            return;
        }

        // Create the Spout capture
        let mut capture = crate::network::SpoutCapture::new(
            &self.device,
            self.environment.width(),
            self.environment.height(),
        );

        // Start sharing with a server name
        let name = format!(
            "Immersive Server - {}",
            self.current_file
                .as_ref()
                .and_then(|p| p.file_stem())
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "environment".to_string())
        );

        match capture.start(&name) {
            Ok(()) => {
                self.spout_capture = Some(capture);
                self.texture_share_enabled = true;
                log::info!("Spout: Started sharing as '{}'", name);
                self.menu_bar.set_status(&format!("Spout: Sharing as '{}'", name));
            }
            Err(e) => {
                log::error!("Spout: Failed to start: {}", e);
                self.menu_bar.set_status(&format!("Spout error: {}", e));
            }
        }
    }

    /// Stop texture sharing
    #[cfg(target_os = "macos")]
    pub fn stop_texture_sharing(&mut self) {
        use crate::network::TextureSharer;

        if let Some(ref mut sharer) = self.texture_sharer {
            sharer.stop();
        }
        self.texture_sharer = None;
        self.metal_command_queue = None;
        self.texture_share_enabled = false;
        log::info!("Syphon: Stopped sharing");
        self.menu_bar.set_status("Syphon: Stopped");
    }

    #[cfg(target_os = "windows")]
    pub fn stop_texture_sharing(&mut self) {
        if let Some(ref mut capture) = self.spout_capture {
            capture.stop();
        }
        self.spout_capture = None;
        self.texture_share_enabled = false;
        log::info!("Spout: Stopped sharing");
        self.menu_bar.set_status("Spout: Stopped");
    }

    /// Get the number of discovered OMT sources
    pub fn omt_source_count(&self) -> usize {
        self.omt_discovery.as_ref().map(|d| d.source_count()).unwrap_or(0)
    }

    /// Initialize default OMT broadcast of the environment
    ///
    /// Call this after App is created to start broadcasting the environment
    /// as an OMT source on the network (default port 9000).
    pub fn init_omt_broadcast(&mut self) {
        if self.omt_broadcast_enabled && self.omt_sender.is_none() {
            let w = self.environment.width();
            let h = self.environment.height();
            let name = format!("Immersive Server ({}×{})", w, h);
            self.start_omt_broadcast(&name, 9000);
        }
    }

    /// Copy a clip to the clipboard
    pub fn copy_clip(&mut self, layer_id: u32, slot: usize) {
        if let Some(layer) = self.environment.get_layer(layer_id) {
            if let Some(clip) = layer.get_clip(slot) {
                self.clip_grid_panel.copy_clip(clip.clone());
                log::info!("📋 Copied clip from layer {} slot {}", layer_id, slot);
                self.menu_bar.set_status(format!("Copied clip: {}", clip.display_name()));
            }
        }
    }

    /// Paste a clip from the clipboard to a slot
    pub fn paste_clip(&mut self, layer_id: u32, slot: usize) {
        if let Some(clip) = self.clip_grid_panel.get_clipboard().cloned() {
            if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                layer.set_clip(slot, clip.clone());
                log::info!("📋 Pasted clip to layer {} slot {}", layer_id, slot);
                self.menu_bar.set_status(format!("Pasted clip: {}", clip.display_name()));
            }
        }
    }

    /// Clone (duplicate) an entire layer
    ///
    /// Creates a copy of the layer with all its clips and settings.
    /// The new layer gets a unique ID and " Copy" suffix on the name.
    /// If the original has active playback, the clone loads the same video independently.
    pub fn clone_layer(&mut self, layer_id: u32) {
        // Get the source layer data
        let (new_layer, active_clip_path) = {
            let Some(source_layer) = self.environment.get_layer(layer_id) else {
                log::warn!("Cannot clone layer {}: not found", layer_id);
                return;
            };

            // Find next available layer ID
            let next_id = self.environment.layers()
                .iter()
                .map(|l| l.id)
                .max()
                .map(|id| id + 1)
                .unwrap_or(1);

            // Clone the layer with new ID and name
            let mut cloned = source_layer.clone();
            cloned.id = next_id;
            cloned.name = format!("{} Copy", source_layer.name);
            // Reset runtime state (source and active_clip are runtime, not saved)
            cloned.source = crate::compositor::LayerSource::None;
            cloned.active_clip = None;

            // If the source has an active clip, get its path so we can load it independently
            let active_path = if let Some(active_slot) = source_layer.active_clip {
                source_layer.get_clip(active_slot)
                    .map(|c| c.source_path.clone())
            } else {
                None
            };

            (cloned, active_path)
        };

        let new_id = new_layer.id;
        let new_name = new_layer.name.clone();

        // Add the cloned layer to the environment
        self.environment.add_existing_layer(new_layer);

        // If the original was playing, load the same video on the clone
        if let Some(path) = active_clip_path {
            if let Err(e) = self.load_layer_video(new_id, &path) {
                log::warn!("Failed to load video for cloned layer: {}", e);
            } else {
                // Mark the first clip slot as active if it matches
                if let Some(layer) = self.environment.get_layer_mut(new_id) {
                    // Find which slot has this path
                    for (slot, clip) in layer.clips.iter().enumerate() {
                        if let Some(c) = clip {
                            if c.source_path == path {
                                layer.active_clip = Some(slot);
                                layer.source = crate::compositor::LayerSource::Video(path.clone());
                                break;
                            }
                        }
                    }
                }
            }
        }

        log::info!("📋 Cloned layer {} -> {} ({})", layer_id, new_id, new_name);
        self.menu_bar.set_status(format!("Cloned layer: {}", new_name));
    }

    /// Set the transition mode for a layer
    pub fn set_layer_transition(
        &mut self,
        layer_id: u32,
        transition: crate::compositor::ClipTransition,
    ) {
        if let Some(layer) = self.environment.get_layer_mut(layer_id) {
            layer.transition = transition;
            log::info!(
                "Set transition for layer {} to {:?}",
                layer_id,
                transition.name()
            );
        }
    }

    /// Check if a clip is active on a layer at the given slot
    pub fn is_clip_active(&self, layer_id: u32, slot: usize) -> bool {
        self.environment
            .get_layer(layer_id)
            .map(|l| l.active_clip == Some(slot))
            .unwrap_or(false)
    }

    /// Get the active clip slot for a layer, if any
    pub fn active_clip_slot(&self, layer_id: u32) -> Option<usize> {
        self.environment
            .get_layer(layer_id)
            .and_then(|l| l.active_clip)
    }

    /// Handle a clip grid action from the UI
    fn handle_clip_action(&mut self, action: crate::ui::ClipGridAction) {
        use crate::ui::ClipGridAction;

        match action {
            ClipGridAction::TriggerClip { layer_id, slot } => {
                if let Err(e) = self.trigger_clip(layer_id, slot) {
                    log::error!("Failed to trigger clip: {}", e);
                    self.menu_bar.set_status(format!("Failed to trigger clip: {}", e));
                }
                // Select the clip in the properties panel
                self.properties_panel.select_clip(layer_id, slot);
                // Ensure properties panel is visible
                if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::PROPERTIES) {
                    p.open = true;
                }
            }
            ClipGridAction::StopClip { layer_id } => {
                self.stop_clip_with_fade(layer_id);
            }
            ClipGridAction::AssignClip { layer_id, slot } => {
                // Mark that we're waiting for a file to be assigned
                self.clip_grid_panel.set_pending_assignment(layer_id, slot);
                // Request file picker via menu_bar
                self.menu_bar.pending_action = Some(crate::ui::menu_bar::FileAction::OpenVideo);
            }
            ClipGridAction::AssignClipWithPath { layer_id, slot, path } => {
                // Direct assignment from drag-drop
                let label = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string());
                if self.set_layer_clip(layer_id, slot, path, label) {
                    log::info!("Assigned clip to layer {} at slot {} via drag-drop", layer_id, slot);
                    self.menu_bar.set_status(format!("Assigned clip to slot {}", slot + 1));
                }
            }
            ClipGridAction::ClearClip { layer_id, slot } => {
                self.clear_layer_clip(layer_id, slot);
            }
            ClipGridAction::SetLayerTransition { layer_id, transition } => {
                self.set_layer_transition(layer_id, transition);
            }
            ClipGridAction::AddLayer => {
                self.add_layer();
            }
            ClipGridAction::DeleteLayer { layer_id } => {
                self.delete_layer(layer_id);
            }
            ClipGridAction::AddColumn => {
                self.add_column();
            }
            ClipGridAction::DeleteColumn { column_index } => {
                self.delete_column(column_index);
            }
            ClipGridAction::SetLayerOpacity { layer_id, opacity } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    layer.set_opacity(opacity);
                }
            }
            ClipGridAction::CopyClip { layer_id, slot } => {
                self.copy_clip(layer_id, slot);
            }
            ClipGridAction::PasteClip { layer_id, slot } => {
                self.paste_clip(layer_id, slot);
            }
            ClipGridAction::CloneLayer { layer_id } => {
                self.clone_layer(layer_id);
            }
            ClipGridAction::SelectLayer { layer_id } => {
                // Select this layer in the properties panel
                self.properties_panel.select_layer(layer_id);
                // Ensure properties panel is visible
                if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::PROPERTIES) {
                    p.open = true;
                }
            }
            ClipGridAction::AssignOmtSource { layer_id, slot, address, name } => {
                // Assign an OMT source to a clip slot
                self.set_layer_omt_clip(layer_id, slot, address, name);
            }
            ClipGridAction::SelectClipForPreview { layer_id, slot } => {
                // Select clip for preview without triggering it
                // Load the clip into the preview player
                if let Some(layer) = self.environment.layers().iter().find(|l| l.id == layer_id) {
                    if let Some(clip) = layer.get_clip(slot) {
                        // Set the clip info in the preview panel
                        let source_info = match &clip.source {
                            crate::compositor::ClipSource::File { path } => path.display().to_string(),
                            crate::compositor::ClipSource::Omt { address, .. } => format!("OMT: {}", address),
                        };
                        self.preview_monitor_panel.set_preview_clip(crate::ui::PreviewClipInfo {
                            layer_id,
                            slot,
                            name: clip.display_name(),
                            source_info,
                        });

                        // Load the clip into the preview player
                        match &clip.source {
                            crate::compositor::ClipSource::File { path } => {
                                if let Err(e) = self.preview_player.load(path, &self.device, &self.video_renderer) {
                                    log::warn!("Failed to load preview: {}", e);
                                    self.menu_bar.set_status(format!("Preview failed: {}", e));
                                }
                            }
                            crate::compositor::ClipSource::Omt { .. } => {
                                log::info!("OMT preview not yet implemented");
                                self.menu_bar.set_status("OMT preview not yet supported".to_string());
                            }
                        }

                        // Select in properties panel too
                        self.properties_panel.select_clip(layer_id, slot);

                        // Open preview monitor panel
                        if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::PREVIEW_MONITOR) {
                            p.open = true;
                        }
                    }
                }
            }
        }
    }

    /// Handle a properties panel action from the UI
    fn handle_properties_action(&mut self, action: crate::ui::properties_panel::PropertiesAction) {
        use crate::ui::properties_panel::PropertiesAction;

        match action {
            PropertiesAction::SetEnvironmentSize { width, height } => {
                self.settings.environment_width = width;
                self.settings.environment_height = height;
                self.sync_environment_from_settings();
                self.menu_bar.set_status(format!("Environment size: {}×{}", width, height));
            }
            PropertiesAction::SetTargetFPS { fps } => {
                self.settings.target_fps = fps;
                self.menu_bar.set_status(format!("Target FPS set to {}", fps));
            }
            PropertiesAction::SetShowFPS { show } => {
                self.settings.show_fps = show;
            }
            PropertiesAction::SetLayerOpacity { layer_id, opacity } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    layer.set_opacity(opacity);
                }
            }
            PropertiesAction::SetLayerBlendMode { layer_id, blend_mode } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    layer.blend_mode = blend_mode;
                }
            }
            PropertiesAction::SetLayerVisibility { layer_id, visible } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    layer.visible = visible;
                }
            }
            PropertiesAction::SetLayerPosition { layer_id, x, y } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    layer.transform.position = (x, y);
                }
            }
            PropertiesAction::SetLayerScale { layer_id, scale_x, scale_y } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    layer.transform.scale = (scale_x, scale_y);
                }
            }
            PropertiesAction::SetLayerRotation { layer_id, degrees } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    layer.transform.rotation = degrees.to_radians();
                }
            }
            PropertiesAction::SetLayerTiling { layer_id, tile_x, tile_y } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    layer.set_tiling(tile_x, tile_y);
                }
            }
            PropertiesAction::SetLayerTransition { layer_id, transition } => {
                self.set_layer_transition(layer_id, transition);
            }
            PropertiesAction::SetOmtBroadcast { enabled } => {
                self.settings.omt_broadcast_enabled = enabled;
                self.omt_broadcast_enabled = enabled;
                if enabled {
                    self.start_omt_broadcast("Immersive Server", 5970);
                } else {
                    self.stop_omt_broadcast();
                }
            }
            PropertiesAction::SetOmtCaptureFps { fps } => {
                self.settings.omt_capture_fps = fps;
                if let Some(ref mut capture) = self.omt_capture {
                    capture.set_target_fps(fps);
                }
            }
            PropertiesAction::SetThumbnailMode { mode } => {
                self.settings.thumbnail_mode = mode;
                // Cache will be automatically cleared on next poll via set_mode()
            }
            PropertiesAction::SetTextureShare { enabled } => {
                self.settings.texture_share_enabled = enabled;
                self.texture_share_enabled = enabled;

                #[cfg(target_os = "macos")]
                {
                    if enabled {
                        self.start_texture_sharing();
                    } else {
                        self.stop_texture_sharing();
                    }
                }

                #[cfg(target_os = "windows")]
                {
                    if enabled {
                        self.start_texture_sharing();
                    } else {
                        self.stop_texture_sharing();
                    }
                }

                #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                {
                    log::warn!("Texture sharing not available on this platform");
                }
            }

            // Effect-related actions
            PropertiesAction::AddLayerEffect { layer_id, effect_type } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    // Get default parameters for this effect type
                    if let Some(params) = self.effect_manager.registry().default_parameters(&effect_type) {
                        let display_name = self.effect_manager.registry()
                            .display_name(&effect_type)
                            .unwrap_or(&effect_type)
                            .to_string();
                        layer.effects.add(&effect_type, &display_name, params);
                        log::info!("Added effect '{}' to layer {}", display_name, layer_id);
                    }
                }
            }
            PropertiesAction::RemoveLayerEffect { layer_id, effect_id } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    layer.effects.remove(effect_id);
                    log::info!("Removed effect {} from layer {}", effect_id, layer_id);
                }
            }
            PropertiesAction::SetLayerEffectBypassed { layer_id, effect_id, bypassed } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    if let Some(effect) = layer.effects.get_mut(effect_id) {
                        effect.bypassed = bypassed;
                    }
                }
            }
            PropertiesAction::SetLayerEffectSoloed { layer_id, effect_id, soloed } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    if soloed {
                        layer.effects.solo(effect_id);
                    } else {
                        layer.effects.unsolo();
                    }
                }
            }
            PropertiesAction::SetLayerEffectParameter { layer_id, effect_id, param_name, value } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    if let Some(effect) = layer.effects.get_mut(effect_id) {
                        effect.set_parameter(&param_name, value);
                    }
                }
            }
            PropertiesAction::ReorderLayerEffect { layer_id, effect_id, new_index } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    layer.effects.move_effect(effect_id, new_index);
                }
            }

            // Clip effect actions
            PropertiesAction::AddClipEffect { layer_id, slot, effect_type } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    if let Some(clip) = layer.get_clip_mut(slot) {
                        if let Some(params) = self.effect_manager.registry().default_parameters(&effect_type) {
                            let display_name = self.effect_manager.registry()
                                .display_name(&effect_type)
                                .unwrap_or(&effect_type)
                                .to_string();
                            clip.effects.add(&effect_type, &display_name, params);
                            log::info!("Added effect '{}' to clip {} on layer {}", display_name, slot, layer_id);
                        }
                    }
                }
            }
            PropertiesAction::RemoveClipEffect { layer_id, slot, effect_id } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    if let Some(clip) = layer.get_clip_mut(slot) {
                        clip.effects.remove(effect_id);
                        log::info!("Removed effect {} from clip {} on layer {}", effect_id, slot, layer_id);
                    }
                }
            }
            PropertiesAction::SetClipEffectBypassed { layer_id, slot, effect_id, bypassed } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    if let Some(clip) = layer.get_clip_mut(slot) {
                        if let Some(effect) = clip.effects.get_mut(effect_id) {
                            effect.bypassed = bypassed;
                        }
                    }
                }
            }
            PropertiesAction::SetClipEffectSoloed { layer_id, slot, effect_id, soloed } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    if let Some(clip) = layer.get_clip_mut(slot) {
                        if soloed {
                            clip.effects.solo(effect_id);
                        } else {
                            clip.effects.unsolo();
                        }
                    }
                }
            }
            PropertiesAction::SetClipEffectParameter { layer_id, slot, effect_id, param_name, value } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    if let Some(clip) = layer.get_clip_mut(slot) {
                        if let Some(effect) = clip.effects.get_mut(effect_id) {
                            effect.set_parameter(&param_name, value);
                        }
                    }
                }
            }
            PropertiesAction::ReorderClipEffect { layer_id, slot, effect_id, new_index } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    if let Some(clip) = layer.get_clip_mut(slot) {
                        clip.effects.move_effect(effect_id, new_index);
                    }
                }
            }

            // Environment effect actions
            PropertiesAction::AddEnvironmentEffect { effect_type } => {
                if let Some(params) = self.effect_manager.registry().default_parameters(&effect_type) {
                    let display_name = self.effect_manager.registry()
                        .display_name(&effect_type)
                        .unwrap_or(&effect_type)
                        .to_string();
                    self.environment.effects_mut().add(&effect_type, &display_name, params);
                    log::info!("Added master effect '{}'", display_name);
                }
            }
            PropertiesAction::RemoveEnvironmentEffect { effect_id } => {
                self.environment.effects_mut().remove(effect_id);
                log::info!("Removed master effect {}", effect_id);
            }
            PropertiesAction::SetEnvironmentEffectBypassed { effect_id, bypassed } => {
                if let Some(effect) = self.environment.effects_mut().get_mut(effect_id) {
                    effect.bypassed = bypassed;
                }
            }
            PropertiesAction::SetEnvironmentEffectSoloed { effect_id, soloed } => {
                if soloed {
                    self.environment.effects_mut().solo(effect_id);
                } else {
                    self.environment.effects_mut().unsolo();
                }
            }
            PropertiesAction::SetEnvironmentEffectParameter { effect_id, param_name, value } => {
                if let Some(effect) = self.environment.effects_mut().get_mut(effect_id) {
                    effect.set_parameter(&param_name, value);
                }
            }
            PropertiesAction::ReorderEnvironmentEffect { effect_id, new_index } => {
                self.environment.effects_mut().move_effect(effect_id, new_index);
            }
        }
    }

    /// Handle a sources panel action from the UI
    fn handle_sources_action(&mut self, action: crate::ui::SourcesAction) {
        use crate::ui::SourcesAction;

        match action {
            SourcesAction::RefreshOmtSources => {
                self.refresh_omt_sources();
            }
        }
    }

    /// Handle a preview monitor panel action from the UI
    fn handle_preview_action(&mut self, action: crate::ui::PreviewMonitorAction) {
        use crate::ui::PreviewMonitorAction;

        match action {
            PreviewMonitorAction::TogglePlayback => {
                self.preview_player.toggle_pause();
            }
            PreviewMonitorAction::RestartPreview => {
                self.preview_player.restart();
            }
            PreviewMonitorAction::TriggerToLayer { layer_id, slot } => {
                // Trigger the previewed clip to its layer (go live)
                if let Err(e) = self.trigger_clip(layer_id, slot) {
                    log::error!("Failed to trigger clip from preview: {}", e);
                    self.menu_bar.set_status(format!("Failed to trigger clip: {}", e));
                }
            }
        }
    }

    /// Complete a pending clip assignment with a video path
    pub fn complete_clip_assignment(&mut self, path: std::path::PathBuf) {
        if let Some((layer_id, slot)) = self.clip_grid_panel.take_pending_assignment() {
            // Extract filename for label
            let label = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string());
            
            if self.set_layer_clip(layer_id, slot, path.clone(), label) {
                log::info!("Assigned clip to layer {} at slot {}", layer_id, slot);
                self.menu_bar.set_status(format!("Assigned clip to slot {}", slot + 1));
            } else {
                log::error!("Failed to assign clip to layer {} at slot {}", layer_id, slot);
            }
        } else {
            // No pending assignment - this is a regular video load (legacy)
            if let Err(e) = self.load_video(&path) {
                log::error!("Failed to load video: {}", e);
                self.menu_bar.set_status(format!("Failed: {}", e));
            }
        }
    }

    /// Check if there's a pending clip assignment
    pub fn has_pending_clip_assignment(&self) -> bool {
        self.clip_grid_panel.pending_clip_assignment.is_some()
    }

    // Legacy compatibility methods (for single-video use case)

    /// Load a video file for playback - creates a new layer
    /// This is a convenience method for single-video playback
    pub fn load_video(&mut self, path: &std::path::Path) -> Result<(), String> {
        // For backward compatibility, we create a layer called "Video"
        // Remove existing video layer if any
        if let Some(layer) = self.environment.layers().first() {
            let id = layer.id;
            self.remove_layer(id);
        }

        self.add_layer_with_video("Video", path)?;
        Ok(())
    }

    /// Update video playback - pick up decoded frames (non-blocking)
    /// Legacy method that updates all layer videos
    pub fn update_video(&mut self) {
        self.update_videos();
    }

    /// Toggle video pause state (all layers)
    pub fn toggle_video_pause(&self) {
        self.toggle_all_pause();
    }

    /// Restart video from beginning (all layers)
    pub fn restart_video(&self) {
        self.restart_all_videos();
    }

    /// Check if video is paused (any layer)
    pub fn is_video_paused(&self) -> bool {
        self.is_any_video_paused()
    }

    /// Get the current video path if loaded (first layer)
    pub fn current_video_path(&self) -> Option<&std::path::Path> {
        self.layer_runtimes
            .values()
            .next()
            .and_then(|r| r.player.as_ref().map(|p| p.path()))
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
