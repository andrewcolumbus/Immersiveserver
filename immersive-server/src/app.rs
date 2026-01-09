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

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::compositor::{Environment, LayerSource, Viewport};
use crate::layer_runtime::{LayerRuntime, TextureUpdateResult};
use crate::settings::EnvironmentSettings;
use crate::ui::MenuBar;
use crate::ui::viewport_widget::{self, ViewportConfig};
use crate::ui::{register_egui_texture, register_egui_texture_ptr, free_egui_texture};
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

/// Refresh source discovery every 2 seconds
const DISCOVERY_REFRESH_INTERVAL_SECS: f64 = 2.0;

/// Main application state holding all wgpu resources
pub struct App {
    /// Reference to the window
    window: Arc<Window>,
    /// Shared GPU context for multi-window support
    gpu: Arc<crate::gpu_context::GpuContext>,
    /// The wgpu surface for presenting rendered frames (main window)
    surface: wgpu::Surface<'static>,
    /// The wgpu device for creating GPU resources (convenience clone from gpu)
    device: wgpu::Device,
    /// The command queue for submitting GPU work (convenience clone from gpu)
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
    /// Separate viewport for floating environment window (avoids coordinate conflict with main window)
    floating_env_viewport: Viewport,
    /// Separate viewport for tiled layout environment (avoids coordinate conflict with main viewport)
    tiled_env_viewport: Viewport,
    /// Current mouse position in window pixels
    cursor_position: (f32, f32),
    /// Last frame time for viewport animation
    last_frame_time: Instant,
    /// Whether the environment is broken out to its own window
    pub environment_broken_out: bool,
    /// Whether the environment is rendered as a floating egui window (within main window)
    pub environment_floating: bool,
    /// Environment texture ID for egui rendering (when floating)
    environment_egui_texture_id: Option<egui::TextureId>,

    // Checkerboard background pipeline
    /// Render pipeline for checkerboard background
    checker_pipeline: wgpu::RenderPipeline,
    /// Uniform buffer for checker params (environment size)
    checker_params_buffer: wgpu::Buffer,
    /// Bind group for checker params
    checker_bind_group: wgpu::BindGroup,

    // Test pattern pipeline
    /// Render pipeline for test pattern
    test_pattern_pipeline: wgpu::RenderPipeline,
    /// Uniform buffer for test pattern params (environment size, time, logo size)
    test_pattern_params_buffer: wgpu::Buffer,
    /// Bind group for test pattern params and logo texture
    test_pattern_bind_group: wgpu::BindGroup,
    /// Logo dimensions for test pattern (constant, loaded from embedded PNG)
    test_pattern_logo_size: [f32; 2],

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
    /// Last time source discovery was refreshed
    last_discovery_refresh: Instant,

    // egui integration
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,

    // UI state
    pub menu_bar: MenuBar,
    /// Whether native OS menus are being used (skip egui menu bar rendering)
    pub use_native_menu: bool,
    /// Clip grid panel for triggering clips
    pub clip_grid_panel: crate::ui::ClipGridPanel,
    /// Docking manager for detachable/resizable panels (legacy)
    pub dock_manager: crate::ui::DockManager,
    /// Tiled layout manager for new grid-based panel layout
    pub tiled_layout: crate::ui::TiledLayout,
    /// Divider drag state for tiled layout
    pub divider_drag_state: crate::ui::DividerDragState,
    /// Panel drag state for moving panels between cells
    pub panel_drag_state: crate::ui::PanelDragState,
    /// Cell drag state for moving entire cells (tiles) in tiled layout
    pub cell_drag_state: crate::ui::CellDragState,
    /// Whether to use the new tiled layout (vs legacy dock manager)
    pub use_tiled_layout: bool,
    /// Pending layout choice when opening a file with different layout than preferences
    /// Contains (project_layout, project_enabled) - shown in dialog
    pub pending_layout_choice: Option<(crate::ui::TiledLayout, bool)>,
    /// Properties panel (Environment/Layer/Clip tabs)
    pub properties_panel: crate::ui::PropertiesPanel,
    /// Sources panel for drag-and-drop of OMT/NDI sources
    pub sources_panel: crate::ui::SourcesPanel,
    /// Effects browser panel for drag-and-drop of effects
    pub effects_browser_panel: crate::ui::EffectsBrowserPanel,
    /// File browser panel for drag-and-drop of video files
    pub file_browser_panel: crate::ui::FileBrowserPanel,
    /// Performance panel for FPS and timing metrics
    pub performance_panel: crate::ui::PerformancePanel,
    /// Preview monitor panel for previewing clips before triggering
    pub preview_monitor_panel: crate::ui::PreviewMonitorPanel,
    /// 3D previsualization panel
    pub previs_panel: crate::ui::PrevisPanel,
    /// Cross-window drag state for effects (enables drag-drop between undocked panels)
    pub cross_window_drag: crate::ui::CrossWindowDragState,
    /// 3D previsualization renderer
    previs_renderer: Option<crate::previs::PrevisRenderer>,
    /// Preview player for clip preview playback
    preview_player: crate::preview_player::PreviewPlayer,
    /// Layer ID for layer preview mode (None = clip preview, Some = layer preview)
    preview_layer_id: Option<u32>,
    /// NDI receiver for source preview (separate from layer runtimes)
    preview_source_receiver: Option<crate::network::NdiReceiver>,
    /// Texture for source preview frames (raw BGRA from NDI)
    preview_source_texture: Option<VideoTexture>,
    /// Output texture after GPU BGRA→RGBA conversion (for egui display)
    preview_source_output_texture: Option<wgpu::Texture>,
    /// View for the output texture
    preview_source_output_view: Option<wgpu::TextureView>,
    /// Bind group for source preview rendering
    preview_source_bind_group: Option<wgpu::BindGroup>,
    /// Params buffer for source preview rendering
    preview_source_params_buffer: Option<wgpu::Buffer>,
    /// Whether source preview has received at least one frame
    preview_source_has_frame: bool,
    /// Current preview height (updated when user resizes window)
    current_preview_height: f32,
    /// Thumbnail cache for video previews in clip grid
    pub thumbnail_cache: crate::ui::ThumbnailCache,
    /// HAP Converter window
    pub converter_window: crate::converter::ConverterWindow,
    /// Preferences window for environment settings
    pub preferences_window: crate::ui::PreferencesWindow,
    /// Advanced Output window for multi-screen configuration
    pub advanced_output_window: crate::ui::AdvancedOutputWindow,
    /// Available displays for output selection (updated from DisplayManager)
    available_displays: Vec<crate::output::DisplayInfo>,
    /// Layout preset manager for saving/restoring UI arrangements
    pub layout_preset_manager: crate::ui::LayoutPresetManager,

    // Settings
    pub settings: EnvironmentSettings,
    pub app_preferences: crate::settings::AppPreferences,
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
    /// Scrub state: tracks which layers were playing before scrubbing started
    /// Key is layer ID, value is true if layer was playing (not paused) before scrub
    scrub_was_playing: HashMap<u32, bool>,
    /// Scrub state for preview player: was video playing before scrub started?
    scrub_was_playing_preview: bool,
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

    // NDI (Network Device Interface) networking
    /// NDI sender for broadcasting the environment
    ndi_sender: Option<crate::network::NdiSender>,
    /// NDI capture for GPU texture readback
    ndi_capture: Option<crate::network::NdiCapture>,
    /// Whether NDI sender is enabled (broadcasts environment)
    ndi_broadcast_enabled: bool,

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

    // Audio system (for FFT-reactive effects)
    /// Audio manager for FFT analysis of audio sources
    audio_manager: crate::audio::AudioManager,

    // Advanced Output system
    /// Output manager for multi-screen projection mapping
    output_manager: Option<crate::output::OutputManager>,
    /// Output preset manager for saving/restoring output configurations
    pub output_preset_manager: crate::output::OutputPresetManager,

    // REST API server
    /// Whether the API server is running
    api_server_running: bool,
    /// Shared state for API handlers
    api_shared_state: Option<crate::api::SharedStateHandle>,
    /// Command receiver for API commands
    api_command_rx: Option<tokio::sync::mpsc::UnboundedReceiver<crate::api::ApiCommand>>,
    /// Shutdown signal for graceful API server termination
    api_shutdown_tx: Option<tokio::sync::watch::Sender<bool>>,

    // Performance profiling
    /// Frame profiler for CPU timing statistics
    frame_profiler: crate::telemetry::FrameProfiler,
    /// GPU profiler for render pass timing
    gpu_profiler: crate::telemetry::GpuProfiler,
    /// Video texture upload time last frame (milliseconds)
    video_frame_time_ms: f64,
    /// UI rendering time last frame (milliseconds)
    ui_frame_time_ms: f64,

    // Frame pacing (GPU double-buffering)
    /// Tracks submission indices of frames currently in flight to the GPU.
    /// Used to prevent unbounded GPU queue growth and control latency vs stability tradeoff.
    frames_in_flight: VecDeque<wgpu::SubmissionIndex>,
    /// Last known low_latency_mode value, for detecting changes and reconfiguring surface.
    last_low_latency_mode: bool,
    /// Last known vsync_enabled value, for detecting changes and reconfiguring surface.
    last_vsync_mode: bool,
}

impl App {
    /// Create a new App instance with initialized wgpu context
    pub async fn new(window: Arc<Window>, settings: EnvironmentSettings) -> Self {
        let size = window.inner_size();

        // Load app preferences (for tiled layout, etc.)
        let app_preferences = crate::settings::AppPreferences::load();

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

        tracing::info!("Using GPU: {}", adapter.get_info().name);
        tracing::info!("Backend: {:?}", adapter.get_info().backend);

        // Request BC texture compression for GPU-native codecs (HAP/DXV)
        let bc_texture_supported = adapter.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let mut required_features = wgpu::Features::empty();
        if bc_texture_supported {
            required_features |= wgpu::Features::TEXTURE_COMPRESSION_BC;
            tracing::info!("BC texture compression enabled (for HAP/DXV support)");
        } else {
            tracing::warn!("BC texture compression not available - HAP/DXV will use software decode");
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

        tracing::info!("Surface format: {:?}", surface_format);

        // Select present mode based on vsync setting:
        // - VSYNC enabled: Use Fifo (syncs to display refresh rate)
        // - VSYNC disabled: Use Immediate for manual FPS control
        let initial_vsync_mode = settings.vsync_enabled;
        let present_mode = if initial_vsync_mode {
            wgpu::PresentMode::Fifo
        } else if surface_caps.present_modes.contains(&wgpu::PresentMode::Immediate) {
            wgpu::PresentMode::Immediate
        } else if surface_caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::Fifo
        };

        tracing::info!("Present mode: {:?} (vsync: {})", present_mode, initial_vsync_mode);

        // Configure frame latency based on low_latency_mode setting:
        // - Low latency mode (true): 1 frame buffer, ~16ms less latency but may stutter
        // - Smooth mode (false): 2 frame buffer, more stable pacing
        let initial_low_latency_mode = settings.low_latency_mode;
        let desired_frame_latency = if initial_low_latency_mode { 1 } else { 2 };

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: desired_frame_latency,
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
            tracing::info!("Created 4 default layers with {} clip slots each", clip_count);
        }

        // Create checkerboard background pipeline
        let (checker_pipeline, checker_params_buffer, checker_bind_group) =
            Self::create_checker_pipeline(&device, &queue, surface_format, env_width, env_height);

        // Create test pattern pipeline
        let (test_pattern_pipeline, test_pattern_params_buffer, test_pattern_bind_group, test_pattern_logo_size) =
            Self::create_test_pattern_pipeline(&device, &queue, surface_format, env_width, env_height);

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

        // Add NotoSans font for better Unicode coverage (geometric shapes, braille, etc.)
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "NotoSans".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/fonts/NotoSans-Regular.ttf"))),
        );
        // Add as fallback for proportional fonts
        fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap()
            .insert(1, "NotoSans".to_owned());
        // Add as fallback for monospace fonts
        fonts.families.get_mut(&egui::FontFamily::Monospace).unwrap()
            .insert(1, "NotoSans".to_owned());
        egui_ctx.set_fonts(fonts);

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

        // Initialize 3D previs renderer
        let previs_renderer = crate::previs::PrevisRenderer::new(&device);

        // Initialize shader hot-reload watcher
        let shader_watcher = match crate::shaders::ShaderWatcher::new() {
            Ok(watcher) => Some(watcher),
            Err(e) => {
                tracing::warn!("Failed to initialize shader watcher: {:?}", e);
                None
            }
        };

        let now = Instant::now();
        let initial_target_fps = settings.target_fps as f64;
        let texture_share_enabled = settings.texture_share_enabled;
        let gpu_profiler = crate::telemetry::GpuProfiler::new(&device, &queue, 32);

        // Initialize NDI buffer capacity from settings
        crate::network::ndi::set_ndi_buffer_capacity(settings.ndi_buffer_capacity);

        // Create shared GPU context for multi-window support
        // Note: instance and adapter are moved into GpuContext, device/queue are cloned for convenience
        let gpu = Arc::new(crate::gpu_context::GpuContext::from_parts(
            instance,
            adapter,
            device.clone(),
            queue.clone(),
            surface_format,
            bc_texture_supported,
        ));

        // Extract discovery settings before moving settings into the struct
        let omt_discovery_enabled = settings.omt_discovery_enabled;

        Self {
            window,
            gpu,
            surface,
            device,
            queue,
            config,
            size,
            bc_texture_supported,
            environment,
            viewport: Viewport::new(),
            floating_env_viewport: Viewport::new(),
            tiled_env_viewport: Viewport::new(),
            cursor_position: (0.0, 0.0),
            last_frame_time: now,
            environment_broken_out: false,
            environment_floating: true,  // Default to floating egui window
            environment_egui_texture_id: None,
            checker_pipeline,
            checker_params_buffer,
            checker_bind_group,
            test_pattern_pipeline,
            test_pattern_params_buffer,
            test_pattern_bind_group,
            test_pattern_logo_size,
            copy_bind_group_layout,
            copy_bind_group,
            copy_pipeline,
            sampler,
            copy_params_buffer,
            ui_frame_count: 0,
            last_ui_fps_update: now,
            ui_frames_since_update: 0,
            ui_fps: initial_target_fps, // Initialize to target so display isn't 0
            last_discovery_refresh: now,
            egui_ctx,
            egui_state,
            egui_renderer,
            menu_bar,
            use_native_menu: false, // Set to true by main.rs when native menu is active
            clip_grid_panel: crate::ui::ClipGridPanel::new(),
            dock_manager: {
                let mut dm = crate::ui::DockManager::new();
                // Register the standard panels with their default dock zones
                // Note: Drag-drop panels have can_undock=false because egui's DragAndDrop
                // API uses context-local payloads that don't work across separate windows
                dm.register_panel(crate::ui::DockablePanel::new_extended(
                    crate::ui::dock::panel_ids::CLIP_GRID,
                    "Clip Grid",
                    crate::ui::DockZone::Right,
                    false,  // can_undock - drop target for sources/files
                    false,  // requires_gpu_rendering
                    crate::ui::dock::PanelCategory::General,
                ));
                dm.register_panel(crate::ui::DockablePanel::new_extended(
                    crate::ui::dock::panel_ids::PROPERTIES,
                    "Properties",
                    crate::ui::DockZone::Left,
                    false,  // can_undock - drop target for effects + drag for reordering
                    false,
                    crate::ui::dock::PanelCategory::General,
                ));
                dm.register_panel(crate::ui::DockablePanel::new_extended(
                    crate::ui::dock::panel_ids::SOURCES,
                    "Sources",
                    crate::ui::DockZone::Left,
                    false,  // can_undock - drag source for OMT/NDI/File
                    false,
                    crate::ui::dock::PanelCategory::General,
                ));
                dm.register_panel(crate::ui::DockablePanel::new_extended(
                    crate::ui::dock::panel_ids::EFFECTS_BROWSER,
                    "Effects",
                    crate::ui::DockZone::Left,
                    false,  // can_undock - drag source for effects
                    false,
                    crate::ui::dock::PanelCategory::General,
                ));
                dm.register_panel(crate::ui::DockablePanel::new_extended(
                    crate::ui::dock::panel_ids::FILES,
                    "Files",
                    crate::ui::DockZone::Left,
                    false,  // can_undock - drag source for video/image files
                    false,
                    crate::ui::dock::PanelCategory::General,
                ));
                dm.register_panel({
                    let mut panel = crate::ui::DockablePanel::new(
                        crate::ui::dock::panel_ids::PREVIEW_MONITOR,
                        "Preview Monitor",
                        crate::ui::DockZone::Floating,
                    );
                    panel.floating_geometry.size = (1000.0, 500.0);
                    panel.floating_size = Some((1000.0, 500.0));
                    panel
                });
                dm.register_panel(crate::ui::DockablePanel::new(
                    crate::ui::dock::panel_ids::PREVIS,
                    "3D Previs",
                    crate::ui::DockZone::Floating,
                ));
                dm
            },
            tiled_layout: app_preferences.tiled_layout.clone()
                .unwrap_or_else(crate::ui::TiledLayout::default_layout),
            divider_drag_state: crate::ui::DividerDragState::default(),
            panel_drag_state: crate::ui::PanelDragState::default(),
            cell_drag_state: crate::ui::CellDragState::default(),
            use_tiled_layout: app_preferences.use_tiled_layout,
            pending_layout_choice: None,
            properties_panel: crate::ui::PropertiesPanel::new(),
            sources_panel: crate::ui::SourcesPanel::new(),
            effects_browser_panel: crate::ui::EffectsBrowserPanel::new(),
            file_browser_panel: crate::ui::FileBrowserPanel::new(),
            performance_panel: crate::ui::PerformancePanel::new(),
            preview_monitor_panel: crate::ui::PreviewMonitorPanel::new(),
            previs_panel: crate::ui::PrevisPanel::new(),
            cross_window_drag: crate::ui::CrossWindowDragState::new(),
            previs_renderer: Some(previs_renderer),
            preview_player: crate::preview_player::PreviewPlayer::new(bc_texture_supported),
            preview_layer_id: None,
            preview_source_receiver: None,
            preview_source_texture: None,
            preview_source_output_texture: None,
            preview_source_output_view: None,
            preview_source_bind_group: None,
            preview_source_params_buffer: None,
            preview_source_has_frame: false,
            current_preview_height: 280.0,
            thumbnail_cache: crate::ui::ThumbnailCache::new(),
            converter_window: crate::converter::ConverterWindow::new(),
            preferences_window: crate::ui::PreferencesWindow::new(),
            advanced_output_window: crate::ui::AdvancedOutputWindow::new(),
            available_displays: Vec::new(),
            layout_preset_manager: {
                let mut manager = crate::ui::LayoutPresetManager::new();
                manager.load_user_presets();
                manager
            },
            settings,
            app_preferences,
            current_file: None,
            video_renderer,
            layer_runtimes: HashMap::new(),
            pending_runtimes: HashMap::new(),
            pending_transition: HashMap::new(),
            scrub_was_playing: HashMap::new(),
            scrub_was_playing_preview: false,
            last_upload_layer: 0,
            shader_watcher,

            // Initialize OMT networking
            omt_discovery: Self::create_omt_discovery(omt_discovery_enabled),
            omt_sender: None,
            omt_capture: None,
            omt_broadcast_enabled: false, // Disabled by default - enable via menu
            tokio_runtime: Self::create_tokio_runtime(),
            pending_omt_sender: None,

            // NDI networking
            ndi_sender: None,
            ndi_capture: None,
            ndi_broadcast_enabled: false,

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

            // Audio (for FFT-reactive effects)
            audio_manager: {
                let mut manager = crate::audio::AudioManager::new();
                // Try to initialize system audio, but don't fail if unavailable
                if let Err(e) = manager.init_system_audio() {
                    tracing::warn!("Could not initialize system audio: {}", e);
                }
                manager
            },

            // Advanced Output system
            output_manager: None, // Initialized lazily when screens are added
            output_preset_manager: {
                let mut manager = crate::output::OutputPresetManager::new();
                manager.load_user_presets();
                manager
            },

            // API server
            api_server_running: false,
            api_shared_state: None,
            api_command_rx: None,
            api_shutdown_tx: None,

            // Performance profiling
            frame_profiler: crate::telemetry::FrameProfiler::new(),
            gpu_profiler,
            video_frame_time_ms: 0.0,
            ui_frame_time_ms: 0.0,

            // Frame pacing (GPU double-buffering)
            frames_in_flight: VecDeque::with_capacity(3),
            last_low_latency_mode: initial_low_latency_mode,
            last_vsync_mode: initial_vsync_mode,
        }
    }

    /// Get the shared GPU context for multi-window support.
    ///
    /// This can be cloned (Arc) and shared with panel windows that need GPU rendering.
    pub fn gpu_context(&self) -> Arc<crate::gpu_context::GpuContext> {
        Arc::clone(&self.gpu)
    }

    /// Get the egui context for creating additional viewports.
    ///
    /// This is needed when creating panel windows that need egui rendering.
    pub fn egui_context(&self) -> &egui::Context {
        &self.egui_ctx
    }

    /// Get the preview texture view for registering with external egui renderers.
    ///
    /// This is needed when rendering the Preview Monitor in an undocked window,
    /// since each window has its own egui_renderer and texture IDs are not transferable.
    pub fn preview_texture_view(&self) -> Option<&wgpu::TextureView> {
        self.preview_player.texture_view()
    }

    /// Render an undocked panel's content to the given UI.
    ///
    /// This is used when a panel is displayed in its own native window.
    /// Returns (should_redock, should_close) indicating user actions.
    ///
    /// `external_preview_texture_id` is used when rendering the Preview Monitor in an undocked window.
    /// Since each window has its own egui_renderer, the main window's texture IDs are invalid.
    /// The caller should register the preview texture with the window's egui_renderer and pass the ID here.
    pub fn render_undocked_panel(
        &mut self,
        panel_id: &str,
        ui: &mut egui::Ui,
        external_preview_texture_id: Option<egui::TextureId>,
    ) -> (bool, bool) {
        use crate::ui::dock::panel_ids;

        // Small button bar at top (window title bar already shows panel name)
        let mut should_redock = false;
        let mut should_close = false;
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Close button
                if ui.small_button(crate::ui::icons::panel::CLOSE).on_hover_text("Close panel").clicked() {
                    should_close = true;
                }
                // Dock button
                if ui.small_button(format!("{} to Main", crate::ui::icons::panel::DOCK)).on_hover_text("Return panel to main window").clicked() {
                    should_redock = true;
                }
            });
        });

        // Render panel content based on panel_id
        match panel_id {
            panel_ids::SOURCES => {
                let actions = self.sources_panel.render_contents(ui);
                for action in actions {
                    self.handle_sources_action(action);
                }
            }
            panel_ids::EFFECTS_BROWSER => {
                let _actions = self.effects_browser_panel.render_contents(
                    ui,
                    self.effect_manager.registry(),
                    &mut self.cross_window_drag,
                );
                // Effects actions are typically drag-drop which is handled elsewhere
            }
            panel_ids::FILES => {
                self.file_browser_panel.render_contents(ui);
            }
            panel_ids::CLIP_GRID => {
                let layers: Vec<_> = self.environment.layers().to_vec();
                let actions = self.clip_grid_panel.render_contents(ui, &layers, &mut self.thumbnail_cache);
                for action in actions {
                    self.handle_clip_action(action);
                }
            }
            panel_ids::PROPERTIES => {
                let layers: Vec<_> = self.environment.layers().to_vec();
                let omt_broadcasting = self.is_omt_broadcasting();
                let ndi_broadcasting = self.is_ndi_broadcasting();
                let layer_video_info = self.layer_video_info();
                let actions = self.properties_panel.render(
                    ui,
                    &self.environment,
                    &layers,
                    &self.settings,
                    omt_broadcasting,
                    ndi_broadcasting,
                    self.texture_share_enabled,
                    self.api_server_running,
                    self.effect_manager.registry(),
                    self.effect_manager.bpm_clock(),
                    self.effect_manager.time(),
                    Some(&self.audio_manager),
                    &self.effect_manager,
                    &layer_video_info,
                    &mut self.cross_window_drag,
                );
                for action in actions {
                    self.handle_properties_action(action);
                }
            }
            panel_ids::PREVIEW_MONITOR => {
                // Gather state needed for preview rendering
                let has_frame = if self.preview_source_receiver.is_some() {
                    // Source preview mode - check if we have received a frame
                    self.preview_source_has_frame
                } else if let Some(layer_id) = self.preview_layer_id {
                    // Layer preview mode - check if layer runtime has a frame
                    self.layer_runtimes.get(&layer_id)
                        .map(|r| r.has_frame)
                        .unwrap_or(false)
                } else {
                    // Clip preview mode
                    self.preview_player.has_frame()
                };
                let is_playing = !self.preview_player.is_paused();
                let video_info = if self.preview_player.has_frame() {
                    self.preview_player.video_info()
                } else if self.preview_source_has_frame && self.preview_monitor_panel.current_clip().is_some() {
                    self.preview_source_output_texture.as_ref().map(|t| {
                        crate::preview_player::VideoInfo {
                            width: t.width(),
                            height: t.height(),
                            frame_rate: 0.0,
                            duration: 0.0,
                            position: 0.0,
                            frame_index: 0,
                        }
                    })
                } else {
                    self.preview_player.video_info()
                };
                let layer_dimensions = self.preview_layer_id.map(|_| {
                    (self.environment.width(), self.environment.height())
                });
                let source_dimensions = self.preview_source_receiver.as_ref()
                    .and_then(|r| {
                        let (w, h) = (r.width(), r.height());
                        if w > 0 && h > 0 { Some((w, h)) } else { None }
                    });

                // Use external texture ID if provided, otherwise fall back to main window's texture
                let texture_id = external_preview_texture_id.or(self.preview_player.egui_texture_id);

                let actions = self.preview_monitor_panel.render_contents(
                    ui,
                    has_frame,
                    is_playing,
                    video_info,
                    layer_dimensions,
                    source_dimensions,
                    self.current_preview_height,
                    |ui, rect, uv_rect| {
                        if let Some(tex_id) = texture_id {
                            ui.painter().image(tex_id, rect, uv_rect, egui::Color32::WHITE);
                        } else if self.preview_player.is_loaded() || self.preview_layer_id.is_some() || self.preview_source_receiver.is_some() {
                            // Texture not yet registered, show loading state
                            ui.painter().rect_filled(rect, 4.0, egui::Color32::from_gray(40));
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "Loading...",
                                egui::FontId::proportional(11.0),
                                egui::Color32::WHITE,
                            );
                        }
                    },
                );
                for action in actions {
                    self.handle_preview_action(action);
                }
            }
            panel_ids::PREVIS => {
                if let Some(renderer) = &mut self.previs_renderer {
                    let actions = self.previs_panel.render(ui, &self.settings.previs_settings, renderer);
                    for action in actions {
                        self.handle_previs_action(action);
                    }
                } else {
                    ui.label("3D Previs renderer not available");
                }
            }
            _ => {
                ui.label(format!("Unknown panel: {}", panel_id));
            }
        }

        (should_redock, should_close)
    }

    /// Create the Tokio runtime for async OMT operations
    fn create_tokio_runtime() -> Option<tokio::runtime::Runtime> {
        match tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
        {
            Ok(rt) => {
                tracing::info!("OMT: Tokio runtime initialized");
                Some(rt)
            }
            Err(e) => {
                tracing::warn!("OMT: Failed to create Tokio runtime: {}", e);
                None
            }
        }
    }

    /// Create the OMT discovery service
    fn create_omt_discovery(enabled: bool) -> Option<crate::network::SourceDiscovery> {
        match crate::network::SourceDiscovery::new() {
            Ok(mut discovery) => {
                // Only start browsing if enabled in settings
                if enabled {
                    if let Err(e) = discovery.start_browsing() {
                        tracing::warn!("OMT: Failed to start source discovery: {}", e);
                    } else {
                        tracing::info!("OMT: Source discovery started");
                    }
                } else {
                    tracing::info!("OMT: Source discovery disabled in settings");
                }
                Some(discovery)
            }
            Err(e) => {
                tracing::warn!("OMT: Discovery service unavailable: {}", e);
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

                    // Border around environment edge (fixed screen pixel width regardless of zoom)
                    // Convert border width from env UV to compensate for scale
                    let border_base = 0.002;
                    let border_x = border_base / params.scale.x;
                    let border_y = border_base / params.scale.y;

                    let near_edge = adjusted_uv.x < border_x ||
                                    adjusted_uv.x > 1.0 - border_x ||
                                    adjusted_uv.y < border_y ||
                                    adjusted_uv.y > 1.0 - border_y;

                    if (near_edge) {
                        let border_color = vec3<f32>(0.4, 0.4, 0.4);
                        return vec4<f32>(border_color, 1.0);
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

    /// Create test pattern pipeline for calibration/alignment
    fn create_test_pattern_pipeline(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        env_width: u32,
        env_height: u32,
    ) -> (wgpu::RenderPipeline, wgpu::Buffer, wgpu::BindGroup, [f32; 2]) {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Test Pattern Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/test_pattern.wgsl").into(),
            ),
        });

        // Load logo texture
        let logo_png_data = include_bytes!("../assets/logos/TIG_TypeLogo_1_Alpha_White.png");
        let logo_img = image::load_from_memory(logo_png_data)
            .expect("Failed to load logo PNG")
            .to_rgba8();
        let (logo_width, logo_height) = logo_img.dimensions();
        let logo_pixels = logo_img.into_raw();

        let logo_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Logo Texture"),
            size: wgpu::Extent3d {
                width: logo_width,
                height: logo_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &logo_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &logo_pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(logo_width * 4),
                rows_per_image: Some(logo_height),
            },
            wgpu::Extent3d {
                width: logo_width,
                height: logo_height,
                depth_or_array_layers: 1,
            },
        );

        let logo_texture_view = logo_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let logo_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Logo Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Test pattern params: env_size (vec2), time (f32), padding (f32), logo_size (vec2), padding (vec2)
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct TestPatternParams {
            env_size: [f32; 2],
            time: f32,
            _pad: f32,
            logo_size: [f32; 2],
            _pad2: [f32; 2],
        }

        let params = TestPatternParams {
            env_size: [env_width as f32, env_height as f32],
            time: 0.0,
            _pad: 0.0,
            logo_size: [logo_width as f32, logo_height as f32],
            _pad2: [0.0, 0.0],
        };

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Test Pattern Params Buffer"),
            size: std::mem::size_of::<TestPatternParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        queue.write_buffer(&params_buffer, 0, bytemuck::bytes_of(&params));

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Test Pattern Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Test Pattern Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&logo_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&logo_sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Test Pattern Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Test Pattern Pipeline"),
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

        let logo_size = [logo_width as f32, logo_height as f32];
        (pipeline, params_buffer, bind_group, logo_size)
    }

    /// Update test pattern params (called each frame when test pattern is active)
    fn update_test_pattern_params(&self, time: f32) {
        #[repr(C)]
        #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
        struct TestPatternParams {
            env_size: [f32; 2],
            time: f32,
            _pad: f32,
            logo_size: [f32; 2],
            _pad2: [f32; 2],
        }

        let params = TestPatternParams {
            env_size: [self.environment.width() as f32, self.environment.height() as f32],
            time,
            _pad: 0.0,
            logo_size: self.test_pattern_logo_size,
            _pad2: [0.0, 0.0],
        };

        self.queue
            .write_buffer(&self.test_pattern_params_buffer, 0, bytemuck::bytes_of(&params));
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

            tracing::debug!("Resized to {}x{}", new_size.width, new_size.height);
        }
    }

    /// Start a new frame (currently a no-op; redraw pacing is handled in `main.rs`)
    pub fn begin_frame(&mut self) {
        // Record frame timing for profiling
        self.frame_profiler.begin_frame();
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

        // Refresh source discovery periodically
        let discovery_elapsed = now.duration_since(self.last_discovery_refresh).as_secs_f64();
        if discovery_elapsed >= DISCOVERY_REFRESH_INTERVAL_SECS {
            self.last_discovery_refresh = now;
            // Only refresh if discovery is enabled
            if self.settings.omt_discovery_enabled {
                self.refresh_omt_sources();
            }
            if self.settings.ndi_discovery_enabled {
                self.refresh_ndi_sources();
            }
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

        // Resize NDI capture buffers if broadcasting
        if let Some(capture) = &mut self.ndi_capture {
            capture.resize(&self.device, desired_width, desired_height);
        }

        self.update_present_params();
        self.update_checker_params();
        self.update_layer_params_for_environment();
    }

    /// Sync low_latency_mode setting - reconfigures surface if changed
    fn sync_low_latency_mode(&mut self) {
        if self.settings.low_latency_mode != self.last_low_latency_mode {
            self.last_low_latency_mode = self.settings.low_latency_mode;

            // Update surface configuration for new frame latency
            let desired_frame_latency = if self.settings.low_latency_mode { 1 } else { 2 };
            self.config.desired_maximum_frame_latency = desired_frame_latency;
            self.surface.configure(&self.device, &self.config);

            tracing::info!(
                "Low latency mode {}: frame latency set to {}",
                if self.settings.low_latency_mode { "enabled" } else { "disabled" },
                desired_frame_latency
            );
        }
    }

    /// Sync vsync_enabled setting - reconfigures surface present mode if changed
    fn sync_vsync_mode(&mut self) {
        if self.settings.vsync_enabled != self.last_vsync_mode {
            self.last_vsync_mode = self.settings.vsync_enabled;

            // Select new present mode based on vsync setting
            let surface_caps = self.surface.get_capabilities(&self.gpu.adapter);
            let present_mode = if self.settings.vsync_enabled {
                wgpu::PresentMode::Fifo
            } else if surface_caps.present_modes.contains(&wgpu::PresentMode::Immediate) {
                wgpu::PresentMode::Immediate
            } else if surface_caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
                wgpu::PresentMode::Mailbox
            } else {
                wgpu::PresentMode::Fifo
            };

            self.config.present_mode = present_mode;
            self.surface.configure(&self.device, &self.config);

            tracing::info!(
                "VSYNC {}: present mode set to {:?}",
                if self.settings.vsync_enabled { "enabled" } else { "disabled" },
                present_mode
            );
        }
    }

    /// Sync layers, effects, screens, and layout from environment to settings (for saving)
    pub fn sync_layers_to_settings(&mut self) {
        let layers: Vec<_> = self.environment.layers().to_vec();
        self.settings.set_layers(&layers);
        // Also sync environment effects
        self.settings.effects = self.environment.effects().clone();
        // Also sync screens from output manager
        if let Some(manager) = &self.output_manager {
            self.settings.screens = manager.export_screens();
        }
        // Sync tiled layout if in tiled mode
        if self.use_tiled_layout {
            self.settings.tiled_layout = Some(self.tiled_layout.clone());
        } else {
            self.settings.tiled_layout = None;
        }
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
                            tracing::warn!("Failed to restore clip for layer {}: {}", layer_id, e);
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
            tracing::info!("No saved layers, created 4 default layers with {} clip slots each", clip_count);
        } else {
            tracing::info!("Restored {} layers from settings", self.environment.layer_count());
        }

        // Restore environment effects from settings
        *self.environment.effects_mut() = self.settings.effects.clone();
        if !self.settings.effects.is_empty() {
            tracing::info!("Restored {} master effects from settings", self.settings.effects.len());
        }
    }

    /// Check if loaded settings have a different layout than app preferences
    /// Returns true if there's a mismatch that needs user decision
    pub fn check_layout_mismatch(&mut self) -> bool {
        // Only check if the project has a saved layout
        let Some(project_layout) = &self.settings.tiled_layout else {
            return false;
        };

        // Check if we have a preference layout to compare against
        let pref_layout = &self.app_preferences.tiled_layout;

        // Simple mismatch detection: different panel counts or layouts differ
        // For a more thorough check, we'd compare the entire tree structure
        let has_mismatch = match pref_layout {
            Some(pref) => {
                let project_panels = project_layout.all_panel_ids();
                let pref_panels = pref.all_panel_ids();
                project_panels != pref_panels
            }
            None => true, // No preference layout means project has one but we don't
        };

        if has_mismatch {
            // Store the project layout for the dialog
            self.pending_layout_choice = Some((project_layout.clone(), true));
            true
        } else {
            false
        }
    }

    /// Apply the user's layout choice (from project or keep preferences)
    pub fn apply_layout_choice(&mut self, use_project_layout: bool) {
        if let Some((project_layout, project_enabled)) = self.pending_layout_choice.take() {
            if use_project_layout {
                // Apply project layout
                self.tiled_layout = project_layout;
                self.use_tiled_layout = project_enabled;
                self.menu_bar.set_status("Applied project layout");
            } else {
                // Keep preferences layout (already loaded at startup)
                self.menu_bar.set_status("Kept preferences layout");
            }
        }
    }

    /// Render the layout choice dialog if there's a pending choice
    pub fn render_layout_choice_dialog(&mut self, ctx: &egui::Context) {
        if self.pending_layout_choice.is_none() {
            return;
        }

        let mut choice: Option<bool> = None;

        egui::Window::new("Layout Difference Detected")
            .id(egui::Id::new("layout_choice_dialog"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.label("This project has a saved panel layout that differs from your preferences.");
                    ui.add_space(10.0);
                    ui.label("Which layout would you like to use?");
                    ui.add_space(15.0);

                    ui.horizontal(|ui| {
                        if ui.button("📁 Use Project Layout").clicked() {
                            choice = Some(true);
                        }
                        ui.add_space(10.0);
                        if ui.button("⚙ Keep My Preferences").clicked() {
                            choice = Some(false);
                        }
                    });
                    ui.add_space(10.0);
                });
            });

        if let Some(use_project) = choice {
            self.apply_layout_choice(use_project);
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

    /// Sync output manager from settings (after loading)
    pub fn sync_output_manager_from_settings(&mut self) {
        if self.settings.screens.is_empty() {
            // No screens configured, don't initialize output manager
            return;
        }

        // Create output manager from settings screens
        let format = self.config.format;
        let mut manager =
            crate::output::OutputManager::from_screens(self.settings.screens.clone(), format);

        // Initialize GPU runtimes
        manager.init_runtimes(&self.device);

        self.output_manager = Some(manager);
        tracing::info!(
            "Output manager initialized with {} screens",
            self.settings.screens.len()
        );
    }

    /// Get a reference to the output manager (if initialized)
    pub fn output_manager(&self) -> Option<&crate::output::OutputManager> {
        self.output_manager.as_ref()
    }

    /// Get a mutable reference to the output manager (if initialized)
    pub fn output_manager_mut(&mut self) -> Option<&mut crate::output::OutputManager> {
        self.output_manager.as_mut()
    }

    /// Ensure output manager exists with a default screen and return a mutable reference
    pub fn ensure_output_manager(&mut self) -> &mut crate::output::OutputManager {
        let env_dims = (self.environment.width(), self.environment.height());
        if self.output_manager.is_none() {
            let format = self.config.format;
            let mut manager = crate::output::OutputManager::new(format);
            // Always create a default screen
            manager.add_screen(&self.device, "Screen 1", env_dims);
            self.output_manager = Some(manager);
        }
        // Ensure there's always at least one screen
        if self.output_manager.as_ref().map(|m| m.screen_count()).unwrap_or(0) == 0 {
            if let Some(manager) = self.output_manager.as_mut() {
                manager.add_screen(&self.device, "Screen 1", env_dims);
            }
        }
        self.output_manager.as_mut().unwrap()
    }

    /// Update the list of available displays for output selection
    ///
    /// Called from main.rs when DisplayManager refreshes
    pub fn set_available_displays(&mut self, displays: Vec<crate::output::DisplayInfo>) {
        self.available_displays = displays;
    }

    /// Render a frame with egui UI
    pub fn render(&mut self) -> Result<bool, wgpu::SurfaceError> {
        // Apply surface configuration changes BEFORE acquiring surface texture.
        // Surface must be reconfigured before get_current_texture() is called.
        self.sync_low_latency_mode();
        self.sync_vsync_mode();

        // Acquire surface texture early - allows GPU to prepare it while CPU does other work.
        // This overlaps GPU surface preparation with CPU work (egui building, video polling).
        let output = self.surface.get_current_texture()?;

        // Update effect manager timing (BPM clock, frame time)
        self.effect_manager.update();

        // Update audio manager (FFT analysis for audio-reactive effects)
        self.audio_manager.update();

        // Poll for shader hot-reload (no-op in release builds)
        self.poll_shader_reload();

        // Poll for pending OMT sender start
        self.poll_pending_omt_sender();

        // Process API commands and update shared state
        self.process_api_commands();
        self.update_api_snapshot();

        // Poll for completed thumbnail generations
        self.thumbnail_cache.poll(&self.egui_ctx);

        // Sync thumbnail mode from settings (clears cache if changed)
        self.thumbnail_cache.set_mode(self.settings.thumbnail_mode);

        // Create command encoder early so we can process preview effects before egui
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // ========== LIVE SOURCE FRAME POLLING ==========
        // Poll NDI receiver and convert BGRA→RGBA BEFORE effect processing
        // This ensures the converted texture is available for effects
        if let Some(receiver) = &mut self.preview_source_receiver {
            if let Some(frame) = receiver.take_frame() {
                let (width, height) = (frame.width, frame.height);

                // Create or recreate texture if dimensions changed
                let need_new_texture = self.preview_source_texture.as_ref()
                    .map(|t| t.width() != width || t.height() != height)
                    .unwrap_or(true);

                if need_new_texture {
                    // Create input texture for raw BGRA frames from NDI
                    // Use BGRA texture format in BGRA pipeline mode
                    let texture = if self.settings.bgra_pipeline_enabled {
                        VideoTexture::new_bgra(&self.device, width, height)
                    } else {
                        VideoTexture::new(&self.device, width, height)
                    };
                    let params_buffer = self.video_renderer.create_params_buffer(&self.device);

                    // Create output texture for RGBA result (render target for GPU swizzle)
                    let output_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("Source Preview Output Texture"),
                        size: wgpu::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: self.config.format,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING
                            | wgpu::TextureUsages::COPY_SRC,
                        view_formats: &[],
                    });
                    let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());

                    self.preview_source_texture = Some(texture);
                    self.preview_source_params_buffer = Some(params_buffer);
                    self.preview_source_output_texture = Some(output_texture);
                    self.preview_source_output_view = Some(output_view);
                }

                // Upload raw BGRA frame to input texture
                if let Some(texture) = &mut self.preview_source_texture {
                    let decoded_frame = crate::video::DecodedFrame {
                        width,
                        height,
                        data: frame.data.to_vec(),
                        pts: 0.0,
                        frame_index: 0,
                        is_gpu_native: false,
                        is_bc3: false,
                    };
                    texture.upload(&self.queue, &decoded_frame);

                    // Create bind group for rendering
                    if let Some(params_buffer) = &self.preview_source_params_buffer {
                        let bind_group = self.video_renderer.create_bind_group_with_buffer(
                            &self.device,
                            texture,
                            params_buffer,
                        );
                        self.preview_source_bind_group = Some(bind_group);

                        // Set params with BGRA swizzle enabled (unless BGRA pipeline mode)
                        let layer_params = LayerParams {
                            is_bgra: if self.settings.bgra_pipeline_enabled { 0.0 } else { 1.0 },
                            ..LayerParams::default()
                        };
                        self.video_renderer.write_layer_params(&self.queue, params_buffer, &layer_params);
                    }

                    self.preview_source_has_frame = true;
                }

                // GPU render pass: convert BGRA→RGBA using main encoder (no separate submit)
                if self.preview_source_has_frame {
                    if let (Some(output_view), Some(bind_group)) = (
                        &self.preview_source_output_view,
                        &self.preview_source_bind_group,
                    ) {
                        self.video_renderer.render_with_blend(
                            &mut encoder,
                            output_view,
                            bind_group,
                            crate::compositor::BlendMode::Normal,
                            true, // clear before rendering
                        );
                    }
                }
            }
        }

        // ========== PREVIEW EFFECT PROCESSING ==========
        // Process effects for the preview clip BEFORE building egui UI
        // This ensures the egui texture ID points to the effect output when the UI is built
        // Works for both file sources (preview_player) and live sources (preview_source_receiver)
        let has_file_frame = self.preview_player.has_frame();
        let has_live_frame = self.preview_source_has_frame && self.preview_monitor_panel.current_clip().is_some();

        if has_file_frame || has_live_frame {
            if let Some(preview_clip_info) = self.preview_monitor_panel.current_clip() {
                // Get the clip's effects from the environment
                if let Some(layer) = self.environment.get_layer(preview_clip_info.layer_id) {
                    if let Some(clip) = layer.get_clip(preview_clip_info.slot) {
                        let active_effect_count = clip.effects.active_effects().count();

                        // Get dimensions and input view based on source type
                        let (width, height, input_view_ptr) = if has_file_frame {
                            let dims = self.preview_player.dimensions();
                            let view = self.preview_player.texture_view()
                                .map(|v| v as *const wgpu::TextureView);
                            (dims.0, dims.1, view)
                        } else {
                            // Live source - use the converted RGBA output texture
                            let dims = self.preview_source_output_texture.as_ref()
                                .map(|t| (t.width(), t.height()))
                                .unwrap_or((1920, 1080));
                            let view = self.preview_source_output_view.as_ref()
                                .map(|v| v as *const wgpu::TextureView);
                            (dims.0, dims.1, view)
                        };

                        if active_effect_count > 0 {
                            // 1. Ensure preview runtime exists
                            self.effect_manager.ensure_preview_runtime(
                                &self.device,
                                width,
                                height,
                                self.config.format,
                            );

                            // 2. Sync preview effects with the clip's effect stack
                            self.effect_manager.sync_preview_effects(
                                &clip.effects,
                                &self.device,
                                &self.queue,
                                self.config.format,
                            );

                            // 3. Copy source texture to preview effect input
                            // Note: Both file preview and live source output are already RGBA
                            // Preview runtime is created at source dimensions, so no size transformation needed
                            if let Some(view_ptr) = input_view_ptr {
                                if let Some(preview_runtime) = self.effect_manager.get_preview_runtime_mut() {
                                    unsafe {
                                        preview_runtime.copy_input_texture(
                                            &mut encoder,
                                            &self.device,
                                            &self.queue,
                                            &*view_ptr,
                                            false, // is_bgra: false - sources are already RGBA
                                            width, height, // source dimensions
                                            width, height, // dest dimensions (same as source for clip preview)
                                        );
                                    }
                                }
                            }

                            // 4. Process preview effects
                            let effect_params = self.effect_manager.build_params();
                            let bpm_clock = self.effect_manager.bpm_clock().clone();
                            if let Some(preview_runtime) = self.effect_manager.get_preview_runtime_mut() {
                                if let (Some(input_view), Some(output_view)) = (
                                    preview_runtime.input_view().map(|v| v as *const _),
                                    preview_runtime.output_view(active_effect_count).map(|v| v as *const _),
                                ) {
                                    unsafe {
                                        preview_runtime.process_with_automation(
                                            &mut encoder,
                                            &self.queue,
                                            &self.device,
                                            &*input_view,
                                            &*output_view,
                                            &clip.effects,
                                            &effect_params,
                                            &bpm_clock,
                                            Some(&self.audio_manager),
                                        );
                                    }
                                }
                            }

                            // 5. Update egui texture to use effect output
                            if let Some(preview_runtime) = self.effect_manager.get_preview_runtime() {
                                if let Some(output_view) = preview_runtime.output_view(active_effect_count) {
                                    register_egui_texture(
                                        &mut self.egui_renderer,
                                        &self.device,
                                        output_view,
                                        &mut self.preview_player.egui_texture_id,
                                    );
                                }
                            }
                        } else {
                            // No active effects - show raw source texture
                            if let Some(view_ptr) = input_view_ptr {
                                unsafe {
                                    register_egui_texture_ptr(
                                        &mut self.egui_renderer,
                                        &self.device,
                                        view_ptr,
                                        &mut self.preview_player.egui_texture_id,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // ========== LAYER PREVIEW TEXTURE REGISTRATION ==========
        // For layer preview mode, register the layer's texture (with effects) for egui display
        if let Some(layer_id) = self.preview_layer_id {
            if let Some(layer_runtime) = self.layer_runtimes.get(&layer_id) {
                if layer_runtime.has_frame {
                    // Get the layer's video texture
                    if let Some(texture) = &layer_runtime.texture {
                        // Get layer and its effects for preview processing
                        let layer_effects_info = self.environment.get_layer(layer_id).map(|layer| {
                            let clip_effects = layer.active_clip
                                .and_then(|slot| layer.get_clip(slot))
                                .map(|c| c.effects.clone());
                            let layer_effects = layer.effects.clone();
                            (clip_effects, layer_effects)
                        });

                        // Process effects if any are active
                        let mut use_raw_texture = true;
                        if let Some((clip_effects, layer_effects)) = layer_effects_info {
                            let clip_effect_count = clip_effects.as_ref()
                                .map(|e| e.active_effects().count())
                                .unwrap_or(0);
                            let layer_effect_count = layer_effects.active_effects().count();
                            let total_effects = clip_effect_count + layer_effect_count;

                            if total_effects > 0 {
                                // Use environment dimensions for layer preview effects
                                // This ensures effects like multiplex fill the entire canvas
                                let (width, height) = (self.environment.width(), self.environment.height());

                                // Ensure preview runtime exists
                                self.effect_manager.ensure_preview_runtime(
                                    &self.device,
                                    width,
                                    height,
                                    self.config.format,
                                );

                                // Create a combined effect stack for preview
                                // We'll process clip effects first, then layer effects
                                let mut combined_effects = crate::effects::EffectStack::new();
                                if let Some(ref clip_fx) = clip_effects {
                                    for effect in &clip_fx.effects {
                                        combined_effects.effects.push(effect.clone());
                                    }
                                }
                                for effect in &layer_effects.effects {
                                    combined_effects.effects.push(effect.clone());
                                }

                                // Sync preview effects with combined stack
                                self.effect_manager.sync_preview_effects(
                                    &combined_effects,
                                    &self.device,
                                    &self.queue,
                                    self.config.format,
                                );

                                // Copy layer texture to preview effect input
                                // In BGRA pipeline mode, no swap needed (all sources are BGRA)
                                // Video texture is positioned within environment-sized preview texture
                                let is_bgra = if self.settings.bgra_pipeline_enabled {
                                    false  // No swap needed
                                } else {
                                    layer_runtime.is_bgra() > 0.5  // Swap NDI sources
                                };
                                let video_w = layer_runtime.video_width;
                                let video_h = layer_runtime.video_height;
                                let env_w = self.environment.width();
                                let env_h = self.environment.height();
                                if let Some(preview_runtime) = self.effect_manager.get_preview_runtime_mut() {
                                    preview_runtime.copy_input_texture(
                                        &mut encoder,
                                        &self.device,
                                        &self.queue,
                                        texture.view(),
                                        is_bgra,
                                        video_w, video_h,
                                        env_w, env_h,
                                    );
                                }

                                // Process effects
                                let mut effect_params = self.effect_manager.build_params();
                                // Set size_scale for effects that need content dimensions
                                effect_params.params[26] = video_w as f32 / env_w as f32;
                                effect_params.params[27] = video_h as f32 / env_h as f32;
                                let bpm_clock = self.effect_manager.bpm_clock().clone();
                                let combined_effect_count = combined_effects.active_effects().count();
                                if let Some(preview_runtime) = self.effect_manager.get_preview_runtime_mut() {
                                    if let (Some(input_view), Some(output_view)) = (
                                        preview_runtime.input_view().map(|v| v as *const _),
                                        preview_runtime.output_view(combined_effect_count).map(|v| v as *const _),
                                    ) {
                                        unsafe {
                                            preview_runtime.process_with_automation(
                                                &mut encoder,
                                                &self.queue,
                                                &self.device,
                                                &*input_view,
                                                &*output_view,
                                                &combined_effects,
                                                &effect_params,
                                                &bpm_clock,
                                                Some(&self.audio_manager),
                                            );
                                        }
                                    }
                                }

                                // Register effect output with egui
                                if let Some(preview_runtime) = self.effect_manager.get_preview_runtime() {
                                    if let Some(output_view) = preview_runtime.output_view(combined_effect_count) {
                                        register_egui_texture(
                                            &mut self.egui_renderer,
                                            &self.device,
                                            output_view,
                                            &mut self.preview_player.egui_texture_id,
                                        );
                                        use_raw_texture = false;
                                    }
                                }
                            }
                        }

                        // No effects - register raw layer texture
                        if use_raw_texture {
                            let texture_view_ptr = texture.view() as *const wgpu::TextureView;
                            unsafe {
                                register_egui_texture_ptr(
                                    &mut self.egui_renderer,
                                    &self.device,
                                    texture_view_ptr,
                                    &mut self.preview_player.egui_texture_id,
                                );
                            }
                        }
                    }
                }
            }
        }

        // ========== SOURCE PREVIEW EGUI REGISTRATION ==========
        // Register source texture with egui ONLY if in Source mode (no effects)
        // In Clip mode, the effect processing section handles egui registration
        // Frame polling and BGRA conversion now happens in LIVE SOURCE FRAME POLLING section above
        if self.preview_source_has_frame && self.preview_monitor_panel.current_source().is_some() {
            if let Some(output_view) = &self.preview_source_output_view {
                let output_view_ptr = output_view as *const wgpu::TextureView;
                unsafe {
                    register_egui_texture_ptr(
                        &mut self.egui_renderer,
                        &self.device,
                        output_view_ptr,
                        &mut self.preview_player.egui_texture_id,
                    );
                }
            }
        }

        // Begin egui frame
        let ui_start = std::time::Instant::now();
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
            (
                crate::ui::dock::panel_ids::PREVIS,
                "3D Previs",
                self.previs_panel.open,
            ),
            (
                crate::ui::dock::panel_ids::PERFORMANCE,
                "Performance",
                self.performance_panel.open,
            ),
        ];

        // Get BPM info from effect manager
        let bpm_info = {
            let clock = self.effect_manager.bpm_clock();
            crate::ui::menu_bar::BpmInfo {
                bpm: clock.bpm(),
                beats_per_bar: clock.beats_per_bar(),
                beat_phase: clock.beat_phase(),
                current_beat_in_bar: (clock.current_beat() % clock.beats_per_bar() as f32).floor() as u32,
            }
        };

        // Render menu bar with appropriate FPS (skip when using native OS menus)
        let audio_levels = self.audio_manager.get_band_levels_with_gain(self.settings.fft_gain);
        let settings_changed = if self.use_native_menu {
            // Native menus handle most UI, but still render a slim status bar
            self.render_status_bar(&self.egui_ctx.clone(), display_fps, display_frame_time_ms);
            false
        } else {
            self.menu_bar.render(
                &self.egui_ctx,
                &mut self.settings,
                &self.current_file,
                display_fps,
                display_frame_time_ms,
                &panel_states,
                Some(&self.layout_preset_manager),
                Some(bpm_info),
                audio_levels,
            )
        };

        // Handle menu actions (e.g., toggle panel) - works with both native and egui menus
        if let Some(action) = self.menu_bar.take_menu_action() {
            match action {
                crate::ui::menu_bar::MenuAction::TogglePanel { panel_id } => {
                    // Floating-only panels (Performance, Previs) have their own open state
                    if panel_id == crate::ui::dock::panel_ids::PERFORMANCE {
                        self.performance_panel.open = !self.performance_panel.open;
                    } else if panel_id == crate::ui::dock::panel_ids::PREVIS {
                        self.previs_panel.open = !self.previs_panel.open;
                    } else if self.use_tiled_layout {
                        // In tiled mode, use TiledLayout's toggle
                        self.tiled_layout.toggle_panel(&panel_id);
                    } else {
                        // In legacy mode, use DockManager's toggle
                        self.dock_manager.toggle_panel(&panel_id);
                    }
                }
                crate::ui::menu_bar::MenuAction::OpenHAPConverter => {
                    self.converter_window.open();
                }
                crate::ui::menu_bar::MenuAction::OpenPreferences => {
                    self.preferences_window.open = true;
                }
                crate::ui::menu_bar::MenuAction::OpenAdvancedOutput => {
                    self.advanced_output_window.open = true;
                }
                crate::ui::menu_bar::MenuAction::ApplyLayoutPreset { index } => {
                    if self.layout_preset_manager.apply_preset(index, &mut self.dock_manager) {
                        if let Some(preset) = self.layout_preset_manager.get_preset(index) {
                            self.menu_bar.set_status(format!("Applied layout: {}", preset.name));
                        }
                    }
                }
                crate::ui::menu_bar::MenuAction::SaveLayout => {
                    // For now, save with a timestamp-based name. Could add dialog later.
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let name = format!("Custom Layout {}", timestamp);
                    match self.layout_preset_manager.save_current_as_preset(&name, &self.dock_manager) {
                        Ok(()) => {
                            self.menu_bar.set_status(format!("Saved layout: {}", name));
                        }
                        Err(e) => {
                            self.menu_bar.set_status(format!("Failed to save layout: {}", e));
                        }
                    }
                }
                crate::ui::menu_bar::MenuAction::LoadLayout => {
                    // Currently not implemented - would open file dialog
                    self.menu_bar.set_status("Load layout from file not yet implemented");
                }
                crate::ui::menu_bar::MenuAction::ResetLayout => {
                    // Apply the Default preset (index 0)
                    if self.layout_preset_manager.apply_preset(0, &mut self.dock_manager) {
                        self.menu_bar.set_status("Reset to default layout");
                    }
                }
                crate::ui::menu_bar::MenuAction::SetBpm { bpm } => {
                    self.effect_manager.bpm_clock_mut().set_bpm(bpm);
                    tracing::debug!("Set BPM to {:.1}", bpm);
                }
                crate::ui::menu_bar::MenuAction::TapTempo => {
                    self.effect_manager.bpm_clock_mut().tap();
                    let new_bpm = self.effect_manager.bpm_clock().bpm();
                    tracing::debug!("Tap tempo: BPM now {:.1}", new_bpm);
                }
                crate::ui::menu_bar::MenuAction::ResyncBpm => {
                    self.effect_manager.bpm_clock_mut().resync_to_bar();
                    tracing::debug!("Resync to bar start");
                }
                crate::ui::menu_bar::MenuAction::BreakoutEnvironment => {
                    // Calculate a reasonable default position and size for the environment window
                    let pos = (100.0, 100.0);
                    let size = (self.environment.width() as f32 * 0.5, self.environment.height() as f32 * 0.5);
                    self.dock_manager.request_environment_breakout(pos, size);
                    self.menu_bar.set_status("Environment viewport broken out to separate window");
                }
                crate::ui::menu_bar::MenuAction::RedockEnvironment => {
                    self.dock_manager.request_environment_redock();
                    self.menu_bar.set_status("Environment viewport returned to main window");
                }
                crate::ui::menu_bar::MenuAction::ToggleTiledLayout => {
                    self.use_tiled_layout = !self.use_tiled_layout;
                    let status = if self.use_tiled_layout {
                        "Switched to tiled layout"
                    } else {
                        "Switched to legacy dock layout"
                    };
                    self.menu_bar.set_status(status);
                    tracing::info!("Tiled layout: {}", self.use_tiled_layout);

                    // Save layout preference
                    self.app_preferences.save_tiled_layout(&self.tiled_layout, self.use_tiled_layout);
                }
                crate::ui::menu_bar::MenuAction::SplitPanel { direction, panel_id, new_first } => {
                    // Split the focused cell (or environment if no focus) with a new panel
                    if let Some(focused_cell) = self.tiled_layout.focused_cell_id() {
                        self.tiled_layout.split_cell(focused_cell, direction, panel_id.clone(), new_first);
                        self.menu_bar.set_status(&format!("Split panel with {}", panel_id));
                    } else {
                        // Default to splitting the environment cell
                        let env_cell = self.tiled_layout.get_environment_cell_id();
                        self.tiled_layout.split_cell(env_cell, direction, panel_id.clone(), new_first);
                        self.menu_bar.set_status(&format!("Split environment with {}", panel_id));
                    }
                }
            }
        }

        // Render dock zones overlay during drag operations
        self.dock_manager.render_dock_zones(&self.egui_ctx);

        // Render HAP Converter window
        self.converter_window.show(&self.egui_ctx);

        // Render Preferences window
        let omt_discovery_active = self.omt_discovery.as_ref().map(|d| d.is_running()).unwrap_or(false);
        let ndi_discovery_active = self.sources_panel.is_ndi_discovery_enabled();
        let discovered_sources = self.sources_panel.all_discovered_sources();
        let pref_actions = self.preferences_window.render(
            &self.egui_ctx,
            &self.environment,
            &self.settings,
            self.is_omt_broadcasting(),
            self.is_ndi_broadcasting(),
            self.texture_share_enabled,
            self.api_server_running,
            omt_discovery_active,
            ndi_discovery_active,
            Some(&self.audio_manager),
            &discovered_sources,
        );
        for action in pref_actions {
            self.handle_properties_action(action);
        }

        // Render Advanced Output window
        let layer_count = self.environment.layers().len();
        let env_dimensions = (self.environment.width(), self.environment.height());
        let output_actions = self.advanced_output_window.render(
            &self.egui_ctx,
            self.output_manager.as_ref(),
            &self.output_preset_manager,
            layer_count,
            &self.available_displays,
            env_dimensions,
        );
        for action in output_actions {
            self.handle_advanced_output_action(action);
        }

        // Render layout choice dialog (if a project with different layout was loaded)
        self.render_layout_choice_dialog(&self.egui_ctx.clone());

        // =====================================================================
        // TILED LAYOUT vs LEGACY DOCK MANAGER
        // =====================================================================
        // When use_tiled_layout is true, render panels in the new grid-based layout.
        // Otherwise, use the legacy dock manager with floating/docked panels.

        // Action vectors - these will be populated by panel rendering
        let mut clip_actions: Vec<crate::ui::ClipGridAction> = Vec::new();
        let mut sources_actions: Vec<crate::ui::SourcesAction> = Vec::new();
        let mut preview_actions: Vec<crate::ui::PreviewMonitorAction> = Vec::new();
        let mut previs_actions: Vec<crate::ui::PrevisAction> = Vec::new();

        if self.use_tiled_layout {
            // Compute available rect (screen minus menu bar area)
            let screen_rect = self.egui_ctx.screen_rect();
            let menu_height = if self.use_native_menu { 24.0 } else { 28.0 }; // Approximate menu/status bar height
            let available_rect = egui::Rect::from_min_max(
                egui::pos2(screen_rect.left(), screen_rect.top() + menu_height),
                screen_rect.max,
            );

            // Render the tiled layout - actions are handled inline in render_panel_content
            self.render_tiled_ui(&self.egui_ctx.clone(), available_rect);

            // Render undocked panels as floating egui windows
            let undocked = self.tiled_layout.undocked_panels().to_vec();
            let ctx = self.egui_ctx.clone();
            let mut panels_to_redock: Vec<String> = Vec::new();
            for panel_id in undocked {
                let title = self.get_panel_title(&panel_id);
                let mut open = true;
                egui::Window::new(&title)
                    .id(egui::Id::new(format!("undocked_{}", panel_id)))
                    .open(&mut open)
                    .default_size([350.0, 400.0])
                    .show(&ctx, |ui| {
                        self.render_panel_content(ui, &panel_id);
                    });
                if !open {
                    // User closed the floating window - schedule for redock
                    panels_to_redock.push(panel_id);
                }
            }
            // Redock panels after the loop to avoid borrow conflicts
            for panel_id in panels_to_redock {
                self.tiled_layout.redock_panel(&panel_id, None);
            }

            // Performance panel is always floating (not part of tiled layout)
            let perf_metrics = self.performance_metrics();
            self.performance_panel.render(&self.egui_ctx, &perf_metrics);

            // Previs panel is always floating (not part of tiled layout)
            if let Some(renderer) = &mut self.previs_renderer {
                let actions = self.previs_panel.render_floating(
                    &self.egui_ctx,
                    &self.settings.previs_settings,
                    renderer,
                );
                for action in actions {
                    self.handle_previs_action(action);
                }
            }
        } else {
        // Legacy dock manager rendering follows...

        // Render Performance panel
        let perf_metrics = self.performance_metrics();
        self.performance_panel.render(&self.egui_ctx, &perf_metrics);

        // Get layer list for property panels
        let layers: Vec<_> = self.environment.layers().to_vec();
        let omt_broadcasting = self.is_omt_broadcasting();
        let ndi_broadcasting = self.is_ndi_broadcasting();
        let layer_video_info = self.layer_video_info();

        // Render properties panel (left panel or floating) - skip if undocked to separate window
        let prop_actions = if let Some(panel) = self.dock_manager.get_panel(crate::ui::dock::panel_ids::PROPERTIES) {
            if panel.open && !panel.is_undocked() {
                let zone = panel.zone;
                let floating_pos = panel.floating_pos;
                let floating_size = panel.floating_size;
                let dock_width = panel.dock_width;
                
                match zone {
                    crate::ui::DockZone::Left => {
                        let mut actions = Vec::new();
                        egui::SidePanel::left("properties_panel")
                            .default_width(dock_width)
                            .max_width(450.0)
                            .resizable(true)
                            .show(&self.egui_ctx, |ui| {
                                // Panel header with float/undock buttons
                                ui.horizontal(|ui| {
                                    ui.heading("Properties");
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        // Undock to separate window (only if panel can be undocked)
                                        if self.dock_manager.get_panel(crate::ui::dock::panel_ids::PROPERTIES).map(|p| p.can_undock).unwrap_or(false) {
                                            if ui.button(crate::ui::icons::panel::UNDOCK).on_hover_text("Undock to separate window").clicked() {
                                                self.dock_manager.request_undock(crate::ui::dock::panel_ids::PROPERTIES);
                                            }
                                        }
                                        // Float within main window
                                        if ui.button(crate::ui::icons::panel::FLOAT).on_hover_text("Float panel").clicked() {
                                            if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::PROPERTIES) {
                                                p.zone = crate::ui::DockZone::Floating;
                                                p.floating_pos = Some((100.0, 100.0));
                                                p.floating_size = Some((300.0, 400.0));
                                            }
                                        }
                                    });
                                });
                                ui.separator();
                                actions = self.properties_panel.render(ui, &self.environment, &layers, &self.settings, omt_broadcasting, ndi_broadcasting, self.texture_share_enabled, self.api_server_running, self.effect_manager.registry(), self.effect_manager.bpm_clock(), self.effect_manager.time(), Some(&self.audio_manager), &self.effect_manager, &layer_video_info, &mut self.cross_window_drag);
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
                                        if ui.small_button(crate::ui::icons::panel::DOCK).on_hover_text("Dock to left").clicked() {
                                            if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::PROPERTIES) {
                                                p.zone = crate::ui::DockZone::Left;
                                            }
                                        }
                                    });
                                });
                                ui.separator();
                                actions = self.properties_panel.render(ui, &self.environment, &layers, &self.settings, omt_broadcasting, ndi_broadcasting, self.texture_share_enabled, self.api_server_running, self.effect_manager.registry(), self.effect_manager.bpm_clock(), self.effect_manager.time(), Some(&self.audio_manager), &self.effect_manager, &layer_video_info, &mut self.cross_window_drag);
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
        
        // Render clip grid panel (right panel or floating) - skip if undocked
        clip_actions = if let Some(panel) = self.dock_manager.get_panel(crate::ui::dock::panel_ids::CLIP_GRID) {
            if panel.open && !panel.is_undocked() {
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
                                // Panel header with float/undock buttons
                                ui.horizontal(|ui| {
                                    ui.heading("Clip Grid");
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        // Undock to separate window (only if panel can be undocked)
                                        if self.dock_manager.get_panel(crate::ui::dock::panel_ids::CLIP_GRID).map(|p| p.can_undock).unwrap_or(false) {
                                            if ui.button(crate::ui::icons::panel::UNDOCK).on_hover_text("Undock to separate window").clicked() {
                                                self.dock_manager.request_undock(crate::ui::dock::panel_ids::CLIP_GRID);
                                            }
                                        }
                                        // Float within main window
                                        if ui.button(crate::ui::icons::panel::FLOAT).on_hover_text("Float panel").clicked() {
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
                                        if ui.small_button(crate::ui::icons::panel::DOCK).on_hover_text("Dock to right").clicked() {
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

        // Render sources panel (floating window for now) - skip if undocked
        sources_actions = if let Some(panel) = self.dock_manager.get_panel(crate::ui::dock::panel_ids::SOURCES) {
            if panel.open && !panel.is_undocked() {
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
                        // Undock button in header (only if panel can be undocked)
                        ui.horizontal(|ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if self.dock_manager.get_panel(crate::ui::dock::panel_ids::SOURCES).map(|p| p.can_undock).unwrap_or(false) {
                                    if ui.small_button(crate::ui::icons::panel::UNDOCK).on_hover_text("Undock to separate window").clicked() {
                                        self.dock_manager.request_undock(crate::ui::dock::panel_ids::SOURCES);
                                    }
                                }
                            });
                        });
                        ui.separator();
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

        // Render effects browser panel (floating window) - skip if undocked
        let _effects_actions = if let Some(panel) = self.dock_manager.get_panel(crate::ui::dock::panel_ids::EFFECTS_BROWSER) {
            if panel.open && !panel.is_undocked() {
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
                        // Undock button in header (only if panel can be undocked)
                        ui.horizontal(|ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if self.dock_manager.get_panel(crate::ui::dock::panel_ids::EFFECTS_BROWSER).map(|p| p.can_undock).unwrap_or(false) {
                                    if ui.small_button(crate::ui::icons::panel::UNDOCK).on_hover_text("Undock to separate window").clicked() {
                                        self.dock_manager.request_undock(crate::ui::dock::panel_ids::EFFECTS_BROWSER);
                                    }
                                }
                            });
                        });
                        ui.separator();
                        actions = self.effects_browser_panel.render_contents(
                            ui,
                            self.effect_manager.registry(),
                            &mut self.cross_window_drag,
                        );
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

        // Render file browser panel (floating window) - skip if undocked
        if let Some(panel) = self.dock_manager.get_panel(crate::ui::dock::panel_ids::FILES) {
            if panel.open && !panel.is_undocked() {
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
                        // Undock button in header (only if panel can be undocked)
                        ui.horizontal(|ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if self.dock_manager.get_panel(crate::ui::dock::panel_ids::FILES).map(|p| p.can_undock).unwrap_or(false) {
                                    if ui.small_button(crate::ui::icons::panel::UNDOCK).on_hover_text("Undock to separate window").clicked() {
                                        self.dock_manager.request_undock(crate::ui::dock::panel_ids::FILES);
                                    }
                                }
                            });
                        });
                        ui.separator();
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

        // Render preview monitor panel (floating window) - skip if undocked
        preview_actions = if let Some(panel) = self.dock_manager.get_panel(crate::ui::dock::panel_ids::PREVIEW_MONITOR) {
            if panel.open && !panel.is_undocked() {
                let floating_pos = panel.floating_pos;
                let floating_size = panel.floating_size;
                let mut actions = Vec::new();
                let pos = floating_pos.unwrap_or((600.0, 100.0));
                let size = floating_size.unwrap_or((320.0, 280.0));
                let mut open = true;

                // Determine has_frame based on mode (clip vs layer vs source preview)
                let has_frame = if self.preview_source_receiver.is_some() {
                    // Source preview mode - check if we have received a frame
                    self.preview_source_has_frame
                } else if let Some(layer_id) = self.preview_layer_id {
                    // Layer preview mode - check if layer runtime has a frame
                    self.layer_runtimes.get(&layer_id)
                        .map(|r| r.has_frame)
                        .unwrap_or(false)
                } else {
                    // Clip preview mode
                    self.preview_player.has_frame()
                };
                let is_playing = !self.preview_player.is_paused();
                // Get video_info - for live source clips, synthesize from source texture dimensions
                let video_info = if self.preview_player.has_frame() {
                    self.preview_player.video_info()
                } else if self.preview_source_has_frame && self.preview_monitor_panel.current_clip().is_some() {
                    // Live source clip - create synthetic video info from source dimensions
                    self.preview_source_output_texture.as_ref().map(|t| {
                        crate::preview_player::VideoInfo {
                            width: t.width(),
                            height: t.height(),
                            frame_rate: 0.0,
                            duration: 0.0,
                            position: 0.0,
                            frame_index: 0,
                        }
                    })
                } else {
                    self.preview_player.video_info()
                };

                // Get layer dimensions for layer preview mode
                // Use environment dimensions since layers are logically environment-sized
                let layer_dimensions = self.preview_layer_id.map(|_| {
                    (self.environment.width(), self.environment.height())
                });

                // Get source dimensions from receiver (available immediately after first frame)
                // rather than texture (which may not exist yet)
                let source_dimensions = self.preview_source_receiver.as_ref()
                    .and_then(|r| {
                        let (w, h) = (r.width(), r.height());
                        if w > 0 && h > 0 { Some((w, h)) } else { None }
                    });

                let window_response = egui::Window::new("Preview Monitor")
                    .id(egui::Id::new("preview_monitor_window"))
                    .default_pos(egui::pos2(pos.0, pos.1))
                    .default_size(egui::vec2(size.0, size.1))
                    .resizable(true)
                    .collapsible(true)
                    .open(&mut open)
                    .show(&self.egui_ctx, |ui| {
                        // Preview Monitor cannot be undocked (requires GPU texture access)
                        actions = self.preview_monitor_panel.render_contents(
                            ui,
                            has_frame,
                            is_playing,
                            video_info,
                            layer_dimensions,
                            source_dimensions,
                            self.current_preview_height,
                            |ui, rect, uv_rect| {
                                // Render the preview texture into the given rect with viewport-adjusted UVs
                                if let Some(texture_id) = self.preview_player.egui_texture_id {
                                    // Display the actual video/layer texture with pan/zoom UVs
                                    ui.painter().image(
                                        texture_id,
                                        rect,
                                        uv_rect,
                                        egui::Color32::WHITE,
                                    );
                                } else if self.preview_player.is_loaded() || self.preview_layer_id.is_some() || self.preview_source_receiver.is_some() {
                                    // Texture not yet registered, show loading state
                                    ui.painter().rect_filled(
                                        rect,
                                        4.0,
                                        egui::Color32::from_gray(40),
                                    );
                                    ui.painter().text(
                                        rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "Loading...",
                                        egui::FontId::proportional(11.0),
                                        egui::Color32::WHITE,
                                    );
                                }
                            },
                        );
                    });

                // Update window position for persistence (no preview height sync - causes feedback loops)
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

        // Render 3D previs panel (floating window) - skip if undocked
        previs_actions = if let Some(panel) = self.dock_manager.get_panel(crate::ui::dock::panel_ids::PREVIS) {
            if panel.open && !panel.is_undocked() {
                let floating_pos = panel.floating_pos;
                let floating_size = panel.floating_size;
                let mut actions = Vec::new();
                let pos = floating_pos.unwrap_or((700.0, 100.0));
                let size = floating_size.unwrap_or((400.0, 450.0));
                let mut open = true;

                let window_response = egui::Window::new("3D Previs")
                    .id(egui::Id::new("previs_panel_window"))
                    .default_pos(egui::pos2(pos.0, pos.1))
                    .default_size(egui::vec2(size.0, size.1))
                    .resizable(true)
                    .collapsible(true)
                    .open(&mut open)
                    .show(&self.egui_ctx, |ui| {
                        // Undock button in header
                        ui.horizontal(|ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button(crate::ui::icons::panel::UNDOCK).on_hover_text("Undock to separate window").clicked() {
                                    self.dock_manager.request_undock(crate::ui::dock::panel_ids::PREVIS);
                                }
                            });
                        });
                        ui.separator();
                        if let Some(renderer) = &mut self.previs_renderer {
                            actions = self.previs_panel.render(ui, &self.settings.previs_settings, renderer);
                        }
                    });

                // Update window position for persistence
                if let Some(resp) = &window_response {
                    let rect = resp.response.rect;
                    if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::PREVIS) {
                        p.floating_pos = Some((rect.left(), rect.top()));
                        p.floating_size = Some((rect.width(), rect.height()));
                    }
                }

                if !open {
                    if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::PREVIS) {
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

        // Render environment as floating egui window (within main window)
        if self.environment_floating && !self.environment_broken_out {
            // Calculate minimum size based on environment aspect ratio
            let env_width = self.environment.width() as f32;
            let env_height = self.environment.height() as f32;
            let env_aspect = env_width / env_height;

            // Minimum size preserves aspect ratio (base minimum ~200px on smaller dimension)
            let base_min = 200.0;
            let min_width = base_min * env_aspect.max(1.0);
            let min_height = base_min / env_aspect.min(1.0);

            let mut env_open = true;
            egui::Window::new("Environment")
                .id(egui::Id::new("environment_floating_window"))
                .default_pos(egui::pos2(300.0, 50.0))
                .default_size(egui::vec2(800.0, 600.0))
                .min_size(egui::vec2(min_width, min_height))
                .resizable(true)
                .collapsible(true)
                .open(&mut env_open)
                .show(&self.egui_ctx, |ui| {
                    // Header with Dock button
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button(crate::ui::icons::panel::DOCK)
                                .on_hover_text("Dock environment to main window")
                                .clicked()
                            {
                                self.environment_floating = false;
                                self.menu_bar.set_status("Environment docked to main window");
                            }
                            if ui.small_button(crate::ui::icons::panel::UNDOCK)
                                .on_hover_text("Undock to separate window")
                                .clicked()
                            {
                                let pos = (100.0, 100.0);
                                let size = (self.environment.width() as f32 * 0.5, self.environment.height() as f32 * 0.5);
                                self.dock_manager.request_environment_breakout(pos, size);
                                self.environment_floating = false;
                                self.menu_bar.set_status("Environment viewport broken out");
                            }
                        });
                    });
                    ui.separator();

                    // Draw environment texture with pan/zoom support
                    if let Some(tex_id) = self.environment_egui_texture_id {
                        let available = ui.available_size();
                        let env_width = self.environment.width() as f32;
                        let env_height = self.environment.height() as f32;
                        let content_size = (env_width, env_height);

                        // Allocate rect with click_and_drag for viewport input
                        let (full_rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());

                        // Handle viewport interactions (right-drag pan, scroll zoom)
                        // Uses separate floating_env_viewport to avoid coordinate conflicts with main window
                        let viewport_response = crate::ui::viewport_widget::handle_viewport_input(
                            ui,
                            &response,
                            full_rect,
                            &mut self.floating_env_viewport,
                            content_size,
                            &crate::ui::ViewportConfig::default(),
                            "floating_env",
                        );

                        if viewport_response.changed {
                            ui.ctx().request_repaint();
                        }

                        // Compute UV and dest rect with viewport transform
                        let render_info = crate::ui::viewport_widget::compute_uv_and_dest_rect(
                            &self.floating_env_viewport,
                            full_rect,
                            content_size,
                        );

                        // Fill background with black for letterboxing
                        ui.painter().rect_filled(full_rect, 0.0, egui::Color32::BLACK);

                        // Draw environment texture with computed UV rect
                        ui.painter().image(tex_id, render_info.dest_rect, render_info.uv_rect, egui::Color32::WHITE);

                        // Draw zoom indicator (shows percentage in bottom-right)
                        crate::ui::viewport_widget::draw_zoom_indicator(ui, full_rect, &self.floating_env_viewport);
                    } else {
                        // Show placeholder on first frame before texture is registered
                        ui.centered_and_justified(|ui| {
                            ui.label("Loading environment...");
                        });
                    }
                });

            // Handle window close (X button clicked)
            if !env_open {
                self.environment_floating = false;
            }
        }

        // Draw environment viewport controls overlay (top-right corner)
        // Only show when environment is docked (not floating, not broken out)
        if !self.environment_floating && !self.environment_broken_out {
            egui::Area::new(egui::Id::new("environment_viewport_controls"))
                .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-16.0, 48.0)) // Below menu bar
                .order(egui::Order::Foreground)
                .interactable(true)
                .show(&self.egui_ctx, |ui| {
                    egui::Frame::popup(ui.style())
                        .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 40, 220))
                        .corner_radius(4.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Environment").small());
                                if ui.small_button(crate::ui::icons::panel::FLOAT)
                                    .on_hover_text("Float environment as window")
                                    .clicked()
                                {
                                    self.environment_floating = true;
                                    self.menu_bar.set_status("Environment floating");
                                }
                                if ui.small_button(crate::ui::icons::panel::UNDOCK)
                                    .on_hover_text("Undock to separate window")
                                    .clicked()
                                {
                                    let pos = (100.0, 100.0);
                                    let size = (self.environment.width() as f32 * 0.5, self.environment.height() as f32 * 0.5);
                                    self.dock_manager.request_environment_breakout(pos, size);
                                    self.menu_bar.set_status("Environment viewport broken out");
                                }
                            });
                        });
                });
        }

        // Draw zoom indicator overlay on main environment preview (bottom-right corner)
        // Only show when zoomed (not at 100%) and environment is docked
        let zoom = self.viewport.zoom();
        if (zoom - 1.0).abs() > 0.01 && !self.environment_floating && !self.environment_broken_out {
            egui::Area::new(egui::Id::new("main_viewport_zoom_indicator"))
                .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
                .order(egui::Order::Tooltip)
                .interactable(false)
                .show(&self.egui_ctx, |ui| {
                    let zoom_percent = (zoom * 100.0).round() as i32;
                    let text = format!("{}%", zoom_percent);
                    let font = egui::FontId::proportional(12.0);
                    let text_color = egui::Color32::WHITE;
                    let bg_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180);

                    let padding = 8.0;
                    let galley = ui.painter().layout_no_wrap(text.clone(), font.clone(), text_color);
                    let text_size = galley.size();

                    let bg_rect = egui::Rect::from_min_size(
                        egui::pos2(0.0, 0.0),
                        egui::vec2(text_size.x + padding * 2.0, text_size.y + padding * 2.0),
                    );

                    let (rect, _) = ui.allocate_exact_size(bg_rect.size(), egui::Sense::hover());

                    ui.painter().rect_filled(rect, 4.0, bg_color);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        text,
                        font,
                        text_color,
                    );
                });
        }

        } // End of else block (legacy dock manager rendering)

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

        // Process previs panel actions
        for action in previs_actions {
            self.handle_previs_action(action);
        }

        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);

        let paint_jobs = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        self.ui_frame_time_ms = ui_start.elapsed().as_secs_f64() * 1000.0;

        // Render to Environment texture (fixed-resolution canvas)
        if self.settings.test_pattern_enabled {
            // TEST PATTERN MODE: Render test pattern instead of composition
            self.update_test_pattern_params(self.effect_manager.time());

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Test Pattern Pass"),
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
            render_pass.set_pipeline(&self.test_pattern_pipeline);
            render_pass.set_bind_group(0, &self.test_pattern_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        } else {
            // NORMAL MODE: Clear to transparent black (no checkerboard)
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Environment Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.environment.texture_view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        // 2. Render layers back-to-front (index 0 = back, last = front) - skip in test pattern mode
        if !self.settings.test_pattern_enabled {
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
                                // Use old clip transform for crossfade
                                let mut params = LayerParams::from_layer_and_clip(
                                    layer,
                                    runtime.old_clip_transform.as_ref(),
                                    runtime.old_video_width,
                                    runtime.old_video_height,
                                    self.environment.width(),
                                    self.environment.height(),
                                );
                                params.opacity = old_opacity;
                                // In BGRA pipeline mode, everything is BGRA so no swap needed
                                params.is_bgra = if self.settings.bgra_pipeline_enabled {
                                    0.0
                                } else {
                                    runtime.is_bgra()
                                };
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
                                    // Effect textures are ENVIRONMENT-SIZED so effects process at composition resolution
                                    // This allows effects like multiplex to extend beyond video bounds

                                    // Track what texture to use as input for the next stage
                                    let mut current_input_is_clip_output = false;

                                    // ========== CLIP EFFECTS ==========
                                    if has_clip_effects {
                                        if let Some(slot) = clip_slot {
                                            // 1. Ensure clip effect runtime exists (environment-sized)
                                            self.effect_manager.ensure_clip_runtime(
                                                layer.id,
                                                slot,
                                                &self.device,
                                                self.environment.width(),
                                                self.environment.height(),
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
                                            // In BGRA pipeline mode, no swap needed (all sources are BGRA)
                                            let is_bgra = if self.settings.bgra_pipeline_enabled {
                                                false  // No swap needed
                                            } else {
                                                runtime.is_bgra() > 0.5  // Swap NDI sources
                                            };
                                            if let Some(video_texture) = &runtime.texture {
                                                if let Some(clip_runtime) = self.effect_manager.get_clip_runtime(layer.id, slot) {
                                                    clip_runtime.copy_input_texture(
                                                        &mut encoder,
                                                        &self.device,
                                                        &self.queue,
                                                        video_texture.view(),
                                                        is_bgra,
                                                        runtime.video_width,
                                                        runtime.video_height,
                                                        self.environment.width(),
                                                        self.environment.height(),
                                                    );
                                                }
                                            }

                                            // 4. Process clip effects with automation (LFO/FFT)
                                            let mut effect_params = self.effect_manager.build_params();
                                            // Set size_scale for effects that need content dimensions
                                            effect_params.params[26] = runtime.video_width as f32 / self.environment.width() as f32;
                                            effect_params.params[27] = runtime.video_height as f32 / self.environment.height() as f32;
                                            let bpm_clock = self.effect_manager.bpm_clock().clone();
                                            if let Some(clip_runtime) = self.effect_manager.get_clip_runtime_mut(layer.id, slot) {
                                                if let (Some(input_view), Some(output_view)) = (
                                                    clip_runtime.input_view().map(|v| v as *const _),
                                                    clip_runtime.output_view(clip_active_effect_count).map(|v| v as *const _),
                                                ) {
                                                    if let Some(clip_effect_stack) = clip_effects {
                                                        unsafe {
                                                            clip_runtime.process_with_automation(
                                                                &mut encoder,
                                                                &self.queue,
                                                                &self.device,
                                                                &*input_view,
                                                                &*output_view,
                                                                clip_effect_stack,
                                                                &effect_params,
                                                                &bpm_clock,
                                                                Some(&self.audio_manager),
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
                                        // 1. Ensure layer effect runtime exists (environment-sized)
                                        self.effect_manager.ensure_layer_runtime(
                                            layer.id,
                                            &self.device,
                                            self.environment.width(),
                                            self.environment.height(),
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
                                            // Use clip effect output as input (effects output RGBA)
                                            // Clip effects are already environment-sized, so no size transformation needed
                                            if let Some(slot) = clip_slot {
                                                if let Some(clip_runtime) = self.effect_manager.get_clip_runtime(layer.id, slot) {
                                                    if let Some(clip_output) = clip_runtime.output_view(clip_active_effect_count) {
                                                        if let Some(layer_runtime) = self.effect_manager.get_layer_runtime(layer.id) {
                                                            let env_w = self.environment.width();
                                                            let env_h = self.environment.height();
                                                            layer_runtime.copy_input_texture(
                                                                &mut encoder,
                                                                &self.device,
                                                                &self.queue,
                                                                clip_output,
                                                                false, // is_bgra: clip effect output is RGBA
                                                                env_w, env_h, // source is already environment-sized
                                                                env_w, env_h,
                                                            );
                                                        }
                                                    }
                                                }
                                            }
                                        } else {
                                            // Use video texture directly (may be BGRA for NDI)
                                            // In BGRA pipeline mode, no swap needed
                                            let is_bgra = if self.settings.bgra_pipeline_enabled {
                                                false  // No swap needed
                                            } else {
                                                runtime.ndi_is_bgra  // Swap NDI sources
                                            };
                                            if let Some(video_texture) = &runtime.texture {
                                                if let Some(layer_runtime) = self.effect_manager.get_layer_runtime(layer.id) {
                                                    layer_runtime.copy_input_texture(
                                                        &mut encoder,
                                                        &self.device,
                                                        &self.queue,
                                                        video_texture.view(),
                                                        is_bgra,
                                                        runtime.video_width,
                                                        runtime.video_height,
                                                        self.environment.width(),
                                                        self.environment.height(),
                                                    );
                                                }
                                            }
                                        }

                                        // 4. Process layer effects with automation (LFO/FFT)
                                        let mut effect_params = self.effect_manager.build_params();
                                        // Set size_scale for effects that need content dimensions
                                        effect_params.params[26] = runtime.video_width as f32 / self.environment.width() as f32;
                                        effect_params.params[27] = runtime.video_height as f32 / self.environment.height() as f32;
                                        let bpm_clock = self.effect_manager.bpm_clock().clone();
                                        if let Some(layer_runtime) = self.effect_manager.get_layer_runtime_mut(layer.id) {
                                            if let (Some(input_view), Some(output_view)) = (
                                                layer_runtime.input_view().map(|v| v as *const _),
                                                layer_runtime.output_view(layer_active_effect_count).map(|v| v as *const _),
                                            ) {
                                                unsafe {
                                                    layer_runtime.process_with_automation(
                                                        &mut encoder,
                                                        &self.queue,
                                                        &self.device,
                                                        &*input_view,
                                                        &*output_view,
                                                        &layer.effects,
                                                        &effect_params,
                                                        &bpm_clock,
                                                        Some(&self.audio_manager),
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
                                        // Get clip transform for current clip
                                        let clip_transform = layer.active_clip
                                            .and_then(|slot| layer.get_clip(slot))
                                            .map(|clip| &clip.transform);
                                        // Effect output is environment-sized, use 1:1 size_scale
                                        // This allows effects like multiplex to extend beyond video bounds
                                        let mut composite_params = LayerParams::from_layer_and_clip(
                                            layer,
                                            clip_transform,
                                            self.environment.width(),
                                            self.environment.height(),
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
                                    // Get clip transform for current clip
                                    let clip_transform = layer.active_clip
                                        .and_then(|slot| layer.get_clip(slot))
                                        .map(|clip| &clip.transform);
                                    let mut params = LayerParams::from_layer_and_clip(
                                        layer,
                                        clip_transform,
                                        runtime.video_width,
                                        runtime.video_height,
                                        self.environment.width(),
                                        self.environment.height(),
                                    );
                                    params.opacity = effective_opacity;
                                    // In BGRA pipeline mode, everything is BGRA so no swap needed.
                                    // Otherwise, only NDI sources need R↔B swap.
                                    params.is_bgra = if self.settings.bgra_pipeline_enabled {
                                        0.0  // All sources are BGRA, no swap needed
                                    } else {
                                        runtime.is_bgra()  // Only swap NDI sources
                                    };
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
        } // End of if !self.settings.test_pattern_enabled (layer rendering)

        // ========== ENVIRONMENT EFFECTS (Master Post-Processing) ==========
        // Process environment effects AFTER all layers composited, BEFORE capture/output
        // Note: Environment effects still apply even in test pattern mode
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
                // Use automation-aware processing for FFT/LFO modulation
                self.effect_manager.process_environment_effects_with_automation(
                    &mut encoder,
                    &self.device,
                    &self.queue,
                    self.environment.texture_view(),
                    env_effects,
                    Some(&self.audio_manager),
                );
            }
        }

        // ============================================================================
        // Advanced Output Rendering
        // ============================================================================
        // Render slices to screen output textures for all enabled screens
        if let Some(output_manager) = &mut self.output_manager {
            // Ensure pipelines are created
            if !output_manager.has_pipelines() {
                output_manager.create_pipelines(&self.device);
            }

            // Collect layer texture views for SliceInput::Layer
            let layer_textures: std::collections::HashMap<u32, &wgpu::TextureView> = self
                .layer_runtimes
                .iter()
                .filter_map(|(id, rt)| rt.texture.as_ref().map(|t| (*id, t.view())))
                .collect();

            // Render each enabled screen
            let screen_ids = output_manager.enabled_screen_ids();
            for screen_id in screen_ids {
                // Render all slices to screen output
                output_manager.render_screen(
                    &self.device,
                    &self.queue,
                    &mut encoder,
                    screen_id,
                    self.environment.texture_view(),
                    &layer_textures,
                );

                // Apply per-screen color correction (if not identity)
                output_manager.apply_screen_color(
                    &self.device,
                    &self.queue,
                    &mut encoder,
                    screen_id,
                );

                // Capture frame to NDI if this screen has NDI output
                output_manager.capture_ndi_frame(&mut encoder, screen_id);

                // Capture frame to OMT if this screen has OMT output
                output_manager.capture_omt_frame(&mut encoder, screen_id);
            }
        }

        // Register Advanced Output textures with egui (if window is open)
        // NOTE: These registrations happen AFTER the egui pass, so we must NOT free old textures
        // here - the paint jobs already reference them. Just re-register (egui_wgpu handles this).
        if self.advanced_output_window.open {
            // Register environment texture for Screens tab
            let env_texture_id = self.egui_renderer.register_native_texture(
                &self.device,
                self.environment.texture_view(),
                wgpu::FilterMode::Linear,
            );
            self.advanced_output_window.environment_texture_id = Some(env_texture_id);

            // Register selected screen's output texture for Output Transformation tab
            if let Some(screen_id) = self.advanced_output_window.selected_screen_id() {
                if let Some(output_manager) = &self.output_manager {
                    if let Some(runtime) = output_manager.get_runtime(screen_id) {
                        let texture_id = self.egui_renderer.register_native_texture(
                            &self.device,
                            runtime.output_view(),
                            wgpu::FilterMode::Linear,
                        );
                        self.advanced_output_window.preview_texture_id = Some(texture_id);
                    }
                }
            } else {
                // No screen selected, clear preview
                self.advanced_output_window.preview_texture_id = None;
            }
        } else {
            // Clear textures when window is closed
            self.advanced_output_window.environment_texture_id = None;
            self.advanced_output_window.preview_texture_id = None;
        }

        // Register environment texture for floating environment window or tiled layout
        let needs_env_texture = (self.environment_floating && !self.environment_broken_out)
            || self.use_tiled_layout;
        if needs_env_texture {
            let env_texture_id = self.egui_renderer.register_native_texture(
                &self.device,
                self.environment.texture_view(),
                wgpu::FilterMode::Linear,
            );
            self.environment_egui_texture_id = Some(env_texture_id);
        } else {
            self.environment_egui_texture_id = None;
        }

        // Capture environment texture for OMT output (before we move on to present)
        if self.omt_broadcast_enabled {
            if let Some(capture) = &mut self.omt_capture {
                capture.capture_frame(&mut encoder, self.environment.texture());
            }
        }

        // Capture environment texture for NDI output
        if self.ndi_broadcast_enabled {
            if let Some(capture) = &mut self.ndi_capture {
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
                        tracing::warn!("Syphon: Failed to publish frame: {}", e);
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

        // Render 3D previs scene (if enabled and panel is open)
        if self.settings.previs_settings.enabled {
            if let Some(panel) = self.dock_manager.get_panel(crate::ui::dock::panel_ids::PREVIS) {
                if panel.open {
                    if let Some(renderer) = &mut self.previs_renderer {
                        // Ensure render target size matches panel viewport
                        let (width, height) = self.previs_panel.viewport_size();
                        renderer.ensure_render_target(&self.device, width, height);

                        // Load camera state from settings if not currently dragging
                        if !self.previs_panel.is_dragging() {
                            renderer.load_camera_state(&self.settings.previs_settings);
                        }

                        // Get floor texture from specified layer if floor mode is enabled
                        let floor_texture_view: Option<&wgpu::TextureView> = if self.settings.previs_settings.floor_enabled {
                            let floor_layer_idx = self.settings.previs_settings.floor_layer_index;
                            let layers = self.environment.layers();
                            if floor_layer_idx < layers.len() {
                                let layer_id = layers[floor_layer_idx].id;
                                self.layer_runtimes.get(&layer_id).and_then(|runtime| {
                                    runtime.texture.as_ref().map(|tex| tex.view())
                                })
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        // Render the 3D scene
                        renderer.render(
                            &mut encoder,
                            &self.device,
                            &self.queue,
                            self.environment.texture_view(),
                            floor_texture_view,
                            &self.settings.previs_settings,
                        );

                        // Register the rendered texture with egui for display
                        // NOTE: This happens after egui pass, so don't free old texture
                        if let Some(texture) = renderer.texture() {
                            let texture_id = self.egui_renderer.register_native_texture(
                                &self.device,
                                &texture.create_view(&Default::default()),
                                wgpu::FilterMode::Linear,
                            );
                            self.previs_panel.texture_id = Some(texture_id);
                        }
                    }
                }
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

        // Present Environment to the window surface (output was acquired early in render())
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

            // Only draw environment to surface if docked (not floating, not broken out)
            // When floating or broken out, show solid black background
            if !self.environment_floating && !self.environment_broken_out {
                render_pass.set_pipeline(&self.copy_pipeline);
                render_pass.set_bind_group(0, &self.copy_bind_group, &[]);
                render_pass.draw(0..3, 0..1);
            }
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

        // Submit GPU work and track the submission for frame pacing
        let submission_index = self.queue.submit(std::iter::once(encoder.finish()));
        self.frames_in_flight.push_back(submission_index);

        // Determine max frames in flight based on low latency mode setting
        // Low latency: 1 frame (~16ms less latency, may stutter under load)
        // Smooth mode: 2 frames (more stable, but ~16ms more latency)
        let max_in_flight = if self.settings.low_latency_mode { 1 } else { 2 };

        // Wait for oldest frame if exceeding max (prevents unbounded GPU queue growth)
        while self.frames_in_flight.len() > max_in_flight {
            if let Some(oldest) = self.frames_in_flight.pop_front() {
                self.device.poll(wgpu::Maintain::WaitForSubmissionIndex(oldest));
            }
        }

        output.present();

        // Process OMT capture pipeline (non-blocking)
        if self.omt_broadcast_enabled {
            if let Some(capture) = &mut self.omt_capture {
                capture.process(&self.device);
            }
        }

        // Process NDI capture pipeline (non-blocking)
        if self.ndi_broadcast_enabled {
            if let Some(capture) = &mut self.ndi_capture {
                capture.process(&self.device);
            }
        }

        // Process advanced output NDI captures (non-blocking)
        if let Some(output_manager) = &mut self.output_manager {
            output_manager.process_ndi_captures(&self.device);
        }

        // Process advanced output OMT captures (non-blocking)
        if let Some(output_manager) = &mut self.output_manager {
            output_manager.process_omt_captures(&self.device);
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

    /// Render a slim status bar when using native menus
    /// Shows FPS, BPM, and status messages without the full egui menu bar
    fn render_status_bar(&mut self, ctx: &egui::Context, fps: f64, frame_time_ms: f64) {
        // Get BPM info
        let bpm_info = {
            let clock = self.effect_manager.bpm_clock();
            crate::ui::menu_bar::BpmInfo {
                bpm: clock.bpm(),
                beats_per_bar: clock.beats_per_bar(),
                beat_phase: clock.beat_phase(),
                current_beat_in_bar: (clock.current_beat() % clock.beats_per_bar() as f32).floor() as u32,
            }
        };

        egui::TopBottomPanel::top("status_bar")
            .exact_height(24.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.settings.show_fps {
                            ui.label(
                                egui::RichText::new(format!("{:.1} fps | {:.2}ms", fps, frame_time_ms))
                                    .monospace()
                                    .color(egui::Color32::from_rgb(120, 200, 120)),
                            );
                            ui.separator();
                        }

                        // BPM display with beat indicators
                        if self.settings.show_bpm {
                            // Beat indicator dots
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 3.0;

                                for beat in 0..bpm_info.beats_per_bar {
                                    let is_current = beat == bpm_info.current_beat_in_bar;
                                    let is_downbeat = beat == 0;

                                    let pulse = if is_current {
                                        1.0 - bpm_info.beat_phase * 0.5
                                    } else {
                                        0.5
                                    };

                                    let color = if is_current {
                                        if is_downbeat {
                                            egui::Color32::from_rgb(
                                                (255.0 * pulse) as u8,
                                                (180.0 * pulse) as u8,
                                                50,
                                            )
                                        } else {
                                            egui::Color32::from_rgb(
                                                50,
                                                (200.0 * pulse) as u8,
                                                (255.0 * pulse) as u8,
                                            )
                                        }
                                    } else {
                                        egui::Color32::from_gray(60)
                                    };

                                    let size = if is_current { 8.0 } else { 6.0 };
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(size, size),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().circle_filled(rect.center(), size / 2.0, color);
                                }
                            });

                            ui.add_space(4.0);

                            // Time signature
                            ui.label(
                                egui::RichText::new(format!("{}/4", bpm_info.beats_per_bar))
                                    .small()
                                    .weak(),
                            );

                            ui.add_space(2.0);

                            // Editable BPM value
                            let mut bpm = bpm_info.bpm;
                            let response = ui.add(
                                egui::DragValue::new(&mut bpm)
                                    .speed(0.5)
                                    .range(20.0..=300.0)
                                    .suffix(" BPM")
                                    .custom_formatter(|n, _| format!("{:.1}", n))
                            );

                            if response.changed() {
                                self.effect_manager.bpm_clock_mut().set_bpm(bpm);
                            }

                            // Tap tempo button
                            if ui
                                .add(egui::Button::new("TAP").small())
                                .on_hover_text("Tap to set tempo")
                                .clicked()
                            {
                                self.effect_manager.bpm_clock_mut().tap();
                            }

                            // Resync button
                            if ui
                                .add(egui::Button::new("⟲").small())
                                .on_hover_text("Resync to bar start")
                                .clicked()
                            {
                                self.effect_manager.bpm_clock_mut().resync_to_bar();
                            }

                            ui.separator();
                        }

                        // Status message (fades after 3 seconds)
                        if let Some((msg, time)) = &self.menu_bar.status_message {
                            let elapsed = time.elapsed().as_secs_f32();
                            if elapsed < 3.0 {
                                let alpha = if elapsed > 2.0 {
                                    ((3.0 - elapsed) * 255.0) as u8
                                } else {
                                    255
                                };
                                ui.label(
                                    egui::RichText::new(msg)
                                        .color(egui::Color32::from_rgba_unmultiplied(180, 180, 255, alpha)),
                                );
                            } else {
                                self.menu_bar.status_message = None;
                            }
                        }
                    });
                });
            });
    }

    /// Get panel states for native menu synchronization
    /// Returns a Vec of (panel_id, title, is_open) tuples
    pub fn get_panel_states(&self) -> Vec<(&str, &str, bool)> {
        vec![
            (
                crate::ui::dock::panel_ids::CLIP_GRID,
                "Clip Grid",
                self.dock_manager
                    .get_panel(crate::ui::dock::panel_ids::CLIP_GRID)
                    .map(|p| p.open)
                    .unwrap_or(false),
            ),
            (
                crate::ui::dock::panel_ids::PROPERTIES,
                "Properties",
                self.dock_manager
                    .get_panel(crate::ui::dock::panel_ids::PROPERTIES)
                    .map(|p| p.open)
                    .unwrap_or(false),
            ),
            (
                crate::ui::dock::panel_ids::SOURCES,
                "Sources",
                self.dock_manager
                    .get_panel(crate::ui::dock::panel_ids::SOURCES)
                    .map(|p| p.open)
                    .unwrap_or(false),
            ),
            (
                crate::ui::dock::panel_ids::EFFECTS_BROWSER,
                "Effects",
                self.dock_manager
                    .get_panel(crate::ui::dock::panel_ids::EFFECTS_BROWSER)
                    .map(|p| p.open)
                    .unwrap_or(false),
            ),
            (
                crate::ui::dock::panel_ids::PREVIEW_MONITOR,
                "Preview Monitor",
                self.dock_manager
                    .get_panel(crate::ui::dock::panel_ids::PREVIEW_MONITOR)
                    .map(|p| p.open)
                    .unwrap_or(false),
            ),
            (
                crate::ui::dock::panel_ids::PERFORMANCE,
                "Performance",
                self.performance_panel.open,
            ),
        ]
    }

    /// Get current performance metrics
    pub fn performance_metrics(&self) -> crate::telemetry::PerformanceMetrics {
        use crate::telemetry::{GpuMemoryStats, PerformanceMetrics};

        // Count active clips and effects
        let mut active_clip_count = 0;
        let mut effect_count = 0;

        for layer in self.environment.layers() {
            if layer.visible && layer.active_clip.is_some() {
                active_clip_count += 1;
            }
            effect_count += layer.effects.len();
        }
        effect_count += self.environment.effects().len();

        // Estimate GPU memory usage
        let env_texture_bytes = (self.environment.width() as u64)
            * (self.environment.height() as u64)
            * 4; // RGBA

        let mut layer_texture_bytes: u64 = 0;
        let mut ndi_stats = Vec::new();
        for (_, runtime) in &self.layer_runtimes {
            if runtime.texture.is_some() && runtime.has_frame {
                // Estimate texture size: width * height * 4 bytes (RGBA)
                layer_texture_bytes += (runtime.video_width as u64)
                    * (runtime.video_height as u64)
                    * 4;
            }
            // Collect NDI stats from active NDI receivers
            if let Some(stats) = runtime.ndi_stats() {
                ndi_stats.push(stats);
            }
        }

        let gpu_memory = GpuMemoryStats {
            environment_texture: env_texture_bytes,
            layer_textures: layer_texture_bytes,
            effect_buffers: 0, // TODO: Track effect buffer usage
            total: env_texture_bytes + layer_texture_bytes,
        };

        PerformanceMetrics {
            frame_stats: self.frame_profiler.stats(),
            fps: self.frame_profiler.fps(),
            target_fps: self.settings.target_fps,
            gpu_timings: self.gpu_profiler.last_timings().clone(),
            gpu_total_ms: self.gpu_profiler.total_ms(),
            layer_count: self.environment.layer_count(),
            active_clip_count,
            effect_count,
            gpu_memory,
            video_frame_time_ms: self.video_frame_time_ms,
            ui_frame_time_ms: self.ui_frame_time_ms,
            ndi_stats,
        }
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

        tracing::info!("Added layer {} with video: {:?}", layer_id, path);
        Ok(layer_id)
    }

    /// Load a video for an existing layer
    fn load_layer_video(&mut self, layer_id: u32, path: &std::path::Path) -> Result<(), String> {
        let old_runtime_exists = self.layer_runtimes.contains_key(&layer_id);

        // Open video player (starts background decode thread)
        // Use BGRA output format when BGRA pipeline mode is enabled
        let use_bgra = self.settings.bgra_pipeline_enabled;
        let player = if use_bgra {
            VideoPlayer::open_bgra(path).map_err(|e| format!("Failed to open video: {}", e))?
        } else {
            VideoPlayer::open(path).map_err(|e| format!("Failed to open video: {}", e))?
        };

        tracing::info!(
            "Layer {}: Loaded video {}x{} @ {:.2}fps, duration: {:.2}s, gpu_native: {}, bgra: {}",
            layer_id,
            player.width(),
            player.height(),
            player.frame_rate(),
            player.duration(),
            player.is_gpu_native(),
            use_bgra
        );

        // Create video texture with appropriate format
        // HAP uses raw DXT extraction for GPU-native upload
        // DXV v4 (most common) requires FFmpeg decode to RGBA due to proprietary compression
        let use_gpu_native = player.is_hap() && self.bc_texture_supported;

        let video_texture = if use_gpu_native {
            tracing::info!("Using GPU-native BC texture for layer {} (HAP fast path)", layer_id);
            VideoTexture::new_gpu_native(&self.device, player.width(), player.height(), player.is_bc3())
        } else if use_bgra {
            tracing::info!("Using BGRA texture for layer {} (BGRA pipeline mode)", layer_id);
            VideoTexture::new_bgra(&self.device, player.width(), player.height())
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
            ndi_receiver: None,
            omt_receiver: None,
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
            old_clip_transform: None,
            old_params_buffer: None,
            params_buffer: Some(params_buffer),
            // Fade-out state (initialized empty)
            fade_out_active: false,
            fade_out_start: None,
            fade_out_duration: std::time::Duration::ZERO,
            // Format tracking (not used for video files, decoded to RGBA)
            ndi_is_bgra: false,
            omt_is_bgra: false,
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

    /// Load an NDI source for an existing layer
    fn load_layer_ndi(
        &mut self,
        layer_id: u32,
        ndi_name: &str,
    ) -> Result<(), String> {
        let old_runtime_exists = self.layer_runtimes.contains_key(&layer_id);

        // Create NDI receiver (url_address not currently used by NdiReceiver)
        let receiver = crate::network::NdiReceiver::connect(ndi_name)
            .map_err(|e| format!("Failed to connect to NDI source: {}", e))?;

        tracing::info!(
            "Layer {}: Connected to NDI source '{}'",
            layer_id,
            ndi_name
        );

        // Create a default-sized texture that will be resized on first frame
        // NDI sources don't report their resolution until we receive a frame
        let default_width = 1920;
        let default_height = 1080;

        // Use BGRA texture format in BGRA pipeline mode
        let video_texture = if self.settings.bgra_pipeline_enabled {
            VideoTexture::new_bgra(&self.device, default_width, default_height)
        } else {
            VideoTexture::new(&self.device, default_width, default_height)
        };

        // Create per-layer params buffer
        let params_buffer = self.video_renderer.create_params_buffer(&self.device);

        // Create bind group
        let bind_group = self
            .video_renderer
            .create_bind_group_with_buffer(&self.device, &video_texture, &params_buffer);

        // Store runtime
        let runtime = LayerRuntime {
            layer_id,
            video_width: default_width,
            video_height: default_height,
            player: None,
            ndi_receiver: Some(receiver),
            omt_receiver: None,
            texture: Some(video_texture),
            bind_group: Some(bind_group),
            has_frame: false,
            // Transition state (initialized empty)
            transition_active: false,
            transition_start: None,
            transition_duration: std::time::Duration::ZERO,
            transition_type: crate::compositor::ClipTransition::Cut,
            old_bind_group: None,
            old_video_width: 0,
            old_video_height: 0,
            old_clip_transform: None,
            old_params_buffer: None,
            params_buffer: Some(params_buffer),
            // Fade-out state (initialized empty)
            fade_out_active: false,
            fade_out_start: None,
            fade_out_duration: std::time::Duration::ZERO,
            // Format tracking - default to BGRA, will be updated from actual frame data
            ndi_is_bgra: true,
            omt_is_bgra: true,
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

    /// Load an OMT stream on a layer
    fn load_layer_omt(
        &mut self,
        layer_id: u32,
        address: &str,
    ) -> Result<(), String> {
        let old_runtime_exists = self.layer_runtimes.contains_key(&layer_id);

        // Create OMT receiver
        let receiver = crate::network::OmtReceiver::connect(address)
            .map_err(|e| format!("Failed to connect to OMT source: {}", e))?;

        tracing::info!(
            "Layer {}: Connected to OMT source '{}'",
            layer_id,
            address
        );

        // Create a default-sized texture that will be resized on first frame
        // OMT sources don't report their resolution until we receive a frame
        let default_width = 1920;
        let default_height = 1080;

        // Use BGRA texture format in BGRA pipeline mode
        let video_texture = if self.settings.bgra_pipeline_enabled {
            VideoTexture::new_bgra(&self.device, default_width, default_height)
        } else {
            VideoTexture::new(&self.device, default_width, default_height)
        };

        // Create per-layer params buffer
        let params_buffer = self.video_renderer.create_params_buffer(&self.device);

        // Create bind group
        let bind_group = self
            .video_renderer
            .create_bind_group_with_buffer(&self.device, &video_texture, &params_buffer);

        // Store runtime
        let runtime = LayerRuntime {
            layer_id,
            video_width: default_width,
            video_height: default_height,
            player: None,
            ndi_receiver: None,
            omt_receiver: Some(receiver),
            texture: Some(video_texture),
            bind_group: Some(bind_group),
            has_frame: false,
            // Transition state (initialized empty)
            transition_active: false,
            transition_start: None,
            transition_duration: std::time::Duration::ZERO,
            transition_type: crate::compositor::ClipTransition::Cut,
            old_bind_group: None,
            old_video_width: 0,
            old_video_height: 0,
            old_clip_transform: None,
            old_params_buffer: None,
            params_buffer: Some(params_buffer),
            // Fade-out state (initialized empty)
            fade_out_active: false,
            fade_out_start: None,
            fade_out_duration: std::time::Duration::ZERO,
            // Format tracking - default to BGRA, will be updated from actual frame data
            ndi_is_bgra: true,
            omt_is_bgra: true,
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
            tracing::info!("Removed layer {}", layer_id);
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
        tracing::info!("Added layer {} with {} clip slots", next_id, clip_count);
        self.menu_bar.set_status(format!("Added Layer {}", next_id));
    }

    /// Delete a layer by ID
    pub fn delete_layer(&mut self, layer_id: u32) {
        // Don't allow deleting the last layer
        if self.environment.layer_count() <= 1 {
            tracing::warn!("Cannot delete the last layer");
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

        tracing::info!("Added column - now {} clip slots", new_count);
        self.menu_bar.set_status(format!("Added column {}", new_count));
    }

    /// Delete a column (clip slot) from all layers
    pub fn delete_column(&mut self, column_index: usize) {
        // Don't allow deleting the last column
        if self.settings.global_clip_count <= 1 {
            tracing::warn!("Cannot delete the last column");
            self.menu_bar.set_status("Cannot delete the last column".to_string());
            return;
        }

        // Check if the column index is valid
        if column_index >= self.settings.global_clip_count {
            tracing::warn!("Invalid column index: {}", column_index);
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
        tracing::info!("Deleted column {} - now {} clip slots", column_index + 1, self.settings.global_clip_count);
        self.menu_bar.set_status(format!("Deleted column {}", column_index + 1));
    }

    /// Update all layer videos - pick up decoded frames (non-blocking)
    ///
    /// Rate-limits texture uploads to MAX_UPLOADS_PER_FRAME to prevent GPU bandwidth spikes.
    /// Uses round-robin ordering to ensure fair distribution across layers.
    pub fn update_videos(&mut self) {
        let video_start = std::time::Instant::now();

        // Limit texture uploads per frame to prevent bandwidth spikes.
        // With 16 layers triggering simultaneously, uploads complete over multiple frames
        // instead of causing a single-frame spike.
        const MAX_UPLOADS_PER_FRAME: usize = 4;
        let mut uploads_this_frame = 0;

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

                // Rate-limited texture upload
                if uploads_this_frame < MAX_UPLOADS_PER_FRAME {
                    match runtime.try_update_texture(&self.queue) {
                        TextureUpdateResult::Uploaded => {
                            self.last_upload_layer = layer_id;
                            uploads_this_frame += 1;
                        }
                        TextureUpdateResult::NeedsResize { width, height } => {
                            runtime.resize_texture(&self.device, &self.video_renderer, width, height);
                            // Next frame will upload after resize
                        }
                        TextureUpdateResult::NoFrame => {}
                    }
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
            tracing::info!("⏹️ Fade-out complete, stopped clip on layer {}", layer_id);
        }
        
        // Update pending runtimes - these get priority since user is waiting for first frame
        // Still count toward upload limit to prevent bandwidth spikes
        for runtime in self.pending_runtimes.values_mut() {
            if uploads_this_frame < MAX_UPLOADS_PER_FRAME {
                match runtime.try_update_texture(&self.queue) {
                    TextureUpdateResult::Uploaded => {
                        uploads_this_frame += 1;
                    }
                    TextureUpdateResult::NeedsResize { width, height } => {
                        runtime.resize_texture(&self.device, &self.video_renderer, width, height);
                        // Next frame will upload after resize
                    }
                    TextureUpdateResult::NoFrame => {}
                }
            }
        }

        // Preview player always gets one upload (not counted toward limit)
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
                    // Also store the old clip transform for crossfade
                    if let Some(layer) = self.environment.get_layer(layer_id) {
                        new_runtime.old_clip_transform = layer.active_clip
                            .and_then(|slot| layer.get_clip(slot))
                            .map(|clip| clip.transform.clone());
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

        self.video_frame_time_ms = video_start.elapsed().as_secs_f64() * 1000.0;
    }

    /// Poll for shader changes and hot-reload if needed
    pub fn poll_shader_reload(&mut self) {
        if let Some(ref mut watcher) = self.shader_watcher {
            if watcher.poll().is_some() {
                // A shader file changed, reload it
                match crate::shaders::load_fullscreen_quad_shader() {
                    Ok(source) => {
                        if let Err(e) = self.video_renderer.rebuild_pipelines(&self.device, &source) {
                            tracing::error!("❌ Shader reload failed: {}", e);
                        }
                    }
                    Err(e) => {
                        tracing::error!("❌ Failed to read shader file: {}", e);
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
                // Get the loop mode for this clip
                let loop_mode = self.environment.get_layer(layer_id)
                    .and_then(|l| l.get_clip(slot))
                    .map(|c| c.loop_mode)
                    .unwrap_or_default();

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
                    tracing::info!("🔄 Restarting clip {} on layer {}", slot, layer_id);
                    self.restart_layer_video(layer_id);
                    // Update loop mode on existing player
                    if let Some(runtime) = self.layer_runtimes.get(&layer_id) {
                        if let Some(player) = &runtime.player {
                            player.set_loop_mode(loop_mode.as_u8());
                        }
                    }
                } else {
                    // Different clip - need to load it
                    tracing::info!("🎬 Loading clip {} on layer {} with {:?} transition: {:?}",
                        slot, layer_id, transition.name(), path.display());

                    // Store the transition type for when the new clip is ready
                    self.pending_transition.insert(layer_id, transition);

                    self.load_layer_video(layer_id, path)?;

                    // Set loop mode on the new player (in pending or current runtime)
                    if let Some(runtime) = self.pending_runtimes.get(&layer_id).or(self.layer_runtimes.get(&layer_id)) {
                        if let Some(player) = &runtime.player {
                            player.set_loop_mode(loop_mode.as_u8());
                        }
                    }
                }

                // Update the active clip slot in the layer
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    layer.active_clip = Some(slot);
                    layer.source = crate::compositor::LayerSource::Video(path.clone());
                }
            }
            crate::compositor::ClipSource::Omt { address, name } => {
                // Check if this is a replay of the same OMT source
                let is_same_source = if let Some(runtime) = self.layer_runtimes.get(&layer_id) {
                    if let Some(receiver) = &runtime.omt_receiver {
                        receiver.source_address() == address
                    } else {
                        false
                    }
                } else {
                    false
                };

                if is_same_source {
                    // Same OMT source - nothing to do (OMT streams continuously)
                    tracing::info!("📡 OMT source already active on layer {}: {} ({})", layer_id, name, address);
                } else {
                    // Different source - connect to new OMT source
                    tracing::info!(
                        "📡 Connecting to OMT source '{}' ({}) on layer {} with {:?} transition",
                        name, address, layer_id, transition.name()
                    );

                    // Store the transition type for when the new source is ready
                    self.pending_transition.insert(layer_id, transition);

                    self.load_layer_omt(layer_id, address)?;
                }

                // Update the active clip slot in the layer
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    layer.active_clip = Some(slot);
                    // Use None for OMT sources (could add LayerSource::Omt variant later)
                    layer.source = crate::compositor::LayerSource::None;
                }
            }
            crate::compositor::ClipSource::Ndi { ndi_name, .. } => {
                // Check if this is a replay of the same NDI source
                let is_same_source = if let Some(runtime) = self.layer_runtimes.get(&layer_id) {
                    if let Some(receiver) = &runtime.ndi_receiver {
                        receiver.source_name() == ndi_name
                    } else {
                        false
                    }
                } else {
                    false
                };

                if is_same_source {
                    // Same NDI source - nothing to do (NDI streams continuously)
                    tracing::info!("📺 NDI source already active on layer {}: {}", layer_id, ndi_name);
                } else {
                    // Different source - connect to new NDI source
                    tracing::info!(
                        "📺 Connecting to NDI source '{}' on layer {} with {:?} transition",
                        ndi_name, layer_id, transition.name()
                    );

                    // Store the transition type for when the new source is ready
                    self.pending_transition.insert(layer_id, transition);

                    self.load_layer_ndi(layer_id, ndi_name)?;
                }

                // Update the active clip slot in the layer
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    layer.active_clip = Some(slot);
                    // Use None for NDI sources (could add LayerSource::Ndi variant later)
                    layer.source = crate::compositor::LayerSource::None;
                }
            }
        }

        // Floor sync: trigger corresponding clip on floor layer
        // Only sync if floor sync is enabled and this is not the floor layer itself
        if self.settings.floor_sync_enabled {
            let floor_layer_idx = self.settings.floor_layer_index;
            let layers = self.environment.layers();
            if floor_layer_idx < layers.len() {
                let floor_layer_id = layers[floor_layer_idx].id;
                // Only sync if this isn't already the floor layer (prevents recursion)
                if floor_layer_id != layer_id {
                    // Check if floor layer has a clip at this slot before triggering
                    if layers[floor_layer_idx].get_clip(slot).is_some() {
                        tracing::debug!(
                            "🔄 Floor sync: triggering clip {} on floor layer {} (index {})",
                            slot, floor_layer_id, floor_layer_idx
                        );
                        // Ignore errors - it's okay if the floor layer doesn't have a clip
                        let _ = self.trigger_clip(floor_layer_id, slot);
                    }
                }
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

        tracing::info!("⏹️ Stopped clip on layer {}", layer_id);
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
                tracing::info!("⏹️ Starting fade-out on layer {} ({}ms)", layer_id, fade_duration);
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
                tracing::info!(
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
        // Get OMT sources only (not NDI) from discovery service
        let sources: Vec<(String, String, String)> = if let Some(discovery) = &self.omt_discovery {
            discovery.get_sources_by_type(crate::network::SourceType::Omt)
                .into_iter()
                .map(|s| {
                    let address = s.address(); // Call before moving name
                    (s.id, s.name, address)
                })
                .collect()
        } else {
            Vec::new()
        };

        self.sources_panel.set_omt_sources(sources);
    }

    /// Update the UI with the current OMT sources (called periodically)
    pub fn update_omt_sources_in_ui(&mut self) {
        if let Some(discovery) = &self.omt_discovery {
            let sources: Vec<(String, String, String)> = discovery
                .get_sources_by_type(crate::network::SourceType::Omt)
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
            tracing::info!("OMT: Broadcast already active or starting");
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
                        tracing::info!("📡 OMT: Started sender as '{}' on port {}", name, port);
                        let _ = tx.send(Ok(sender));
                    }
                    Err(e) => {
                        tracing::error!("OMT: Failed to start broadcast: {}", e);
                        let _ = tx.send(Err(format!("{}", e)));
                    }
                }
            });

            self.pending_omt_sender = Some(rx);
            self.menu_bar.set_status("Starting OMT broadcast...");
        } else {
            tracing::warn!("OMT: Cannot start broadcast - no Tokio runtime");
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
                    tracing::info!("📡 OMT: Creating capture pipeline for {}x{}", w, h);

                    let mut capture = crate::network::OmtCapture::new(&self.device, w, h);
                    capture.set_target_fps(self.settings.target_fps);
                    capture.start_sender_thread(sender, rt.handle().clone());
                    self.omt_capture = Some(capture);
                    self.omt_broadcast_enabled = true;
                    self.menu_bar.set_status(format!("OMT broadcast started ({}x{})", w, h));
                    tracing::info!("📡 OMT: Capture pipeline started, broadcasting {}x{} on port 5970", w, h);
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
            tracing::info!("📡 OMT: Stopped broadcast");
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

    // =========================================================================
    // REST API Server Methods
    // =========================================================================

    /// Start the REST API server on the configured port
    pub fn start_api_server(&mut self) {
        if self.api_server_running {
            tracing::info!("🌐 API: Server already running");
            return;
        }

        let port = self.settings.api_port;

        if let Some(rt) = &self.tokio_runtime {
            let runtime_handle = rt.handle().clone();

            // Create shared state and command channel
            let (shared_state, command_rx) = crate::api::create_shared_state();
            self.api_shared_state = Some(shared_state.clone());
            self.api_command_rx = Some(command_rx);

            // Create shutdown channel for graceful termination
            let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
            self.api_shutdown_tx = Some(shutdown_tx);

            // Spawn the API server on a background thread (includes dashboard at /)
            std::thread::spawn(move || {
                runtime_handle.block_on(async {
                    if let Err(e) = crate::api::run_server(port, shared_state, shutdown_rx).await {
                        log::error!("🌐 API: Server error: {}", e);
                    }
                });
            });

            self.api_server_running = true;
            self.menu_bar.set_status(format!("API + Dashboard on port {}", port));
            log::info!("🌐 API + Dashboard: http://0.0.0.0:{}", port);
        } else {
            tracing::warn!("🌐 API: Cannot start server - no Tokio runtime");
        }
    }

    /// Check if the API server is running
    pub fn is_api_server_running(&self) -> bool {
        self.api_server_running
    }

    /// Update the API shared state with current app state
    /// Call this once per frame to keep the API snapshot up-to-date
    pub fn update_api_snapshot(&self) {
        if let Some(ref shared_state) = self.api_shared_state {
            let snapshot = self.create_api_snapshot();
            shared_state.update_snapshot(snapshot);
        }
    }

    /// Create an API snapshot from current app state
    fn create_api_snapshot(&self) -> crate::api::AppSnapshot {
        use crate::api::{
            AppSnapshot, ClipSnapshot, EffectSnapshot, EffectTypeInfo, LayerSnapshot,
            OutputSnapshot, PerformanceSnapshot, StreamingSnapshot, ViewportSnapshot,
        };
        use crate::compositor::ClipSource;

        // Helper to convert effect instances to snapshots
        let effect_to_snapshot = |effect: &crate::effects::EffectInstance| -> EffectSnapshot {
            EffectSnapshot {
                id: effect.id.to_string(),
                effect_type: effect.effect_type.clone(),
                enabled: !effect.bypassed,
                bypassed: effect.bypassed,
                solo: effect.soloed,
            }
        };

        let layers: Vec<LayerSnapshot> = self.environment.layers().iter().map(|layer| {
            let clips: Vec<ClipSnapshot> = layer.clips.iter().enumerate().map(|(slot, clip_opt)| {
                if let Some(clip) = clip_opt {
                    let (source_type, source_path) = match &clip.source {
                        ClipSource::File { path } => (Some("file".to_string()), Some(path.display().to_string())),
                        ClipSource::Omt { name, .. } => (Some("omt".to_string()), Some(name.clone())),
                        ClipSource::Ndi { ndi_name, .. } => (Some("ndi".to_string()), Some(ndi_name.clone())),
                    };
                    // Include clip effects
                    let clip_effects: Vec<EffectSnapshot> = clip.effects.effects.iter()
                        .map(&effect_to_snapshot)
                        .collect();
                    ClipSnapshot {
                        slot,
                        source_type,
                        source_path,
                        label: clip.label.clone(),
                        effects: clip_effects,
                    }
                } else {
                    ClipSnapshot {
                        slot,
                        source_type: None,
                        source_path: None,
                        label: None,
                        effects: Vec::new(),
                    }
                }
            }).collect();

            // Include layer effects
            let layer_effects: Vec<EffectSnapshot> = layer.effects.effects.iter()
                .map(&effect_to_snapshot)
                .collect();

            LayerSnapshot {
                id: layer.id,
                name: layer.name.clone(),
                visible: layer.visible,
                opacity: layer.opacity,
                blend_mode: layer.blend_mode,
                position: layer.transform.position,
                scale: layer.transform.scale,
                rotation: layer.transform.rotation,
                anchor: layer.transform.anchor,
                transition: layer.transition.clone(),
                clips,
                active_clip: layer.active_clip,
                effects: layer_effects,
            }
        }).collect();

        // Build output displays from available_displays
        let outputs: Vec<OutputSnapshot> = self.available_displays.iter().map(|display| {
            OutputSnapshot {
                id: display.id,
                name: display.name.clone(),
                width: display.size.0,
                height: display.size.1,
                is_primary: display.is_primary,
                refresh_rate_hz: display.refresh_rate_millihertz.map(|mhz| mhz / 1000),
            }
        }).collect();

        // Build performance metrics from frame profiler
        let frame_stats = self.frame_profiler.stats();
        let performance = PerformanceSnapshot {
            frame_time_avg_ms: frame_stats.avg_ms as f32,
            frame_time_min_ms: frame_stats.min_ms as f32,
            frame_time_max_ms: frame_stats.max_ms as f32,
            frame_time_p95_ms: frame_stats.p95_ms as f32,
            frame_time_p99_ms: frame_stats.p99_ms as f32,
            gpu_timings: self.gpu_profiler.last_timings().iter()
                .map(|(k, v)| (k.clone(), *v as f32))
                .collect(),
            gpu_total_ms: self.gpu_profiler.total_ms() as f32,
            gpu_memory_mb: 0.0, // GPU memory tracking not yet implemented
        };

        // Build effect types from registry with parameter definitions
        let effect_types: Vec<EffectTypeInfo> = self.effect_manager.registry().effects()
            .map(|def| {
                let parameters: Vec<crate::api::EffectParamInfo> = def.default_parameters()
                    .iter()
                    .map(|p| {
                        let (param_type, default_val) = match &p.meta.default {
                            crate::effects::ParameterValue::Float(v) => ("float", serde_json::json!(v)),
                            crate::effects::ParameterValue::Int(v) => ("int", serde_json::json!(v)),
                            crate::effects::ParameterValue::Bool(v) => ("bool", serde_json::json!(v)),
                            crate::effects::ParameterValue::Color(v) => ("color", serde_json::json!(v)),
                            crate::effects::ParameterValue::Vec2(v) => ("vec2", serde_json::json!(v)),
                            crate::effects::ParameterValue::Vec3(v) => ("vec3", serde_json::json!(v)),
                            crate::effects::ParameterValue::Enum { index, options } => ("enum", serde_json::json!({"index": index, "options": options})),
                            crate::effects::ParameterValue::String(v) => ("string", serde_json::json!(v)),
                        };
                        crate::api::EffectParamInfo {
                            name: p.meta.name.clone(),
                            param_type: param_type.to_string(),
                            default: default_val,
                            min: p.meta.min,
                            max: p.meta.max,
                        }
                    })
                    .collect();
                EffectTypeInfo {
                    effect_type: def.effect_type().to_string(),
                    display_name: def.display_name().to_string(),
                    category: def.category().to_string(),
                    parameters,
                }
            })
            .collect();

        // Get effect categories in display order
        let effect_categories: Vec<String> = self.effect_manager.registry()
            .categories()
            .iter()
            .cloned()
            .collect();

        AppSnapshot {
            env_width: self.environment.width(),
            env_height: self.environment.height(),
            target_fps: self.settings.target_fps,
            current_fps: self.ui_fps as f32,
            frame_time_ms: if self.ui_fps > 0.0 { 1000.0 / self.ui_fps as f32 } else { 0.0 },
            paused: false, // TODO: Track global pause state
            layers,
            viewport: ViewportSnapshot {
                zoom: self.viewport.zoom(),
                pan_x: self.viewport.offset().0,
                pan_y: self.viewport.offset().1,
            },
            streaming: StreamingSnapshot {
                omt_broadcasting: self.is_omt_broadcasting(),
                omt_name: None, // OMT name is passed at broadcast start, not stored
                omt_port: None, // OMT port is passed at broadcast start, not stored
                omt_capture_fps: self.settings.omt_capture_fps,
                ndi_broadcasting: self.is_ndi_broadcasting(),
                ndi_name: None, // NDI name is passed at broadcast start, not stored
                texture_sharing: self.texture_share_enabled,
            },
            sources: Vec::new(), // TODO: Populate discovered sources
            file: crate::api::FileSnapshot {
                current_path: self.current_file.as_ref().map(|p| p.display().to_string()),
                modified: false, // TODO: Track modified state
                recent_files: Vec::new(), // TODO: Track recent files
            },
            environment_effects: self.environment.effects().effects.iter()
                .map(&effect_to_snapshot)
                .collect(),
            clip_columns: self.settings.global_clip_count,
            outputs,
            performance,
            effect_types,
            effect_categories,
        }
    }

    /// Process pending API commands
    /// Call this once per frame to handle commands from the API server
    pub fn process_api_commands(&mut self) {
        use crate::api::ApiCommand;

        // Take the receiver temporarily to avoid borrow issues
        let mut rx = match self.api_command_rx.take() {
            Some(rx) => rx,
            None => return,
        };

        // Process all pending commands
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                // Environment commands
                ApiCommand::SetEnvironmentSize { width, height } => {
                    self.environment.resize(&self.device, width, height);
                    self.settings.environment_width = width;
                    self.settings.environment_height = height;
                    tracing::info!("🌐 API: Resized environment to {}x{}", width, height);
                }
                ApiCommand::SetTargetFps { fps } => {
                    self.settings.target_fps = fps;
                    tracing::info!("🌐 API: Set target FPS to {}", fps);
                }

                // Layer commands
                ApiCommand::CreateLayer { name } => {
                    let id = self.environment.add_layer(&name);
                    tracing::info!("🌐 API: Created layer '{}' (id={})", name, id);
                }
                ApiCommand::DeleteLayer { id } => {
                    if self.environment.remove_layer(id).is_some() {
                        self.layer_runtimes.remove(&id);
                        tracing::info!("🌐 API: Deleted layer {}", id);
                    }
                }
                ApiCommand::UpdateLayer { id, name, visible, opacity, blend_mode } => {
                    if let Some(layer) = self.environment.get_layer_mut(id) {
                        if let Some(n) = name { layer.name = n; }
                        if let Some(v) = visible { layer.visible = v; }
                        if let Some(o) = opacity { layer.set_opacity(o); }
                        if let Some(b) = blend_mode { layer.blend_mode = b; }
                        tracing::debug!("🌐 API: Updated layer {}", id);
                    }
                }
                ApiCommand::ReorderLayer { id, position } => {
                    if let Some(current_idx) = self.environment.layers().iter().position(|l| l.id == id) {
                        self.environment.move_layer(current_idx, position);
                        tracing::info!("🌐 API: Moved layer {} to position {}", id, position);
                    }
                }

                // Layer transform commands
                ApiCommand::SetLayerPosition { id, x, y } => {
                    if let Some(layer) = self.environment.get_layer_mut(id) {
                        layer.set_position(x, y);
                    }
                }
                ApiCommand::SetLayerScale { id, scale_x, scale_y } => {
                    if let Some(layer) = self.environment.get_layer_mut(id) {
                        layer.set_scale(scale_x, scale_y);
                    }
                }
                ApiCommand::SetLayerRotation { id, rotation } => {
                    if let Some(layer) = self.environment.get_layer_mut(id) {
                        layer.set_rotation(rotation);
                    }
                }
                ApiCommand::SetLayerTransform { id, position, scale, rotation, anchor } => {
                    if let Some(layer) = self.environment.get_layer_mut(id) {
                        if let Some((x, y)) = position { layer.transform.position = (x, y); }
                        if let Some((sx, sy)) = scale { layer.transform.scale = (sx, sy); }
                        if let Some(r) = rotation { layer.transform.rotation = r; }
                        if let Some((ax, ay)) = anchor { layer.transform.anchor = (ax, ay); }
                    }
                }

                // Layer property commands
                ApiCommand::SetLayerOpacity { id, opacity } => {
                    if let Some(layer) = self.environment.get_layer_mut(id) {
                        layer.set_opacity(opacity);
                    }
                }
                ApiCommand::SetLayerBlendMode { id, blend_mode } => {
                    if let Some(layer) = self.environment.get_layer_mut(id) {
                        layer.blend_mode = blend_mode;
                    }
                }
                ApiCommand::SetLayerVisibility { id, visible } => {
                    if let Some(layer) = self.environment.get_layer_mut(id) {
                        layer.visible = visible;
                    }
                }
                ApiCommand::SetLayerTransition { id, transition } => {
                    if let Some(layer) = self.environment.get_layer_mut(id) {
                        layer.transition = transition;
                    }
                }

                // Clip commands
                ApiCommand::SetClip { layer_id, slot, source_type, path, source_id, label } => {
                    if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                        let cell = match source_type.as_str() {
                            "file" => {
                                if let Some(p) = path {
                                    let mut cell = crate::compositor::ClipCell::new(&p);
                                    cell.label = label;
                                    Some(cell)
                                } else { None }
                            }
                            "omt" => {
                                if let Some(id) = source_id {
                                    let mut cell = crate::compositor::ClipCell::from_omt(&id, &id);
                                    cell.label = label;
                                    Some(cell)
                                } else { None }
                            }
                            "ndi" => {
                                if let Some(id) = source_id {
                                    let mut cell = crate::compositor::ClipCell::from_ndi(&id, None);
                                    cell.label = label;
                                    Some(cell)
                                } else { None }
                            }
                            _ => None,
                        };
                        if let Some(cell) = cell {
                            layer.set_clip(slot, cell);
                            tracing::info!("🌐 API: Set clip at layer {} slot {}", layer_id, slot);
                        }
                    }
                }
                ApiCommand::ClearClip { layer_id, slot } => {
                    if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                        layer.clear_clip(slot);
                        tracing::info!("🌐 API: Cleared clip at layer {} slot {}", layer_id, slot);
                    }
                }
                ApiCommand::TriggerClip { layer_id, slot } => {
                    let _ = self.trigger_clip(layer_id, slot);
                    tracing::info!("🌐 API: Triggered clip at layer {} slot {}", layer_id, slot);
                }
                ApiCommand::StopClip { layer_id } => {
                    self.stop_clip(layer_id);
                    tracing::info!("🌐 API: Stopped layer {}", layer_id);
                }

                // Playback commands
                ApiCommand::PauseAll => {
                    for runtime in self.layer_runtimes.values() {
                        if let Some(player) = &runtime.player {
                            player.pause();
                        }
                    }
                    tracing::info!("🌐 API: Paused all layers");
                }
                ApiCommand::ResumeAll => {
                    for runtime in self.layer_runtimes.values() {
                        if let Some(player) = &runtime.player {
                            player.resume();
                        }
                    }
                    tracing::info!("🌐 API: Resumed all layers");
                }
                ApiCommand::TogglePause => {
                    for runtime in self.layer_runtimes.values() {
                        runtime.toggle_pause();
                    }
                    tracing::info!("🌐 API: Toggled pause");
                }
                ApiCommand::RestartAll => {
                    for runtime in self.layer_runtimes.values() {
                        runtime.restart();
                    }
                    tracing::info!("🌐 API: Restarted all layers");
                }
                ApiCommand::PauseLayer { id } => {
                    if let Some(runtime) = self.layer_runtimes.get(&id) {
                        if let Some(player) = &runtime.player {
                            player.pause();
                        }
                    }
                }
                ApiCommand::ResumeLayer { id } => {
                    if let Some(runtime) = self.layer_runtimes.get(&id) {
                        if let Some(player) = &runtime.player {
                            player.resume();
                        }
                    }
                }
                ApiCommand::RestartLayer { id } => {
                    if let Some(runtime) = self.layer_runtimes.get(&id) {
                        runtime.restart();
                    }
                }

                // Viewport commands
                ApiCommand::ResetViewport => {
                    self.viewport.reset();
                    tracing::info!("🌐 API: Reset viewport");
                }
                ApiCommand::SetViewportZoom { zoom } => {
                    self.viewport.set_zoom(zoom);
                }
                ApiCommand::SetViewportPan { x, y } => {
                    self.viewport.set_offset(x, y);
                }

                // Streaming commands
                ApiCommand::StartOmtBroadcast { name, port } => {
                    self.start_omt_broadcast(&name, port);
                    tracing::info!("🌐 API: Started OMT broadcast '{}' on port {}", name, port);
                }
                ApiCommand::StopOmtBroadcast => {
                    self.stop_omt_broadcast();
                    tracing::info!("🌐 API: Stopped OMT broadcast");
                }
                ApiCommand::SetOmtCaptureFps { fps } => {
                    self.settings.omt_capture_fps = fps;
                    tracing::info!("🌐 API: Set OMT capture FPS to {}", fps);
                }
                ApiCommand::StartNdiBroadcast { name } => {
                    self.start_ndi_broadcast(&name);
                    tracing::info!("🌐 API: Started NDI broadcast '{}'", name);
                }
                ApiCommand::StopNdiBroadcast => {
                    self.stop_ndi_broadcast();
                    tracing::info!("🌐 API: Stopped NDI broadcast");
                }
                ApiCommand::StartTextureShare => {
                    self.start_texture_sharing();
                    tracing::info!("🌐 API: Started texture sharing");
                }
                ApiCommand::StopTextureShare => {
                    self.stop_texture_sharing();
                    tracing::info!("🌐 API: Stopped texture sharing");
                }

                // Source discovery commands
                ApiCommand::RefreshOmtSources => {
                    if let Some(discovery) = &mut self.omt_discovery {
                        discovery.refresh();
                    }
                    tracing::debug!("🌐 API: Refreshed OMT sources");
                }
                ApiCommand::StartNdiDiscovery => {
                    self.start_ndi_discovery();
                    tracing::info!("🌐 API: Started NDI discovery");
                }
                ApiCommand::StopNdiDiscovery => {
                    self.stop_ndi_discovery();
                    tracing::info!("🌐 API: Stopped NDI discovery");
                }
                ApiCommand::RefreshNdiSources => {
                    self.refresh_ndi_sources();
                    tracing::debug!("🌐 API: Refreshed NDI sources");
                }

                ApiCommand::PasteClip { layer_id, slot } => {
                    self.paste_clip(layer_id, slot);
                    tracing::debug!("🌐 API: Pasted clip to layer {} slot {}", layer_id, slot);
                }

                // Grid management commands
                ApiCommand::AddColumn => {
                    self.add_column();
                    tracing::info!("🌐 API: Added clip column");
                }
                ApiCommand::DeleteColumn { index } => {
                    self.delete_column(index);
                    tracing::info!("🌐 API: Deleted clip column {}", index);
                }

                // File operations
                ApiCommand::OpenFile { path } => {
                    let path = std::path::PathBuf::from(path);
                    match EnvironmentSettings::load_from_file(&path) {
                        Ok(settings) => {
                            self.settings = settings;
                            self.current_file = Some(path.clone());
                            tracing::info!("🌐 API: Opened file {:?}", path);
                        }
                        Err(e) => {
                            tracing::error!("🌐 API: Failed to open file: {}", e);
                        }
                    }
                }
                ApiCommand::SaveFile => {
                    if let Some(path) = self.current_file.clone() {
                        if let Err(e) = self.settings.save_to_file(&path) {
                            tracing::error!("🌐 API: Failed to save file: {}", e);
                        } else {
                            tracing::info!("🌐 API: Saved file {:?}", path);
                        }
                    } else {
                        tracing::warn!("🌐 API: No current file to save");
                    }
                }
                ApiCommand::SaveFileAs { path } => {
                    let path = std::path::PathBuf::from(path);
                    if let Err(e) = self.settings.save_to_file(&path) {
                        tracing::error!("🌐 API: Failed to save file as: {}", e);
                    } else {
                        self.current_file = Some(path.clone());
                        tracing::info!("🌐 API: Saved file as {:?}", path);
                    }
                }

                // Environment effects commands
                ApiCommand::AddEnvironmentEffect { effect_type } => {
                    if let Some(params) = self.effect_manager.registry().default_parameters(&effect_type) {
                        let display_name = self.effect_manager.registry()
                            .get(&effect_type)
                            .map(|d| d.display_name().to_string())
                            .unwrap_or_else(|| effect_type.clone());
                        let _id = self.environment.effects_mut().add(&effect_type, &display_name, params);
                        tracing::info!("🌐 API: Added environment effect '{}'", effect_type);
                    }
                }
                ApiCommand::RemoveEnvironmentEffect { effect_id } => {
                    if let Ok(id) = effect_id.parse::<u32>() {
                        self.environment.effects_mut().remove(id);
                        tracing::info!("🌐 API: Removed environment effect {}", effect_id);
                    }
                }
                ApiCommand::UpdateEnvironmentEffect { effect_id, parameters } => {
                    if let Ok(id) = effect_id.parse::<u32>() {
                        if let Some(effect) = self.environment.effects_mut().get_mut(id) {
                            // Update parameters from JSON
                            if let Ok(params_map) = serde_json::from_value::<std::collections::HashMap<String, serde_json::Value>>(parameters) {
                                for (name, json_value) in params_map {
                                    if let Some(param) = effect.parameters.iter_mut().find(|p| p.meta.name == name) {
                                        // Convert JSON value to ParameterValue
                                        if let Some(v) = json_value.as_f64() {
                                            param.value = crate::effects::ParameterValue::Float(v as f32);
                                        } else if let Some(v) = json_value.as_i64() {
                                            param.value = crate::effects::ParameterValue::Int(v as i32);
                                        } else if let Some(v) = json_value.as_bool() {
                                            param.value = crate::effects::ParameterValue::Bool(v);
                                        }
                                    }
                                }
                            }
                            tracing::debug!("🌐 API: Updated environment effect {}", effect_id);
                        }
                    }
                }
                ApiCommand::BypassEnvironmentEffect { effect_id } => {
                    if let Ok(id) = effect_id.parse::<u32>() {
                        if let Some(effect) = self.environment.effects_mut().get_mut(id) {
                            effect.bypassed = !effect.bypassed;
                            tracing::debug!("🌐 API: Toggled bypass for environment effect {}", effect_id);
                        }
                    }
                }
                ApiCommand::SoloEnvironmentEffect { effect_id } => {
                    tracing::warn!("🌐 API: Solo environment effect {} not yet implemented", effect_id);
                }
                ApiCommand::ReorderEnvironmentEffects { order: _ } => {
                    tracing::warn!("🌐 API: Reorder environment effects not yet implemented");
                }

                // Layer effects commands
                ApiCommand::AddLayerEffect { layer_id, effect_type } => {
                    if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                        if let Some(params) = self.effect_manager.registry().default_parameters(&effect_type) {
                            let display_name = self.effect_manager.registry()
                                .get(&effect_type)
                                .map(|d| d.display_name().to_string())
                                .unwrap_or_else(|| effect_type.clone());
                            let _id = layer.effects.add(&effect_type, &display_name, params);
                            tracing::info!("🌐 API: Added layer {} effect '{}'", layer_id, effect_type);
                        }
                    }
                }
                ApiCommand::RemoveLayerEffect { layer_id, effect_id } => {
                    if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                        if let Ok(id) = effect_id.parse::<u32>() {
                            layer.effects.remove(id);
                            tracing::info!("🌐 API: Removed layer {} effect {}", layer_id, effect_id);
                        }
                    }
                }
                ApiCommand::UpdateLayerEffect { layer_id, effect_id, parameters } => {
                    if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                        if let Ok(id) = effect_id.parse::<u32>() {
                            if let Some(effect) = layer.effects.get_mut(id) {
                                if let Ok(params_map) = serde_json::from_value::<std::collections::HashMap<String, serde_json::Value>>(parameters) {
                                    for (name, json_value) in params_map {
                                        if let Some(param) = effect.parameters.iter_mut().find(|p| p.meta.name == name) {
                                            if let Some(v) = json_value.as_f64() {
                                                param.value = crate::effects::ParameterValue::Float(v as f32);
                                            } else if let Some(v) = json_value.as_i64() {
                                                param.value = crate::effects::ParameterValue::Int(v as i32);
                                            } else if let Some(v) = json_value.as_bool() {
                                                param.value = crate::effects::ParameterValue::Bool(v);
                                            }
                                        }
                                    }
                                }
                                tracing::debug!("🌐 API: Updated layer {} effect {}", layer_id, effect_id);
                            }
                        }
                    }
                }
                ApiCommand::BypassLayerEffect { layer_id, effect_id } => {
                    if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                        if let Ok(id) = effect_id.parse::<u32>() {
                            if let Some(effect) = layer.effects.get_mut(id) {
                                effect.bypassed = !effect.bypassed;
                                tracing::debug!("🌐 API: Toggled bypass for layer {} effect {}", layer_id, effect_id);
                            }
                        }
                    }
                }
                ApiCommand::SoloLayerEffect { layer_id, effect_id } => {
                    tracing::warn!("🌐 API: Solo layer {} effect {} not yet implemented", layer_id, effect_id);
                }
                ApiCommand::ReorderLayerEffects { layer_id, order: _ } => {
                    tracing::warn!("🌐 API: Reorder layer {} effects not yet implemented", layer_id);
                }

                // Clip effects commands
                ApiCommand::AddClipEffect { layer_id, slot, effect_type } => {
                    if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                        if let Some(Some(clip)) = layer.clips.get_mut(slot) {
                            if let Some(params) = self.effect_manager.registry().default_parameters(&effect_type) {
                                let display_name = self.effect_manager.registry()
                                    .get(&effect_type)
                                    .map(|d| d.display_name().to_string())
                                    .unwrap_or_else(|| effect_type.clone());
                                let _id = clip.effects.add(&effect_type, &display_name, params);
                                tracing::info!("🌐 API: Added clip effect '{}' to layer {} slot {}", effect_type, layer_id, slot);
                            }
                        }
                    }
                }
                ApiCommand::RemoveClipEffect { layer_id, slot, effect_id } => {
                    if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                        if let Some(Some(clip)) = layer.clips.get_mut(slot) {
                            if let Ok(id) = effect_id.parse::<u32>() {
                                clip.effects.remove(id);
                                tracing::info!("🌐 API: Removed clip effect {} from layer {} slot {}", effect_id, layer_id, slot);
                            }
                        }
                    }
                }
                ApiCommand::UpdateClipEffect { layer_id, slot, effect_id, parameters } => {
                    if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                        if let Some(Some(clip)) = layer.clips.get_mut(slot) {
                            if let Ok(id) = effect_id.parse::<u32>() {
                                if let Some(effect) = clip.effects.get_mut(id) {
                                    if let Ok(params_map) = serde_json::from_value::<std::collections::HashMap<String, serde_json::Value>>(parameters) {
                                        for (name, json_value) in params_map {
                                            if let Some(param) = effect.parameters.iter_mut().find(|p| p.meta.name == name) {
                                                if let Some(v) = json_value.as_f64() {
                                                    param.value = crate::effects::ParameterValue::Float(v as f32);
                                                } else if let Some(v) = json_value.as_i64() {
                                                    param.value = crate::effects::ParameterValue::Int(v as i32);
                                                } else if let Some(v) = json_value.as_bool() {
                                                    param.value = crate::effects::ParameterValue::Bool(v);
                                                }
                                            }
                                        }
                                    }
                                    tracing::debug!("🌐 API: Updated clip effect {} at layer {} slot {}", effect_id, layer_id, slot);
                                }
                            }
                        }
                    }
                }
                ApiCommand::BypassClipEffect { layer_id, slot, effect_id } => {
                    if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                        if let Some(Some(clip)) = layer.clips.get_mut(slot) {
                            if let Ok(id) = effect_id.parse::<u32>() {
                                if let Some(effect) = clip.effects.get_mut(id) {
                                    effect.bypassed = !effect.bypassed;
                                    tracing::debug!("🌐 API: Toggled bypass for clip effect {} at layer {} slot {}", effect_id, layer_id, slot);
                                }
                            }
                        }
                    }
                }

                // Catch-all for unimplemented commands
                _ => {
                    tracing::warn!("🌐 API: Unimplemented command received: {:?}", cmd);
                }
            }
        }

        // Put the receiver back
        self.api_command_rx = Some(rx);
    }

    // =========================================================================
    // NDI (Network Device Interface) Methods
    // =========================================================================

    /// Set an NDI source as a clip in a layer's clip slots
    pub fn set_layer_ndi_clip(
        &mut self,
        layer_id: u32,
        slot: usize,
        ndi_name: String,
        url_address: Option<String>,
    ) -> bool {
        if let Some(layer) = self.environment.get_layer_mut(layer_id) {
            let cell = crate::compositor::ClipCell::from_ndi(&ndi_name, url_address);
            if layer.set_clip(slot, cell) {
                tracing::info!(
                    "📺 Assigned NDI source '{}' to layer {} slot {}",
                    ndi_name, layer_id, slot
                );
                // Extract display name for status
                let display_name = if let Some(paren_start) = ndi_name.find(" (") {
                    ndi_name[paren_start + 2..].trim_end_matches(')').to_string()
                } else {
                    ndi_name.clone()
                };
                self.menu_bar.set_status(format!("Assigned NDI source: {}", display_name));
                return true;
            }
        }
        false
    }

    /// Start OMT discovery
    pub fn start_omt_discovery(&mut self) {
        if let Some(discovery) = &mut self.omt_discovery {
            if let Err(e) = discovery.start_browsing() {
                tracing::error!("📡 OMT: Failed to start discovery: {}", e);
                self.menu_bar.set_status(format!("OMT discovery failed: {}", e));
            } else {
                tracing::info!("📡 OMT: Discovery started");
                self.settings.omt_discovery_enabled = true;
                self.sources_panel.set_omt_discovery_enabled(true);
                self.menu_bar.set_status("OMT discovery started");
            }
        }
    }

    /// Stop OMT discovery
    pub fn stop_omt_discovery(&mut self) {
        if let Some(discovery) = &mut self.omt_discovery {
            discovery.stop_browsing();
            tracing::info!("📡 OMT: Discovery stopped");
            self.settings.omt_discovery_enabled = false;
            self.sources_panel.set_omt_discovery_enabled(false);
            self.sources_panel.set_omt_sources(Vec::new());
            self.menu_bar.set_status("OMT discovery stopped");
        }
    }

    /// Start NDI discovery
    pub fn start_ndi_discovery(&mut self) {
        if let Some(discovery) = &mut self.omt_discovery {
            match discovery.start_ndi_discovery() {
                Ok(()) => {
                    tracing::info!("📺 NDI: Discovery started");
                    self.settings.ndi_discovery_enabled = true;
                    self.sources_panel.set_ndi_discovery_enabled(true);
                    self.menu_bar.set_status("NDI discovery started");
                }
                Err(e) => {
                    tracing::error!("📺 NDI: Failed to start discovery: {}", e);
                    self.menu_bar.set_status(format!("NDI discovery failed: {}", e));
                }
            }
        }
    }

    /// Stop NDI discovery
    pub fn stop_ndi_discovery(&mut self) {
        if let Some(discovery) = &mut self.omt_discovery {
            discovery.stop_ndi_discovery();
            tracing::info!("📺 NDI: Discovery stopped");
            self.settings.ndi_discovery_enabled = false;
            self.sources_panel.set_ndi_discovery_enabled(false);
            self.sources_panel.set_ndi_sources(Vec::new());
            self.menu_bar.set_status("NDI discovery stopped");
        }
    }

    /// Refresh the list of discovered NDI sources
    pub fn refresh_ndi_sources(&mut self) {
        if let Some(discovery) = &mut self.omt_discovery {
            discovery.refresh();
        }
        self.update_ndi_sources_in_ui();
    }

    /// Update the UI with the current NDI sources (called periodically)
    pub fn update_ndi_sources_in_ui(&mut self) {
        if let Some(discovery) = &self.omt_discovery {
            if discovery.is_ndi_enabled() {
                let sources: Vec<(String, String, Option<String>)> = discovery
                    .get_sources_by_type(crate::network::SourceType::Ndi)
                    .into_iter()
                    .map(|s| {
                        let url_address = s.properties.get("url_address").cloned();
                        let ndi_name = s.id.strip_prefix("ndi:").unwrap_or(&s.id).to_string();
                        (ndi_name, s.name, url_address)
                    })
                    .collect();
                self.sources_panel.set_ndi_sources(sources);
            }
        }
    }

    /// Start NDI broadcast
    pub fn start_ndi_broadcast(&mut self, name: &str) {
        if self.ndi_capture.is_some() {
            tracing::info!("NDI: Broadcast already active");
            return;
        }

        // Create NDI sender with environment's target FPS (synced to render loop)
        let env_fps = self.settings.target_fps;
        match crate::network::NdiSender::new(name, env_fps) {
            Ok(sender) => {
                let w = self.environment.width();
                let h = self.environment.height();
                tracing::info!("📺 NDI: Creating capture pipeline for {}x{} @ {}fps", w, h, env_fps);

                let mut capture = crate::network::NdiCapture::new(&self.device, w, h);
                capture.start_sender_thread(sender);
                self.ndi_capture = Some(capture);
                self.ndi_broadcast_enabled = true;
                self.menu_bar.set_status(format!("NDI broadcast started ({}x{})", w, h));
                tracing::info!("📺 NDI: Capture pipeline started, broadcasting {}x{}", w, h);
            }
            Err(e) => {
                tracing::error!("NDI: Failed to create sender: {}", e);
                self.ndi_broadcast_enabled = false;
                self.settings.ndi_broadcast_enabled = false;
                self.menu_bar.set_status(format!("NDI broadcast failed: {}", e));
            }
        }
    }

    /// Stop NDI broadcast
    pub fn stop_ndi_broadcast(&mut self) {
        if self.ndi_capture.is_some() {
            // Drop capture - this stops the sender thread
            self.ndi_capture = None;
            self.ndi_sender = None;
            self.ndi_broadcast_enabled = false;
            tracing::info!("📺 NDI: Stopped broadcast");
            self.menu_bar.set_status("NDI broadcast stopped");
        }
    }

    /// Check if NDI broadcast is active
    pub fn is_ndi_broadcasting(&self) -> bool {
        self.ndi_capture.as_ref().map(|c| c.is_sender_running()).unwrap_or(false)
    }

    /// Collect video info for all layers (for transport controls in UI)
    pub fn layer_video_info(&self) -> HashMap<u32, crate::layer_runtime::LayerVideoInfo> {
        self.layer_runtimes
            .iter()
            .filter_map(|(id, runtime)| {
                runtime.video_info().map(|info| (*id, info))
            })
            .collect()
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
            tracing::info!("Syphon: Already sharing");
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
                tracing::error!("Syphon: Failed to get Metal device from wgpu");
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
                tracing::info!("Syphon: Started sharing as '{}'", name);
                self.menu_bar
                    .set_status(&format!("Syphon: Sharing as '{}'", name));
            }
            Err(e) => {
                tracing::error!("Syphon: Failed to start: {}", e);
                self.menu_bar.set_status(&format!("Syphon error: {}", e));
            }
        }
    }

    #[cfg(target_os = "windows")]
    pub fn start_texture_sharing(&mut self) {
        if self.spout_capture.is_some() {
            tracing::info!("Spout: Already sharing");
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
                tracing::info!("Spout: Started sharing as '{}'", name);
                self.menu_bar.set_status(&format!("Spout: Sharing as '{}'", name));
            }
            Err(e) => {
                tracing::error!("Spout: Failed to start: {}", e);
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
        tracing::info!("Syphon: Stopped sharing");
        self.menu_bar.set_status("Syphon: Stopped");
    }

    #[cfg(target_os = "windows")]
    pub fn stop_texture_sharing(&mut self) {
        if let Some(ref mut capture) = self.spout_capture {
            capture.stop();
        }
        self.spout_capture = None;
        self.texture_share_enabled = false;
        tracing::info!("Spout: Stopped sharing");
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
                tracing::info!("📋 Copied clip from layer {} slot {}", layer_id, slot);
                self.menu_bar.set_status(format!("Copied clip: {}", clip.display_name()));
            }
        }
    }

    /// Paste a clip from the clipboard to a slot
    pub fn paste_clip(&mut self, layer_id: u32, slot: usize) {
        if let Some(clip) = self.clip_grid_panel.get_clipboard().cloned() {
            if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                layer.set_clip(slot, clip.clone());
                tracing::info!("📋 Pasted clip to layer {} slot {}", layer_id, slot);
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
                tracing::warn!("Cannot clone layer {}: not found", layer_id);
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
                tracing::warn!("Failed to load video for cloned layer: {}", e);
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

        tracing::info!("📋 Cloned layer {} -> {} ({})", layer_id, new_id, new_name);
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
            tracing::info!(
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
                    tracing::error!("Failed to trigger clip: {}", e);
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
                    tracing::info!("Assigned clip to layer {} at slot {} via drag-drop", layer_id, slot);
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
            ClipGridAction::AssignNdiSource { layer_id, slot, ndi_name, url_address } => {
                // Assign an NDI source to a clip slot
                self.set_layer_ndi_clip(layer_id, slot, ndi_name, url_address);
            }
            ClipGridAction::SelectClipForPreview { layer_id, slot } => {
                // Select clip for preview without triggering it
                // Clear layer preview mode (switching to clip mode)
                self.preview_layer_id = None;

                // Clear all existing preview state (switching to new clip)
                self.preview_source_receiver = None;
                self.preview_source_texture = None;
                self.preview_source_output_texture = None;
                self.preview_source_output_view = None;
                self.preview_source_bind_group = None;
                self.preview_source_params_buffer = None;
                self.preview_source_has_frame = false;
                self.preview_player.clear();

                // Load the clip into the preview player
                if let Some(layer) = self.environment.layers().iter().find(|l| l.id == layer_id) {
                    if let Some(clip) = layer.get_clip(slot) {
                        // Set the clip info in the preview panel
                        let source_info = match &clip.source {
                            crate::compositor::ClipSource::File { path } => path.display().to_string(),
                            crate::compositor::ClipSource::Omt { address, .. } => format!("OMT: {}", address),
                            crate::compositor::ClipSource::Ndi { ndi_name, .. } => format!("NDI: {}", ndi_name),
                        };
                        self.preview_monitor_panel.set_preview_clip(crate::ui::PreviewClipInfo {
                            layer_id,
                            slot,
                            name: clip.display_name(),
                            source_info,
                        });

                        // Free old preview texture from egui before loading new one
                        free_egui_texture(&mut self.egui_renderer, &mut self.preview_player.egui_texture_id);

                        // Load the clip into the preview player
                        match &clip.source {
                            crate::compositor::ClipSource::File { path } => {
                                if let Err(e) = self.preview_player.load(path, &self.device, &self.video_renderer) {
                                    tracing::warn!("Failed to load preview: {}", e);
                                    self.menu_bar.set_status(format!("Preview failed: {}", e));
                                } else {
                                    // Register the preview texture with egui for display
                                    // Use ptr to avoid borrowing self.preview_player during texture registration
                                    let texture_view_ptr = self.preview_player.texture_view()
                                        .map(|v| v as *const wgpu::TextureView);
                                    if let Some(view_ptr) = texture_view_ptr {
                                        unsafe {
                                            register_egui_texture_ptr(
                                                &mut self.egui_renderer,
                                                &self.device,
                                                view_ptr,
                                                &mut self.preview_player.egui_texture_id,
                                            );
                                        }
                                        tracing::info!("Preview texture registered with egui");
                                    }
                                }
                            }
                            crate::compositor::ClipSource::Omt { address, name: _ } => {
                                // Set up OMT receiver for clip preview (stay in Clip mode for effects)
                                // OMT receiver not yet implemented
                                tracing::warn!("Clip preview: OMT receiver not yet implemented for '{}'", address);
                            }
                            crate::compositor::ClipSource::Ndi { ndi_name, url_address: _ } => {
                                // Set up NDI receiver for clip preview (stay in Clip mode for effects)
                                tracing::info!("Clip preview: Connecting to NDI source '{}'", ndi_name);
                                match crate::network::NdiReceiver::connect(ndi_name) {
                                    Ok(receiver) => {
                                        self.preview_source_receiver = Some(receiver);
                                        tracing::info!("Clip preview: NDI receiver connected");
                                    }
                                    Err(e) => {
                                        tracing::error!("Clip preview: Failed to connect to NDI source '{}': {}", ndi_name, e);
                                        self.menu_bar.set_status(format!("NDI connect failed: {}", e));
                                    }
                                }
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
            ClipGridAction::SelectLayerForPreview { layer_id } => {
                // Select layer for preview (show live layer output with effects)
                if let Some(layer) = self.environment.layers().iter().find(|l| l.id == layer_id) {
                    // Set the layer info in the preview panel
                    self.preview_monitor_panel.set_preview_layer(crate::ui::PreviewLayerInfo {
                        layer_id,
                        name: layer.name.clone(),
                    });

                    // Store the layer ID for preview rendering
                    self.preview_layer_id = Some(layer_id);

                    // Clear all existing preview state (switching to layer mode)
                    self.preview_source_receiver = None;
                    self.preview_source_texture = None;
                    self.preview_source_output_texture = None;
                    self.preview_source_output_view = None;
                    self.preview_source_bind_group = None;
                    self.preview_source_params_buffer = None;
                    self.preview_source_has_frame = false;
                    self.preview_player.clear();

                    // Free old preview texture from egui
                    free_egui_texture(&mut self.egui_renderer, &mut self.preview_player.egui_texture_id);

                    // Open preview monitor panel
                    if let Some(p) = self.dock_manager.get_panel_mut(crate::ui::dock::panel_ids::PREVIEW_MONITOR) {
                        p.open = true;
                    }

                    tracing::info!("Layer preview selected: {} (id={})", layer.name, layer_id);
                }
            }
            ClipGridAction::LaunchColumn { column_index } => {
                // Launch all clips in a column (like Resolume's column launch)
                let layer_ids: Vec<u32> = self.environment.layers().iter().map(|l| l.id).collect();
                for layer_id in layer_ids {
                    if let Err(e) = self.trigger_clip(layer_id, column_index) {
                        tracing::debug!("Skipping layer {} column {}: {}", layer_id, column_index, e);
                    }
                }
                self.menu_bar.set_status(format!("Launched column {}", column_index + 1));
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
            PropertiesAction::SetNdiBroadcast { enabled } => {
                self.settings.ndi_broadcast_enabled = enabled;
                self.ndi_broadcast_enabled = enabled;
                if enabled {
                    self.start_ndi_broadcast("Immersive Server");
                } else {
                    self.stop_ndi_broadcast();
                }
            }
            PropertiesAction::SetNdiCaptureFps { fps: _ } => {
                // NDI capture is now synced to environment FPS (target_fps).
                // This setting is ignored - NDI sends every rendered frame.
            }
            PropertiesAction::SetNdiBufferCapacity { capacity } => {
                self.settings.ndi_buffer_capacity = capacity;
                crate::network::ndi::set_ndi_buffer_capacity(capacity);
            }
            PropertiesAction::SetOmtDiscovery { enabled } => {
                if enabled {
                    self.start_omt_discovery();
                } else {
                    self.stop_omt_discovery();
                }
            }
            PropertiesAction::SetNdiDiscovery { enabled } => {
                if enabled {
                    self.start_ndi_discovery();
                } else {
                    self.stop_ndi_discovery();
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
                    tracing::warn!("Texture sharing not available on this platform");
                }
            }
            PropertiesAction::SetApiServer { enabled } => {
                self.settings.api_server_enabled = enabled;
                if enabled && !self.api_server_running {
                    self.start_api_server();
                }
                // Note: We don't stop the server when disabled - it requires app restart
                // This is because the server runs in a background thread and graceful shutdown
                // would add complexity. The setting will take effect on next app start.
                if !enabled && self.api_server_running {
                    self.menu_bar.set_status("API server will stop on restart");
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
                        tracing::info!("Added effect '{}' to layer {}", display_name, layer_id);
                    }
                }
            }
            PropertiesAction::RemoveLayerEffect { layer_id, effect_id } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    layer.effects.remove(effect_id);
                    tracing::info!("Removed effect {} from layer {}", effect_id, layer_id);
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
                            tracing::info!("Added effect '{}' to clip {} on layer {}", display_name, slot, layer_id);
                        }
                    }
                }
            }
            PropertiesAction::RemoveClipEffect { layer_id, slot, effect_id } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    if let Some(clip) = layer.get_clip_mut(slot) {
                        clip.effects.remove(effect_id);
                        tracing::info!("Removed effect {} from clip {} on layer {}", effect_id, slot, layer_id);
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
                    tracing::info!("Added master effect '{}'", display_name);
                }
            }
            PropertiesAction::RemoveEnvironmentEffect { effect_id } => {
                self.environment.effects_mut().remove(effect_id);
                tracing::info!("Removed master effect {}", effect_id);
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
            PropertiesAction::SetLayerEffectExpanded { layer_id, effect_id, expanded } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    if let Some(effect) = layer.effects.get_mut(effect_id) {
                        effect.expanded = expanded;
                    }
                }
            }
            PropertiesAction::SetClipEffectExpanded { layer_id, slot, effect_id, expanded } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    if let Some(clip) = layer.get_clip_mut(slot) {
                        if let Some(effect) = clip.effects.get_mut(effect_id) {
                            effect.expanded = expanded;
                        }
                    }
                }
            }
            PropertiesAction::SetEnvironmentEffectExpanded { effect_id, expanded } => {
                if let Some(effect) = self.environment.effects_mut().get_mut(effect_id) {
                    effect.expanded = expanded;
                }
            }
            PropertiesAction::SetLayerEffectParameterAutomation { layer_id, effect_id, param_name, automation } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    if let Some(effect) = layer.effects.get_mut(effect_id) {
                        if let Some(param) = effect.get_parameter_mut(&param_name) {
                            param.automation = automation;
                        }
                    }
                }
            }
            PropertiesAction::SetClipEffectParameterAutomation { layer_id, slot, effect_id, param_name, automation } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    if let Some(clip) = layer.get_clip_mut(slot) {
                        if let Some(effect) = clip.effects.get_mut(effect_id) {
                            if let Some(param) = effect.get_parameter_mut(&param_name) {
                                param.automation = automation;
                            }
                        }
                    }
                }
            }
            PropertiesAction::SetEnvironmentEffectParameterAutomation { effect_id, param_name, automation } => {
                if let Some(effect) = self.environment.effects_mut().get_mut(effect_id) {
                    if let Some(param) = effect.get_parameter_mut(&param_name) {
                        param.automation = automation;
                    }
                }
            }
            // Clip transform actions
            PropertiesAction::SetClipPosition { layer_id, slot, x, y } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    if let Some(clip) = layer.get_clip_mut(slot) {
                        clip.transform.position = (x, y);
                    }
                }
            }
            PropertiesAction::SetClipScale { layer_id, slot, scale_x, scale_y } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    if let Some(clip) = layer.get_clip_mut(slot) {
                        clip.transform.scale = (scale_x, scale_y);
                    }
                }
            }
            PropertiesAction::SetClipRotation { layer_id, slot, degrees } => {
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    if let Some(clip) = layer.get_clip_mut(slot) {
                        clip.transform.rotation = degrees.to_radians();
                    }
                }
            }

            // Clip transport actions
            PropertiesAction::ToggleClipPlayback { layer_id } => {
                if let Some(runtime) = self.layer_runtimes.get(&layer_id) {
                    if let Some(player) = &runtime.player {
                        player.toggle_pause();
                    }
                }
            }
            PropertiesAction::RestartClip { layer_id } => {
                if let Some(runtime) = self.layer_runtimes.get(&layer_id) {
                    if let Some(player) = &runtime.player {
                        player.restart();
                    }
                }
            }
            PropertiesAction::SeekClip { layer_id, time_secs } => {
                if let Some(runtime) = self.layer_runtimes.get(&layer_id) {
                    runtime.seek(time_secs);
                }
            }
            PropertiesAction::PreviewClip { layer_id, slot } => {
                // Trigger the same action as right-click preview in clip grid
                self.handle_clip_action(crate::ui::ClipGridAction::SelectClipForPreview {
                    layer_id,
                    slot,
                });
            }
            PropertiesAction::SetClipLoopMode { layer_id, slot, mode } => {
                // Check if this clip is currently playing first (before borrowing mutably)
                let is_active_clip = self.environment.get_layer(layer_id)
                    .map(|l| l.active_clip == Some(slot))
                    .unwrap_or(false);

                // Update clip data
                if let Some(layer) = self.environment.get_layer_mut(layer_id) {
                    if let Some(Some(clip)) = layer.clips.get_mut(slot) {
                        clip.loop_mode = mode;
                    }
                }

                // Update active player if this clip is currently playing
                if is_active_clip {
                    if let Some(runtime) = self.layer_runtimes.get(&layer_id) {
                        if let Some(player) = &runtime.player {
                            player.set_loop_mode(mode.as_u8());
                        }
                    }
                }
            }
            PropertiesAction::SetFloorSyncEnabled { enabled } => {
                self.settings.floor_sync_enabled = enabled;
            }
            PropertiesAction::SetFloorLayerIndex { index } => {
                self.settings.floor_layer_index = index;
            }
            PropertiesAction::SetLowLatencyMode { enabled } => {
                self.settings.low_latency_mode = enabled;
                // Note: sync_low_latency_mode() will reconfigure the surface on next render
            }
            PropertiesAction::SetVsyncEnabled { enabled } => {
                self.settings.vsync_enabled = enabled;
                // Note: sync_vsync_mode() will reconfigure the surface on next render
            }
            PropertiesAction::SetTestPattern { enabled } => {
                self.settings.test_pattern_enabled = enabled;
                self.environment.set_test_pattern_enabled(enabled);
                self.menu_bar.set_status(if enabled {
                    "Test pattern enabled"
                } else {
                    "Test pattern disabled"
                });
            }
            PropertiesAction::SetBgraPipelineEnabled { enabled } => {
                self.settings.bgra_pipeline_enabled = enabled;
                tracing::info!("BGRA pipeline mode set to {}. Restart required.", enabled);
                self.menu_bar.set_status(if enabled {
                    "BGRA pipeline enabled (restart required)"
                } else {
                    "BGRA pipeline disabled (restart required)"
                });
            }
            PropertiesAction::StartScrub { layer_id } => {
                // Store whether the video was playing before scrubbing
                if let Some(runtime) = self.layer_runtimes.get(&layer_id) {
                    let was_playing = !runtime.is_paused();
                    self.scrub_was_playing.insert(layer_id, was_playing);
                    // Pause during scrub
                    if let Some(player) = &runtime.player {
                        player.pause();
                    }
                }
            }
            PropertiesAction::EndScrub { layer_id, time_secs } => {
                // Final seek to the scrub position, then restore previous play state
                if let Some(runtime) = self.layer_runtimes.get(&layer_id) {
                    runtime.seek(time_secs);
                }
                if let Some(was_playing) = self.scrub_was_playing.remove(&layer_id) {
                    if was_playing {
                        if let Some(runtime) = self.layer_runtimes.get(&layer_id) {
                            if let Some(player) = &runtime.player {
                                player.resume();
                            }
                        }
                    }
                }
            }
            PropertiesAction::SetAudioSource { source_type } => {
                use crate::settings::AudioSourceType;
                use std::time::Instant;
                let total_start = Instant::now();
                tracing::debug!("[AUDIO] SetAudioSource handler started, source_type={:?}", source_type);

                // Update settings
                self.settings.audio_source = source_type.clone();

                // Clear existing audio sources
                let clear_start = Instant::now();
                self.audio_manager.clear_sources();
                tracing::debug!("[AUDIO] SetAudioSource: clear_sources() took {:?}", clear_start.elapsed());

                // Initialize new audio source based on type
                let init_start = Instant::now();
                match source_type {
                    AudioSourceType::None => {
                        tracing::info!("Audio source disabled");
                        self.menu_bar.set_status("Audio source disabled");
                    }
                    AudioSourceType::SystemDefault => {
                        match self.audio_manager.init_system_audio() {
                            Ok(()) => {
                                self.menu_bar.set_status("Audio: System default");
                            }
                            Err(e) => {
                                tracing::warn!("Failed to init system audio: {}", e);
                                self.menu_bar.set_status("Audio: Failed to initialize");
                            }
                        }
                    }
                    AudioSourceType::SystemDevice(ref device_name) => {
                        match self.audio_manager.init_system_audio_device(Some(device_name)) {
                            Ok(()) => {
                                self.menu_bar.set_status(format!("Audio: {}", device_name));
                            }
                            Err(e) => {
                                tracing::warn!("Failed to init device '{}': {}", device_name, e);
                                self.menu_bar.set_status("Audio: Device failed");
                            }
                        }
                    }
                    AudioSourceType::Ndi(ref ndi_name) => {
                        // For NDI audio, we need to store the audio state and pass it when
                        // creating NDI receivers. For now, just create the audio source.
                        let _audio_state = self.audio_manager.add_ndi_source(ndi_name);
                        // Note: The audio state needs to be passed to NDI receivers when
                        // they are created/connected. This is stored in the manager.
                        self.menu_bar.set_status(format!("Audio: NDI {}", ndi_name));
                        tracing::info!("NDI audio source added: {}", ndi_name);
                    }
                    AudioSourceType::Omt(ref address) => {
                        // For OMT audio, similar to NDI
                        let _audio_state = self.audio_manager.add_omt_source(address);
                        self.menu_bar.set_status(format!("Audio: OMT {}", address));
                        tracing::info!("OMT audio source added: {}", address);
                    }
                }
                tracing::debug!("[AUDIO] SetAudioSource: init took {:?}", init_start.elapsed());
                tracing::debug!("[AUDIO] SetAudioSource handler total took {:?}", total_start.elapsed());
            }
            PropertiesAction::SetFftGain { gain } => {
                self.settings.fft_gain = gain;
                self.audio_manager.set_master_sensitivity(gain);
                tracing::debug!("[AUDIO] FFT gain set to {:.2}x", gain);
            }
        }
    }

    /// Handle an advanced output window action from the UI
    fn handle_advanced_output_action(&mut self, action: crate::ui::AdvancedOutputAction) {
        use crate::ui::AdvancedOutputAction;

        match action {
            AdvancedOutputAction::AddScreen => {
                // Ensure output manager exists first
                let _ = self.ensure_output_manager();
                // Now get screen count before adding
                let screen_num = self.output_manager.as_ref().map(|m| m.screen_count() + 1).unwrap_or(1);
                let screen_name = format!("Screen {}", screen_num);
                // Add screen (borrows device and manager separately)
                let env_dims = (self.environment.width(), self.environment.height());
                if let Some(manager) = self.output_manager.as_mut() {
                    let screen_id = manager.add_screen(&self.device, &screen_name, env_dims);
                    // Auto-select the newly added screen
                    self.advanced_output_window.select_screen(screen_id, &screen_name, 1920, 1080);
                    self.advanced_output_window.mark_dirty();
                    self.menu_bar.set_status(format!("Added Screen {}", screen_num));
                    tracing::info!("Added screen {:?}", screen_id);
                }
            }
            AdvancedOutputAction::RemoveScreen { screen_id } => {
                if let Some(manager) = self.output_manager.as_mut() {
                    manager.remove_screen(screen_id);
                    self.advanced_output_window.mark_dirty();
                    self.menu_bar.set_status("Removed screen");
                }
            }
            AdvancedOutputAction::AddSlice { screen_id } => {
                if let Some(manager) = self.output_manager.as_mut() {
                    let slice_num = manager.get_screen(screen_id).map(|s| s.slices.len() + 1).unwrap_or(1);
                    if let Some(slice_id) = manager.add_slice(&self.device, screen_id, format!("Slice {}", slice_num)) {
                        self.advanced_output_window.mark_dirty();
                        self.menu_bar.set_status(format!("Added Slice {}", slice_num));
                        tracing::info!("Added slice {:?} to screen {:?}", slice_id, screen_id);
                    }
                }
            }
            AdvancedOutputAction::RemoveSlice { screen_id, slice_id } => {
                if let Some(manager) = self.output_manager.as_mut() {
                    manager.remove_slice(screen_id, slice_id);
                    self.advanced_output_window.mark_dirty();
                    self.menu_bar.set_status("Removed slice");
                }
            }
            AdvancedOutputAction::MoveSliceUp { screen_id, slice_id } => {
                if let Some(manager) = self.output_manager.as_mut() {
                    if let Some(screen) = manager.get_screen_mut(screen_id) {
                        if let Some(idx) = screen.slices.iter().position(|s| s.id == slice_id) {
                            if idx > 0 {
                                screen.slices.swap(idx, idx - 1);
                            }
                        }
                    }
                    self.advanced_output_window.mark_dirty();
                    // Sync runtime to pick up changes
                    let target_fps = self.settings.target_fps as f32;
                    let tokio_handle = self.tokio_runtime.as_ref().map(|rt| rt.handle());
                    manager.sync_runtime(&self.device, screen_id, target_fps, tokio_handle);
                }
            }
            AdvancedOutputAction::MoveSliceDown { screen_id, slice_id } => {
                if let Some(manager) = self.output_manager.as_mut() {
                    if let Some(screen) = manager.get_screen_mut(screen_id) {
                        if let Some(idx) = screen.slices.iter().position(|s| s.id == slice_id) {
                            if idx < screen.slices.len() - 1 {
                                screen.slices.swap(idx, idx + 1);
                            }
                        }
                    }
                    self.advanced_output_window.mark_dirty();
                    // Sync runtime to pick up changes
                    let target_fps = self.settings.target_fps as f32;
                    let tokio_handle = self.tokio_runtime.as_ref().map(|rt| rt.handle());
                    manager.sync_runtime(&self.device, screen_id, target_fps, tokio_handle);
                }
            }
            AdvancedOutputAction::UpdateSlice { screen_id, slice_id, slice } => {
                if let Some(manager) = self.output_manager.as_mut() {
                    // Update slice data on the screen
                    if let Some(screen) = manager.get_screen_mut(screen_id) {
                        if let Some(existing) = screen.slices.iter_mut().find(|s| s.id == slice_id) {
                            *existing = slice;
                        }
                    }
                    self.advanced_output_window.mark_dirty();
                    // Sync runtime to pick up changes
                    let target_fps = self.settings.target_fps as f32;
                    let tokio_handle = self.tokio_runtime.as_ref().map(|rt| rt.handle());
                    manager.sync_runtime(&self.device, screen_id, target_fps, tokio_handle);
                }
            }
            AdvancedOutputAction::UpdateScreen { screen_id, screen: updated_screen } => {
                if let Some(manager) = self.output_manager.as_mut() {
                    // Update screen data
                    if let Some(screen) = manager.get_screen_mut(screen_id) {
                        screen.name = updated_screen.name;
                        screen.enabled = updated_screen.enabled;
                        screen.device = updated_screen.device.clone();
                        screen.delay_ms = updated_screen.delay_ms;
                        screen.color = updated_screen.color.clone();
                        // Note: width/height changes require texture recreation
                        if screen.width != updated_screen.width || screen.height != updated_screen.height {
                            screen.width = updated_screen.width;
                            screen.height = updated_screen.height;
                        }
                    }
                    self.advanced_output_window.mark_dirty();
                    // Sync runtime to pick up changes
                    let target_fps = self.settings.target_fps as f32;
                    let tokio_handle = self.tokio_runtime.as_ref().map(|rt| rt.handle());
                    manager.sync_runtime(&self.device, screen_id, target_fps, tokio_handle);
                }
            }
            AdvancedOutputAction::UpdateScreenInputRect { screen_id, input_rect } => {
                // Update input_rect on all slices of this screen
                if let Some(manager) = self.output_manager.as_mut() {
                    if let Some(screen) = manager.get_screen_mut(screen_id) {
                        for slice in &mut screen.slices {
                            slice.input_rect.x = input_rect.x;
                            slice.input_rect.y = input_rect.y;
                            slice.input_rect.width = input_rect.width;
                            slice.input_rect.height = input_rect.height;
                        }
                    }
                    self.advanced_output_window.mark_dirty();
                    // Sync runtime to pick up changes
                    let target_fps = self.settings.target_fps as f32;
                    let tokio_handle = self.tokio_runtime.as_ref().map(|rt| rt.handle());
                    manager.sync_runtime(&self.device, screen_id, target_fps, tokio_handle);
                }
            }
            AdvancedOutputAction::UpdateSliceInputRect { screen_id, slice_id, input_rect } => {
                // Update input_rect on a specific slice
                if let Some(manager) = self.output_manager.as_mut() {
                    if let Some(screen) = manager.get_screen_mut(screen_id) {
                        if let Some(slice) = screen.slices.iter_mut().find(|s| s.id == slice_id) {
                            slice.input_rect.x = input_rect.x;
                            slice.input_rect.y = input_rect.y;
                            slice.input_rect.width = input_rect.width;
                            slice.input_rect.height = input_rect.height;
                        }
                    }
                    self.advanced_output_window.mark_dirty();
                    // Sync runtime to pick up changes
                    let target_fps = self.settings.target_fps as f32;
                    let tokio_handle = self.tokio_runtime.as_ref().map(|rt| rt.handle());
                    manager.sync_runtime(&self.device, screen_id, target_fps, tokio_handle);
                }
            }
            AdvancedOutputAction::SaveComposition => {
                // Auto-save when closing the Advanced Output window
                if let Some(path) = self.current_file.clone() {
                    // Sync current state to settings
                    self.sync_layers_to_settings();
                    // Save to file
                    if let Err(e) = self.settings.save_to_file(&path) {
                        tracing::error!("Failed to auto-save composition: {}", e);
                        self.menu_bar.set_status(format!("Failed to save: {}", e));
                    } else {
                        tracing::info!("Auto-saved composition to: {}", path.display());
                        self.menu_bar.set_status("Composition saved");
                    }
                }
            }
            AdvancedOutputAction::LoadPreset { name } => {
                // Load screens from preset and apply to output manager
                if let Some(preset) = self.output_preset_manager.get_preset_by_name(&name) {
                    let screens = preset.screens.clone();
                    self.apply_output_preset_screens(screens);
                    self.advanced_output_window.set_current_preset(Some(name.clone()));
                    self.output_preset_manager.set_active_preset_by_name(&name);
                    self.menu_bar.set_status(format!("Loaded preset: {}", name));
                    tracing::info!("Loaded output preset: {}", name);
                } else {
                    self.menu_bar.set_status(format!("Preset not found: {}", name));
                }
            }
            AdvancedOutputAction::SaveAsPreset { name } => {
                // Get current screens from output manager and save as preset
                if let Some(manager) = &self.output_manager {
                    let screens: Vec<_> = manager.screens().cloned().collect();
                    match self.output_preset_manager.save_as_preset(&name, screens) {
                        Ok(()) => {
                            self.advanced_output_window.set_current_preset(Some(name.clone()));
                            self.menu_bar.set_status(format!("Saved preset: {}", name));
                            tracing::info!("Saved output preset: {}", name);
                        }
                        Err(e) => {
                            self.menu_bar.set_status(format!("Failed to save preset: {}", e));
                            tracing::error!("Failed to save output preset: {}", e);
                        }
                    }
                } else {
                    self.menu_bar.set_status("No output configuration to save");
                }
            }
            AdvancedOutputAction::DeletePreset { name } => {
                // Delete a user preset
                match self.output_preset_manager.delete_preset(&name) {
                    Ok(()) => {
                        // If deleted preset was current, clear it
                        if self.advanced_output_window.current_preset_name.as_deref() == Some(&name) {
                            self.advanced_output_window.set_current_preset(None);
                        }
                        self.menu_bar.set_status(format!("Deleted preset: {}", name));
                        tracing::info!("Deleted output preset: {}", name);
                    }
                    Err(e) => {
                        self.menu_bar.set_status(format!("Failed to delete preset: {}", e));
                        tracing::error!("Failed to delete output preset: {}", e);
                    }
                }
            }
            AdvancedOutputAction::NewConfiguration => {
                // Create a new configuration with a single virtual screen
                let screens = vec![crate::output::Screen::new_with_default_slice(
                    crate::output::ScreenId(1),
                    "Screen 1",
                    crate::output::SliceId(1),
                )];
                self.apply_output_preset_screens(screens);
                self.advanced_output_window.set_current_preset(None);
                self.advanced_output_window.clear_dirty();
                self.menu_bar.set_status("Created new output configuration");
                tracing::info!("Created new output configuration");
            }
        }
    }

    /// Apply screens from a preset to the output manager
    fn apply_output_preset_screens(&mut self, screens: Vec<crate::output::Screen>) {
        // Ensure output manager exists
        let _ = self.ensure_output_manager();

        let env_dims = (self.environment.width(), self.environment.height());
        if let Some(manager) = self.output_manager.as_mut() {
            // Clear existing screens
            let existing_ids: Vec<_> = manager.screens().map(|s| s.id).collect();
            for id in existing_ids {
                manager.remove_screen(id);
            }

            // Add new screens from preset
            for screen in screens {
                manager.add_screen_from_data(&self.device, screen, env_dims);
            }

            // Sync all screen runtimes
            let target_fps = self.settings.target_fps as f32;
            let tokio_handle = self.tokio_runtime.as_ref().map(|rt| rt.handle());
            for screen_id in manager.screens().map(|s| s.id).collect::<Vec<_>>() {
                manager.sync_runtime(&self.device, screen_id, target_fps, tokio_handle);
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
            SourcesAction::RefreshNdiSources => {
                self.refresh_ndi_sources();
            }
            SourcesAction::StartOmtDiscovery => {
                self.start_omt_discovery();
            }
            SourcesAction::StopOmtDiscovery => {
                self.stop_omt_discovery();
            }
            SourcesAction::StartNdiDiscovery => {
                self.start_ndi_discovery();
            }
            SourcesAction::StopNdiDiscovery => {
                self.stop_ndi_discovery();
            }
            SourcesAction::SelectSourceForPreview { source } => {
                self.select_source_for_preview(source);
            }
        }
    }

    /// Select a network source for preview in the Preview Monitor
    fn select_source_for_preview(&mut self, source: crate::ui::DraggableSource) {
        use crate::network::SourceType;
        use crate::ui::{DraggableSource, PreviewSourceInfo};

        // Clear any existing preview mode
        self.preview_layer_id = None;
        self.preview_player.clear();

        // Clear existing source preview
        self.preview_source_receiver = None;
        self.preview_source_texture = None;
        self.preview_source_output_texture = None;
        self.preview_source_output_view = None;
        self.preview_source_bind_group = None;
        self.preview_source_params_buffer = None;
        self.preview_source_has_frame = false;

        // Free old preview texture from egui
        free_egui_texture(&mut self.egui_renderer, &mut self.preview_player.egui_texture_id);

        match &source {
            DraggableSource::Ndi { ndi_name, display_name, url_address } => {
                tracing::info!("Source preview: Connecting to NDI source '{}'", ndi_name);

                // Connect to NDI source
                match crate::network::NdiReceiver::connect(ndi_name) {
                    Ok(receiver) => {
                        self.preview_source_receiver = Some(receiver);

                        // Set preview mode to Source
                        self.preview_monitor_panel.set_preview_source(PreviewSourceInfo {
                            source_type: SourceType::Ndi,
                            name: display_name.clone(),
                            ndi_name: Some(ndi_name.clone()),
                            address: url_address.clone(),
                        });
                    }
                    Err(e) => {
                        tracing::error!("Source preview: Failed to connect to NDI source '{}': {}", ndi_name, e);
                    }
                }
            }
            DraggableSource::Omt { name, address, .. } => {
                // OMT receiver not yet implemented
                tracing::warn!("Source preview: OMT receiver not yet implemented for '{}'", name);

                // Set preview mode to Source anyway to show "Connecting..." state
                self.preview_monitor_panel.set_preview_source(PreviewSourceInfo {
                    source_type: SourceType::Omt,
                    name: name.clone(),
                    ndi_name: None,
                    address: Some(address.clone()),
                });
            }
            DraggableSource::File { path: _, name } => {
                // File sources should use clip preview, not source preview
                tracing::warn!("Source preview: File source '{}' should use clip preview", name);
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
            PreviewMonitorAction::SeekTo { time_secs } => {
                self.preview_player.seek(time_secs);
            }
            PreviewMonitorAction::StartScrub => {
                // Store whether video was playing before scrubbing
                self.scrub_was_playing_preview = !self.preview_player.is_paused();
                // Pause during scrub
                self.preview_player.pause();
            }
            PreviewMonitorAction::EndScrub { time_secs } => {
                // Final seek to scrub position
                self.preview_player.seek(time_secs);
                // Restore previous play state
                if self.scrub_was_playing_preview {
                    self.preview_player.resume();
                }
            }
            PreviewMonitorAction::TriggerToLayer { layer_id, slot } => {
                // Trigger the previewed clip to its layer (go live)
                if let Err(e) = self.trigger_clip(layer_id, slot) {
                    tracing::error!("Failed to trigger clip from preview: {}", e);
                    self.menu_bar.set_status(format!("Failed to trigger clip: {}", e));
                }
            }
        }
    }

    /// Handle a previs panel action from the UI
    fn handle_previs_action(&mut self, action: crate::ui::PrevisAction) {
        use crate::ui::{PrevisAction, WallId};

        match action {
            PrevisAction::SetSurfaceType(surface_type) => {
                self.settings.previs_settings.surface_type = surface_type;
            }
            PrevisAction::SetEnabled(enabled) => {
                self.settings.previs_settings.enabled = enabled;
            }
            PrevisAction::SetCircleRadius(radius) => {
                self.settings.previs_settings.circle_radius = radius;
            }
            PrevisAction::SetCircleSegments(segments) => {
                self.settings.previs_settings.circle_segments = segments;
            }
            PrevisAction::SetWallEnabled(wall_id, enabled) => {
                let wall = match wall_id {
                    WallId::Front => &mut self.settings.previs_settings.wall_front,
                    WallId::Back => &mut self.settings.previs_settings.wall_back,
                    WallId::Left => &mut self.settings.previs_settings.wall_left,
                    WallId::Right => &mut self.settings.previs_settings.wall_right,
                };
                wall.enabled = enabled;
            }
            PrevisAction::SetWallWidth(wall_id, width) => {
                let wall = match wall_id {
                    WallId::Front => &mut self.settings.previs_settings.wall_front,
                    WallId::Back => &mut self.settings.previs_settings.wall_back,
                    WallId::Left => &mut self.settings.previs_settings.wall_left,
                    WallId::Right => &mut self.settings.previs_settings.wall_right,
                };
                wall.width = width;
            }
            PrevisAction::SetWallHeight(wall_id, height) => {
                let wall = match wall_id {
                    WallId::Front => &mut self.settings.previs_settings.wall_front,
                    WallId::Back => &mut self.settings.previs_settings.wall_back,
                    WallId::Left => &mut self.settings.previs_settings.wall_left,
                    WallId::Right => &mut self.settings.previs_settings.wall_right,
                };
                wall.height = height;
            }
            PrevisAction::SetDomeRadius(radius) => {
                self.settings.previs_settings.dome_radius = radius;
            }
            PrevisAction::SetDomeSegmentsH(segments) => {
                self.settings.previs_settings.dome_segments_horizontal = segments;
            }
            PrevisAction::SetDomeSegmentsV(segments) => {
                self.settings.previs_settings.dome_segments_vertical = segments;
            }
            PrevisAction::SaveCameraState { yaw, pitch, distance } => {
                self.settings.previs_settings.camera_yaw = yaw;
                self.settings.previs_settings.camera_pitch = pitch;
                self.settings.previs_settings.camera_distance = distance;
            }
            PrevisAction::ResetCamera => {
                if let Some(renderer) = &mut self.previs_renderer {
                    renderer.camera_mut().reset();
                    // Update settings with default values
                    self.settings.previs_settings.camera_yaw = 0.0;
                    self.settings.previs_settings.camera_pitch = 0.3;
                    self.settings.previs_settings.camera_distance = 10.0;
                }
            }
            PrevisAction::SetFloorEnabled(enabled) => {
                self.settings.previs_settings.floor_enabled = enabled;
            }
            PrevisAction::SetFloorLayerIndex(index) => {
                self.settings.previs_settings.floor_layer_index = index;
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
                tracing::info!("Assigned clip to layer {} at slot {}", layer_id, slot);
                self.menu_bar.set_status(format!("Assigned clip to slot {}", slot + 1));
            } else {
                tracing::error!("Failed to assign clip to layer {} at slot {}", layer_id, slot);
            }
        } else {
            // No pending assignment - this is a regular video load (legacy)
            if let Err(e) = self.load_video(&path) {
                tracing::error!("Failed to load video: {}", e);
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

    /// Toggle test pattern mode on/off
    pub fn toggle_test_pattern(&mut self) {
        let enabled = !self.settings.test_pattern_enabled;
        self.settings.test_pattern_enabled = enabled;
        self.environment.set_test_pattern_enabled(enabled);
        self.menu_bar.set_status(if enabled {
            "Test pattern enabled"
        } else {
            "Test pattern disabled"
        });
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
        let response = viewport_widget::handle_winit_right_mouse_down(
            &mut self.viewport,
            (x, y),
            &ViewportConfig::default(),
        );
        if response.changed {
            self.update_present_params();
        }
        response.was_reset
    }

    /// Handle right mouse button release
    pub fn on_right_mouse_up(&mut self) {
        viewport_widget::handle_winit_right_mouse_up(&mut self.viewport);
    }

    /// Handle mouse movement
    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        self.cursor_position = (x, y);
        let window_size = (self.size.width as f32, self.size.height as f32);
        let env_size = (self.environment.width() as f32, self.environment.height() as f32);
        let response = viewport_widget::handle_winit_mouse_move(
            &mut self.viewport,
            (x, y),
            window_size,
            env_size,
        );
        if response.changed {
            self.update_present_params();
        }
    }

    /// Handle scroll wheel for zooming
    pub fn on_scroll(&mut self, delta: f32) {
        let window_size = (self.size.width as f32, self.size.height as f32);
        let env_size = (self.environment.width() as f32, self.environment.height() as f32);

        // Main window scroll is already normalized in main.rs (PixelDelta divided by 50)
        // Use sensitivity=1.0 to avoid double-division
        let config = ViewportConfig {
            scroll_sensitivity: 1.0,
            scroll_threshold: 0.001, // Match original threshold from main.rs
            double_click_reset: true,
            pan_sensitivity: 1.0,
        };

        let response = viewport_widget::handle_winit_scroll(
            &mut self.viewport,
            delta,
            self.cursor_position,
            window_size,
            env_size,
            &config,
        );
        if response.changed {
            self.update_present_params();
        }
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

        // Update main environment viewport
        if self.viewport.needs_update() {
            let window_size = (self.size.width as f32, self.size.height as f32);
            let env_size = (self.environment.width() as f32, self.environment.height() as f32);
            self.viewport.update(dt, window_size, env_size);
            self.update_present_params();
        }

        // Update preview monitor viewport
        if self.preview_monitor_panel.viewport().needs_update() {
            // Use environment size as content size for preview (layers are environment-sized)
            let env_size = (self.environment.width() as f32, self.environment.height() as f32);
            // Preview size is approximate - the actual size depends on the panel layout
            // Using a reasonable default that matches typical preview panel dimensions
            let preview_size = (320.0, 180.0);
            self.preview_monitor_panel.update_viewport(dt, preview_size, env_size);
        }
    }

    /// Get current zoom level (for UI display)
    pub fn viewport_zoom(&self) -> f32 {
        self.viewport.zoom()
    }

    /// Render the environment texture to an external surface (e.g., environment breakout window)
    ///
    /// This method renders the composed environment texture to the provided surface view
    /// using the copy pipeline, applying proper aspect ratio scaling.
    pub fn render_environment_to_surface(
        &self,
        surface_view: &wgpu::TextureView,
        window_width: u32,
        window_height: u32,
    ) {
        // Calculate viewport params for this window size
        let window_size = (window_width as f32, window_height as f32);
        let env_size = (self.environment.width() as f32, self.environment.height() as f32);

        // Get shader params from viewport (applies current zoom/pan state)
        let (scale_x, scale_y, offset_x, offset_y) = self.viewport.get_shader_params(window_size, env_size);

        let params = VideoParams {
            scale: [scale_x, scale_y],
            offset: [offset_x, offset_y],
            opacity: 1.0,
            _padding: [0.0; 3],
        };

        // Update the copy params buffer
        self.queue.write_buffer(&self.copy_params_buffer, 0, bytemuck::bytes_of(&params));

        // Create command encoder
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Environment Window Encoder"),
        });

        // Render pass: clear to black and draw environment
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Environment Window Present Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
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

        // Submit GPU work
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    // ========================================================================
    // Tiled Layout Rendering
    // ========================================================================

    /// Render the tiled layout UI.
    ///
    /// This is the new grid-based panel layout system that replaces the legacy
    /// dock manager. Panels fill 100% of available space in a binary split tree.
    pub fn render_tiled_ui(&mut self, ctx: &egui::Context, available_rect: egui::Rect) {
        use crate::ui::dock::panel_ids;
        use crate::ui::tiled_layout::{SplitDirection, DropTarget, can_panel_undock};

        // Compute layout rectangles from the tree
        let layout = self.tiled_layout.compute_layout(available_rect);

        // Handle divider hover - change cursor when over a divider
        let hover_pos = ctx.input(|i| i.pointer.hover_pos());
        let mut hovered_divider: Option<usize> = None;

        if let Some(pos) = hover_pos {
            for (idx, divider) in layout.dividers.iter().enumerate() {
                if divider.rect.contains(pos) {
                    hovered_divider = Some(idx);
                    let cursor = match divider.direction {
                        SplitDirection::Horizontal => egui::CursorIcon::ResizeHorizontal,
                        SplitDirection::Vertical => egui::CursorIcon::ResizeVertical,
                    };
                    ctx.set_cursor_icon(cursor);
                    break;
                }
            }
        }

        // Handle divider dragging
        let pointer_pressed = ctx.input(|i| i.pointer.primary_pressed());
        let pointer_released = ctx.input(|i| i.pointer.primary_released());
        let pointer_pos = ctx.input(|i| i.pointer.hover_pos().unwrap_or_default());

        if pointer_pressed && !self.divider_drag_state.is_dragging() {
            if let Some(idx) = hovered_divider {
                let divider = &layout.dividers[idx];
                self.divider_drag_state.dragging_path = Some(divider.tree_path.clone());
                self.divider_drag_state.drag_direction = Some(divider.direction);
                self.divider_drag_state.drag_start_pos = Some(pointer_pos);
                self.divider_drag_state.start_ratio = divider.ratio;
                // Use the divider's parent_rect (local coordinate space), NOT available_rect
                self.divider_drag_state.parent_rect = Some(divider.parent_rect);
            }
        }

        if self.divider_drag_state.is_dragging() {
            if let Some(new_ratio) = self.tiled_layout.calculate_drag_ratio(&self.divider_drag_state, pointer_pos) {
                if let Some(path) = &self.divider_drag_state.dragging_path {
                    self.tiled_layout.set_ratio_at_path(path, new_ratio);
                }
            }

            if pointer_released {
                self.divider_drag_state.clear();
            }
        }

        // Render each cell
        let environment_cell_id = self.tiled_layout.get_environment_cell_id();
        for (cell_id, rect) in &layout.cell_rects {
            if let Some(cell) = self.tiled_layout.find_cell(*cell_id) {
                let panel_ids_in_cell = cell.panel_ids.clone();
                let active_tab = cell.active_tab;
                let has_tabs = panel_ids_in_cell.len() > 1;
                let cell_id_copy = *cell_id;

                egui::Area::new(egui::Id::new(format!("tiled_cell_{}", cell_id)))
                    .fixed_pos(rect.min)
                    .order(egui::Order::Background)
                    .show(ctx, |ui| {
                        ui.set_clip_rect(*rect);
                        let inner_rect = rect.shrink(1.0); // Small margin
                        ui.allocate_ui_at_rect(inner_rect, |ui| {
                            // Draw cell background
                            ui.painter().rect_filled(
                                inner_rect,
                                0.0,
                                egui::Color32::from_gray(30),
                            );

                            // Render tab bar if multiple panels (or single panel header for drag handle)
                            ui.horizontal(|ui| {
                                // Cell drag handle (grip icon) - only for non-environment cells
                                if cell_id_copy != environment_cell_id {
                                    let grip_response = ui.add(
                                        egui::Button::new("\u{2630}")  // ☰ trigram for heaven (hamburger menu icon)
                                            .frame(false)
                                            .sense(egui::Sense::click_and_drag())
                                    );

                                    if grip_response.drag_started() {
                                        self.cell_drag_state.start(cell_id_copy, grip_response.rect.center());
                                    }

                                    // Visual feedback: change cursor on hover
                                    if grip_response.hovered() && !self.cell_drag_state.is_dragging() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                                    }

                                    ui.add_space(2.0);
                                }

                                for (idx, panel_id) in panel_ids_in_cell.iter().enumerate() {
                                    let is_active = idx == active_tab || !has_tabs;
                                    let title = self.get_panel_title(panel_id);

                                    // Use Button with drag sensing (selectable_label doesn't support drag)
                                    let response = ui.add(
                                        egui::Button::new(&title)
                                            .selected(is_active)
                                            .sense(egui::Sense::click_and_drag())
                                    );

                                    // Handle tab click
                                    if response.clicked() && has_tabs {
                                        if let Some(c) = self.tiled_layout.find_cell_mut(cell_id_copy) {
                                            c.active_tab = idx;
                                        }
                                    }

                                    // Handle drag start (except for environment which can't be moved)
                                    if response.drag_started() && panel_id != panel_ids::ENVIRONMENT {
                                        self.panel_drag_state.start(
                                            panel_id.clone(),
                                            cell_id_copy,
                                            response.rect.center(),
                                        );
                                    }

                                    // Right-click context menu for panel operations
                                    let panel_id_clone = panel_id.clone();
                                    response.context_menu(|ui| {
                                        if panel_id_clone != panel_ids::ENVIRONMENT {
                                            // Only show undock option for panels without drag-drop
                                            // (egui's DragAndDrop uses context-local payloads)
                                            if can_panel_undock(&panel_id_clone) {
                                                if ui.button("🗗 Float as Window").clicked() {
                                                    self.tiled_layout.undock_panel(&panel_id_clone);
                                                    ui.close_menu();
                                                }
                                                ui.separator();
                                            }
                                            if ui.button("✕ Close Panel").clicked() {
                                                self.tiled_layout.close_panel(&panel_id_clone);
                                                ui.close_menu();
                                            }
                                        } else {
                                            ui.label("(Environment cannot be closed)");
                                        }
                                    });
                                }
                            });
                            ui.separator();

                            // Render active panel content
                            if let Some(panel_id) = panel_ids_in_cell.get(active_tab) {
                                self.render_panel_content(ui, panel_id);
                            }
                        });
                    });
            }
        }

        // Handle panel drag-and-drop
        if self.panel_drag_state.is_dragging() {
            // Update current position
            self.panel_drag_state.current_pos = hover_pos;

            // Determine drop target based on cursor position
            let mut current_drop_target: Option<DropTarget> = None;
            let edge_zone_size = 40.0; // Pixels from edge for split zones

            if let Some(pos) = hover_pos {
                for (cell_id, rect) in &layout.cell_rects {
                    if rect.contains(pos) {
                        // Check edge zones for splits
                        let left_zone = egui::Rect::from_min_max(
                            rect.min,
                            egui::pos2(rect.min.x + edge_zone_size, rect.max.y),
                        );
                        let right_zone = egui::Rect::from_min_max(
                            egui::pos2(rect.max.x - edge_zone_size, rect.min.y),
                            rect.max,
                        );
                        let top_zone = egui::Rect::from_min_max(
                            rect.min,
                            egui::pos2(rect.max.x, rect.min.y + edge_zone_size),
                        );
                        let bottom_zone = egui::Rect::from_min_max(
                            egui::pos2(rect.min.x, rect.max.y - edge_zone_size),
                            rect.max,
                        );

                        if left_zone.contains(pos) {
                            current_drop_target = Some(DropTarget::Split {
                                cell_id: *cell_id,
                                direction: SplitDirection::Horizontal,
                                new_first: true,
                            });
                        } else if right_zone.contains(pos) {
                            current_drop_target = Some(DropTarget::Split {
                                cell_id: *cell_id,
                                direction: SplitDirection::Horizontal,
                                new_first: false,
                            });
                        } else if top_zone.contains(pos) {
                            current_drop_target = Some(DropTarget::Split {
                                cell_id: *cell_id,
                                direction: SplitDirection::Vertical,
                                new_first: true,
                            });
                        } else if bottom_zone.contains(pos) {
                            current_drop_target = Some(DropTarget::Split {
                                cell_id: *cell_id,
                                direction: SplitDirection::Vertical,
                                new_first: false,
                            });
                        } else {
                            // Center zone - add as tab
                            current_drop_target = Some(DropTarget::Tab { cell_id: *cell_id });
                        }
                        break;
                    }
                }
            }

            // Render drop zone highlights
            egui::Area::new(egui::Id::new("panel_drop_zones"))
                .fixed_pos(available_rect.min)
                .order(egui::Order::Foreground)
                .interactable(false)
                .show(ctx, |ui| {
                    if let Some(pos) = hover_pos {
                        for (cell_id, rect) in &layout.cell_rects {
                            if rect.contains(pos) {
                                // Highlight drop zones
                                let edge_color = egui::Color32::from_rgba_unmultiplied(100, 150, 255, 100);
                                let center_color = egui::Color32::from_rgba_unmultiplied(100, 255, 150, 60);

                                let left_zone = egui::Rect::from_min_max(
                                    rect.min,
                                    egui::pos2(rect.min.x + edge_zone_size, rect.max.y),
                                );
                                let right_zone = egui::Rect::from_min_max(
                                    egui::pos2(rect.max.x - edge_zone_size, rect.min.y),
                                    rect.max,
                                );
                                let top_zone = egui::Rect::from_min_max(
                                    rect.min,
                                    egui::pos2(rect.max.x, rect.min.y + edge_zone_size),
                                );
                                let bottom_zone = egui::Rect::from_min_max(
                                    egui::pos2(rect.min.x, rect.max.y - edge_zone_size),
                                    rect.max,
                                );
                                let center_zone = rect.shrink(edge_zone_size);

                                // Highlight hovered zone
                                if left_zone.contains(pos) {
                                    ui.painter().rect_filled(left_zone, 4.0, edge_color);
                                } else if right_zone.contains(pos) {
                                    ui.painter().rect_filled(right_zone, 4.0, edge_color);
                                } else if top_zone.contains(pos) {
                                    ui.painter().rect_filled(top_zone, 4.0, edge_color);
                                } else if bottom_zone.contains(pos) {
                                    ui.painter().rect_filled(bottom_zone, 4.0, edge_color);
                                } else {
                                    ui.painter().rect_filled(center_zone, 4.0, center_color);
                                }

                                // Draw border around target cell
                                ui.painter().rect_stroke(
                                    *rect,
                                    2.0,
                                    egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 150, 255)),
                                    egui::epaint::StrokeKind::Outside,
                                );
                                break;
                            }
                        }
                    }

                    // Draw dragged panel indicator near cursor
                    if let (Some(panel_id), Some(pos)) = (&self.panel_drag_state.dragged_panel, hover_pos) {
                        let title = self.get_panel_title(panel_id);
                        let label_rect = egui::Rect::from_min_size(
                            egui::pos2(pos.x + 10.0, pos.y + 10.0),
                            egui::vec2(100.0, 24.0),
                        );
                        ui.painter().rect_filled(label_rect, 4.0, egui::Color32::from_rgba_unmultiplied(60, 60, 80, 220));
                        ui.painter().text(
                            label_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            &title,
                            egui::FontId::default(),
                            egui::Color32::WHITE,
                        );
                    }
                });

            // Handle drop on mouse release
            if pointer_released {
                if let Some(panel_id) = self.panel_drag_state.dragged_panel.clone() {
                    if let Some(target) = current_drop_target {
                        self.tiled_layout.handle_drop(&panel_id, target);
                    }
                }
                self.panel_drag_state.clear();
            }
        }

        // Handle cell drag-and-drop (moving entire cells/tiles)
        if self.cell_drag_state.is_dragging() {
            // Update current position
            self.cell_drag_state.current_pos = hover_pos;

            // Determine drop target based on cursor position (same logic as panel drag)
            let mut current_drop_target: Option<DropTarget> = None;
            let edge_zone_size = 40.0;
            let dragged_cell_id = self.cell_drag_state.dragged_cell_id.unwrap_or(u32::MAX);

            if let Some(pos) = hover_pos {
                for (cell_id, rect) in &layout.cell_rects {
                    // Skip the cell being dragged
                    if *cell_id == dragged_cell_id {
                        continue;
                    }

                    if rect.contains(pos) {
                        // Check edge zones for splits
                        let left_zone = egui::Rect::from_min_max(
                            rect.min,
                            egui::pos2(rect.min.x + edge_zone_size, rect.max.y),
                        );
                        let right_zone = egui::Rect::from_min_max(
                            egui::pos2(rect.max.x - edge_zone_size, rect.min.y),
                            rect.max,
                        );
                        let top_zone = egui::Rect::from_min_max(
                            rect.min,
                            egui::pos2(rect.max.x, rect.min.y + edge_zone_size),
                        );
                        let bottom_zone = egui::Rect::from_min_max(
                            egui::pos2(rect.min.x, rect.max.y - edge_zone_size),
                            rect.max,
                        );

                        if left_zone.contains(pos) {
                            current_drop_target = Some(DropTarget::Split {
                                cell_id: *cell_id,
                                direction: SplitDirection::Horizontal,
                                new_first: true,
                            });
                        } else if right_zone.contains(pos) {
                            current_drop_target = Some(DropTarget::Split {
                                cell_id: *cell_id,
                                direction: SplitDirection::Horizontal,
                                new_first: false,
                            });
                        } else if top_zone.contains(pos) {
                            current_drop_target = Some(DropTarget::Split {
                                cell_id: *cell_id,
                                direction: SplitDirection::Vertical,
                                new_first: true,
                            });
                        } else if bottom_zone.contains(pos) {
                            current_drop_target = Some(DropTarget::Split {
                                cell_id: *cell_id,
                                direction: SplitDirection::Vertical,
                                new_first: false,
                            });
                        } else {
                            // Center zone - merge tabs
                            current_drop_target = Some(DropTarget::Tab { cell_id: *cell_id });
                        }
                        break;
                    }
                }
            }

            // Render drop zone highlights for cell drag
            egui::Area::new(egui::Id::new("cell_drop_zones"))
                .fixed_pos(available_rect.min)
                .order(egui::Order::Foreground)
                .interactable(false)
                .show(ctx, |ui| {
                    if let Some(pos) = hover_pos {
                        for (cell_id, rect) in &layout.cell_rects {
                            // Skip the cell being dragged
                            if *cell_id == dragged_cell_id {
                                // Draw a dimmed overlay on the source cell
                                ui.painter().rect_filled(
                                    *rect,
                                    2.0,
                                    egui::Color32::from_rgba_unmultiplied(0, 0, 0, 100),
                                );
                                continue;
                            }

                            if rect.contains(pos) {
                                // Highlight drop zones (orange tint to differentiate from panel drag)
                                let edge_color = egui::Color32::from_rgba_unmultiplied(255, 150, 50, 100);
                                let center_color = egui::Color32::from_rgba_unmultiplied(255, 200, 100, 60);

                                let left_zone = egui::Rect::from_min_max(
                                    rect.min,
                                    egui::pos2(rect.min.x + edge_zone_size, rect.max.y),
                                );
                                let right_zone = egui::Rect::from_min_max(
                                    egui::pos2(rect.max.x - edge_zone_size, rect.min.y),
                                    rect.max,
                                );
                                let top_zone = egui::Rect::from_min_max(
                                    rect.min,
                                    egui::pos2(rect.max.x, rect.min.y + edge_zone_size),
                                );
                                let bottom_zone = egui::Rect::from_min_max(
                                    egui::pos2(rect.min.x, rect.max.y - edge_zone_size),
                                    rect.max,
                                );
                                let center_zone = rect.shrink(edge_zone_size);

                                // Highlight hovered zone
                                if left_zone.contains(pos) {
                                    ui.painter().rect_filled(left_zone, 4.0, edge_color);
                                } else if right_zone.contains(pos) {
                                    ui.painter().rect_filled(right_zone, 4.0, edge_color);
                                } else if top_zone.contains(pos) {
                                    ui.painter().rect_filled(top_zone, 4.0, edge_color);
                                } else if bottom_zone.contains(pos) {
                                    ui.painter().rect_filled(bottom_zone, 4.0, edge_color);
                                } else {
                                    ui.painter().rect_filled(center_zone, 4.0, center_color);
                                }

                                // Draw border around target cell
                                ui.painter().rect_stroke(
                                    *rect,
                                    2.0,
                                    egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 150, 50)),
                                    egui::epaint::StrokeKind::Outside,
                                );
                                break;
                            }
                        }
                    }

                    // Draw dragged cell indicator near cursor
                    if let Some(pos) = hover_pos {
                        // Get the cell's panel titles for the label
                        let label = if let Some(cell) = self.tiled_layout.find_cell(dragged_cell_id) {
                            if cell.panel_ids.len() == 1 {
                                self.get_panel_title(&cell.panel_ids[0])
                            } else {
                                format!("{} tabs", cell.panel_ids.len())
                            }
                        } else {
                            "Cell".to_string()
                        };

                        let label_rect = egui::Rect::from_min_size(
                            egui::pos2(pos.x + 10.0, pos.y + 10.0),
                            egui::vec2(100.0, 24.0),
                        );
                        ui.painter().rect_filled(label_rect, 4.0, egui::Color32::from_rgba_unmultiplied(80, 60, 40, 220));
                        ui.painter().text(
                            label_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            &label,
                            egui::FontId::default(),
                            egui::Color32::WHITE,
                        );
                    }
                });

            // Handle drop on mouse release
            if pointer_released {
                if let Some(cell_id) = self.cell_drag_state.dragged_cell_id {
                    if let Some(target) = current_drop_target {
                        self.tiled_layout.handle_cell_drop(cell_id, target);
                    }
                }
                self.cell_drag_state.clear();
            }
        }

        // Render dividers on top
        egui::Area::new(egui::Id::new("tiled_dividers"))
            .fixed_pos(available_rect.min)
            .order(egui::Order::Middle)
            .interactable(false)
            .show(ctx, |ui| {
                for (idx, divider) in layout.dividers.iter().enumerate() {
                    let is_dragging = self.divider_drag_state.dragging_path
                        .as_ref()
                        .map(|p| p == &divider.tree_path)
                        .unwrap_or(false);
                    let is_hovered = hovered_divider == Some(idx);

                    let color = if is_dragging {
                        egui::Color32::from_rgb(100, 150, 255)
                    } else if is_hovered {
                        egui::Color32::from_rgb(80, 80, 100)
                    } else {
                        egui::Color32::from_rgb(50, 50, 60)
                    };

                    ui.painter().rect_filled(divider.rect, 0.0, color);
                }
            });
    }

    /// Get the display title for a panel ID.
    fn get_panel_title(&self, panel_id: &str) -> String {
        use crate::ui::dock::panel_ids;
        match panel_id {
            panel_ids::ENVIRONMENT => "Environment".to_string(),
            panel_ids::PROPERTIES => "Properties".to_string(),
            panel_ids::CLIP_GRID => "Clip Grid".to_string(),
            panel_ids::SOURCES => "Sources".to_string(),
            panel_ids::EFFECTS_BROWSER => "Effects".to_string(),
            panel_ids::FILES => "Files".to_string(),
            panel_ids::PREVIEW_MONITOR => "Preview Monitor".to_string(),
            panel_ids::PREVIS => "3D Previs".to_string(),
            panel_ids::PERFORMANCE => "Performance".to_string(),
            _ => panel_id.to_string(),
        }
    }

    /// Render the content of a specific panel into the given UI area.
    ///
    /// Note: This is a simplified version for the tiled layout. Full integration
    /// with action handling will be added incrementally.
    fn render_panel_content(&mut self, ui: &mut egui::Ui, panel_id: &str) {
        use crate::ui::dock::panel_ids;

        match panel_id {
            panel_ids::ENVIRONMENT => {
                // Environment viewport - render the composition canvas with pan/zoom support
                if let Some(tex_id) = self.environment_egui_texture_id {
                    let available = ui.available_size();
                    let env_width = self.environment.width() as f32;
                    let env_height = self.environment.height() as f32;
                    let content_size = (env_width, env_height);

                    // Allocate rect with click_and_drag for viewport input
                    let (full_rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());

                    // Handle viewport interactions (right-drag pan, scroll zoom)
                    let viewport_response = crate::ui::viewport_widget::handle_viewport_input(
                        ui,
                        &response,
                        full_rect,
                        &mut self.tiled_env_viewport,
                        content_size,
                        &crate::ui::ViewportConfig::default(),
                        "tiled_env",
                    );

                    if viewport_response.changed {
                        ui.ctx().request_repaint();
                    }

                    // Compute UV and dest rect with viewport transform
                    let render_info = crate::ui::viewport_widget::compute_uv_and_dest_rect(
                        &self.tiled_env_viewport,
                        full_rect,
                        content_size,
                    );

                    // Fill background with black for letterboxing
                    ui.painter().rect_filled(full_rect, 0.0, egui::Color32::BLACK);

                    // Draw environment texture with computed UV rect
                    ui.painter().image(tex_id, render_info.dest_rect, render_info.uv_rect, egui::Color32::WHITE);

                    // Draw zoom indicator (shows percentage in bottom-right)
                    crate::ui::viewport_widget::draw_zoom_indicator(ui, full_rect, &self.tiled_env_viewport);
                } else {
                    // Show placeholder on first frame before texture is registered
                    ui.centered_and_justified(|ui| {
                        ui.label("Loading environment...");
                    });
                }
            }
            panel_ids::PROPERTIES => {
                let layers = self.environment.layers().to_vec();
                let layer_video_info = self.layer_video_info();
                let actions = self.properties_panel.render(
                    ui,
                    &self.environment,
                    &layers,
                    &self.settings,
                    self.omt_broadcast_enabled,
                    self.ndi_sender.is_some(),
                    self.texture_share_enabled,
                    self.api_server_running,
                    self.effect_manager.registry(),
                    self.effect_manager.bpm_clock(),
                    self.effect_manager.time(),
                    Some(&self.audio_manager),
                    &self.effect_manager,
                    &layer_video_info,
                    &mut self.cross_window_drag,
                );
                for action in actions {
                    self.handle_properties_action(action);
                }
            }
            panel_ids::CLIP_GRID => {
                let layers = self.environment.layers().to_vec();
                let actions = self.clip_grid_panel.render_contents(ui, &layers, &mut self.thumbnail_cache);
                for action in actions {
                    self.handle_clip_action(action);
                }
            }
            panel_ids::SOURCES => {
                let actions = self.sources_panel.render_contents(ui);
                for action in actions {
                    self.handle_sources_action(action);
                }
            }
            panel_ids::EFFECTS_BROWSER => {
                // Effects browser drag-drop is handled via cross_window_drag state
                self.effects_browser_panel.render_contents(
                    ui,
                    self.effect_manager.registry(),
                    &mut self.cross_window_drag,
                );
            }
            panel_ids::FILES => {
                // File browser drag-drop is handled separately
                self.file_browser_panel.render_contents(ui);
            }
            panel_ids::PREVIEW_MONITOR => {
                // Preview monitor panel - placeholder
                ui.centered_and_justified(|ui| {
                    ui.label("Preview Monitor\n(Not yet integrated)");
                });
            }
            panel_ids::PREVIS => {
                // 3D Previs panel - placeholder
                ui.centered_and_justified(|ui| {
                    ui.label("3D Previs\n(Not yet integrated)");
                });
            }
            panel_ids::PERFORMANCE => {
                // Performance panel needs PerformanceMetrics, use a basic render
                ui.vertical(|ui| {
                    ui.label(format!("FPS: {:.1}", self.ui_fps));
                    ui.label(format!("Frame time: {:.2} ms", self.ui_frame_time_ms));
                });
            }
            _ => {
                ui.label(format!("Unknown panel: {}", panel_id));
            }
        }
    }

    /// Shutdown the application gracefully
    /// This is called automatically via Drop, but can be called explicitly
    pub fn shutdown(&mut self) {
        // Signal API server to shut down gracefully
        if let Some(tx) = self.api_shutdown_tx.take() {
            log::info!("Signaling API server shutdown...");
            let _ = tx.send(true);
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.shutdown();
    }
}
