//! Main application state and event handling.

use std::sync::Arc;
use std::time::Instant;

/// Helper function to render egui pass, working around lifetime issues in egui-wgpu.
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

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::camera::{NdiFinder, NdiReceiver};
use crate::calibration::{CalibrationConfig, CalibrationSession, CalibrationState};
use crate::render::{PatternRenderer, PreviewRenderer, RenderPipeline};
use crate::ui::UiState;

/// Main application state.
pub struct CalibrationApp {
    /// Main window
    window: Option<Arc<Window>>,
    /// GPU render pipeline
    render: Option<RenderPipeline>,
    /// Pattern renderer for calibration
    pattern_renderer: Option<PatternRenderer>,
    /// Camera preview renderer
    preview_renderer: Option<PreviewRenderer>,
    /// egui context
    egui_ctx: egui::Context,
    /// egui-winit state
    egui_state: Option<egui_winit::State>,
    /// egui-wgpu renderer
    egui_renderer: Option<egui_wgpu::Renderer>,
    /// NDI source finder
    ndi_finder: Option<NdiFinder>,
    /// NDI camera receiver
    ndi_receiver: Option<NdiReceiver>,
    /// Available NDI sources
    ndi_sources: Vec<String>,
    /// Calibration session
    calibration_session: CalibrationSession,
    /// Calibration state (for display)
    calibration_state: CalibrationState,
    /// UI state
    ui_state: UiState,
    /// Last frame time for FPS calculation
    last_frame_time: Instant,
}

impl CalibrationApp {
    pub fn new() -> Self {
        Self {
            window: None,
            render: None,
            pattern_renderer: None,
            preview_renderer: None,
            egui_ctx: egui::Context::default(),
            egui_state: None,
            egui_renderer: None,
            ndi_finder: None,
            ndi_receiver: None,
            ndi_sources: Vec::new(),
            calibration_session: CalibrationSession::new(CalibrationConfig::default()),
            calibration_state: CalibrationState::Idle,
            ui_state: UiState::default(),
            last_frame_time: Instant::now(),
        }
    }

    fn initialize_graphics(&mut self, window: Arc<Window>) {
        let size = window.inner_size();

        // Create wgpu instance
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // Create surface
        let surface = instance.create_surface(window.clone()).expect("Failed to create surface");

        // Request adapter
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to get adapter");

        log::info!("Using adapter: {:?}", adapter.get_info().name);

        // Create device and queue
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Main Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
            },
            None,
        ))
        .expect("Failed to create device");

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Create render pipeline
        let render = RenderPipeline::new(device, queue, surface, config);

        // Initialize egui
        let egui_state = egui_winit::State::new(
            self.egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        let egui_renderer = egui_wgpu::Renderer::new(
            render.device(),
            surface_format,
            None,
            1,
            false,
        );

        // Create pattern and preview renderers
        let pattern_renderer = PatternRenderer::new(
            render.device(),
            surface_format,
            1920, // Default projector width
            1080, // Default projector height
        );
        let preview_renderer = PreviewRenderer::new(render.device(), surface_format);

        self.window = Some(window);
        self.pattern_renderer = Some(pattern_renderer);
        self.preview_renderer = Some(preview_renderer);
        self.render = Some(render);
        self.egui_state = Some(egui_state);
        self.egui_renderer = Some(egui_renderer);

        // Start NDI discovery
        self.ndi_finder = NdiFinder::new();
        if self.ndi_finder.is_some() {
            log::info!("NDI discovery started");
        }
    }

    fn handle_resize(&mut self, size: PhysicalSize<u32>) {
        if let Some(render) = &mut self.render {
            render.resize(size.width.max(1), size.height.max(1));
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.state == ElementState::Pressed {
            match key.physical_key {
                PhysicalKey::Code(KeyCode::Escape) => {
                    if let Some(window) = &self.window {
                        window.set_visible(false);
                    }
                }
                PhysicalKey::Code(KeyCode::Space) => {
                    // Toggle calibration
                    match &self.calibration_state {
                        CalibrationState::Idle => {
                            if self.ndi_receiver.is_some() {
                                log::info!("Starting calibration...");
                                // Would start calibration here
                            }
                        }
                        _ => {
                            log::info!("Stopping calibration");
                            self.calibration_state = CalibrationState::Idle;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn update_ndi_sources(&mut self) {
        if let Some(finder) = &self.ndi_finder {
            self.ndi_sources = finder.get_sources();
        }
    }

    fn render_frame(&mut self) {
        // Update NDI sources periodically
        self.update_ndi_sources();

        // Update calibration session state machine
        self.calibration_session.update();

        // Process calibration if in decoding/homography state
        if matches!(
            self.calibration_session.state,
            CalibrationState::Decoding { .. } | CalibrationState::ComputingHomography { .. }
        ) {
            self.calibration_session.process_calibration();
        }

        // Get camera frame if available
        let camera_frame = if let Some(receiver) = &mut self.ndi_receiver {
            receiver.take_frame()
        } else {
            None
        };

        // Submit camera frame to calibration session
        if let Some(ref frame) = camera_frame {
            // Convert BGRA to grayscale for calibration (use green channel as it's most reliable)
            let grayscale: Vec<u8> = frame.data
                .chunks(4)
                .map(|bgra| bgra[1]) // Green channel
                .collect();
            self.calibration_session.submit_frame(grayscale, frame.width, frame.height);
        }

        // Upload camera frame for preview
        if let (Some(ref frame), Some(render), Some(preview)) = (
            &camera_frame,
            &self.render,
            &mut self.preview_renderer,
        ) {
            preview.upload_frame(render.device(), render.queue(), &frame.data, frame.width, frame.height);
        }

        // Get window reference for egui input
        let Some(window) = &self.window else { return };
        let Some(egui_state) = &mut self.egui_state else { return };

        // Begin egui frame
        let raw_input = egui_state.take_egui_input(window);
        self.egui_ctx.begin_pass(raw_input);

        // Draw UI
        self.draw_ui(&camera_frame);

        // End egui frame
        let full_output = self.egui_ctx.end_pass();

        let Some(window) = &self.window else { return };
        let Some(egui_state) = &mut self.egui_state else { return };
        egui_state.handle_platform_output(window, full_output.platform_output);

        // Tessellate shapes
        let pixels_per_point = self.egui_ctx.pixels_per_point();
        let clipped_primitives = self.egui_ctx.tessellate(full_output.shapes, pixels_per_point);

        // Now do the rendering
        let Some(render) = &self.render else { return };
        let Some(egui_renderer) = &mut self.egui_renderer else { return };

        // Render
        let output = match render.surface().get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                let Some(window) = &self.window else { return };
                let size = window.inner_size();
                if let Some(render) = &mut self.render {
                    render.resize(size.width, size.height);
                }
                return;
            }
            Err(e) => {
                log::error!("Surface error: {:?}", e);
                return;
            }
        };

        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = render.device().create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // Clear pass
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        // egui pass
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [render.config().width, render.config().height],
            pixels_per_point,
        };

        for (id, delta) in &full_output.textures_delta.set {
            egui_renderer.update_texture(render.device(), render.queue(), *id, delta);
        }

        egui_renderer.update_buffers(
            render.device(),
            render.queue(),
            &mut encoder,
            &clipped_primitives,
            &screen_descriptor,
        );

        render_egui_pass(
            egui_renderer,
            &mut encoder,
            &view,
            &clipped_primitives,
            &screen_descriptor,
        );

        for id in &full_output.textures_delta.free {
            egui_renderer.free_texture(id);
        }

        render.queue().submit(std::iter::once(encoder.finish()));
        output.present();
    }

    fn draw_ui(&mut self, _camera_frame: &Option<crate::camera::NdiFrame>) {
        egui::TopBottomPanel::top("menu_bar").show(&self.egui_ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Project").clicked() {
                        ui.close_menu();
                    }
                    if ui.button("Open Project...").clicked() {
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Export Calibration...").clicked() {
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        std::process::exit(0);
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        ui.close_menu();
                    }
                });
            });
        });

        egui::SidePanel::left("settings_panel")
            .min_width(300.0)
            .show(&self.egui_ctx, |ui| {
                ui.heading("Camera");
                ui.separator();

                // NDI source selection
                ui.horizontal(|ui| {
                    ui.label("NDI Source:");
                    egui::ComboBox::from_id_salt("ndi_source")
                        .selected_text(
                            self.ndi_receiver
                                .as_ref()
                                .map(|r| r.source_name())
                                .unwrap_or("Select..."),
                        )
                        .show_ui(ui, |ui| {
                            for source in &self.ndi_sources {
                                if ui.selectable_label(false, source).clicked() {
                                    match NdiReceiver::connect(source) {
                                        Ok(receiver) => {
                                            self.ndi_receiver = Some(receiver);
                                            log::info!("Connected to NDI source: {}", source);
                                        }
                                        Err(e) => {
                                            log::error!("Failed to connect to NDI source: {}", e);
                                        }
                                    }
                                }
                            }
                        });
                });

                if let Some(receiver) = &self.ndi_receiver {
                    ui.label(format!(
                        "Status: {} ({}x{} @ {:.1} fps)",
                        if receiver.is_connected() { "Connected" } else { "Connecting..." },
                        receiver.width(),
                        receiver.height(),
                        receiver.average_fps()
                    ));
                }

                ui.add_space(20.0);
                ui.heading("Projectors");
                ui.separator();

                ui.horizontal(|ui| {
                    if ui.button("Add Projector").clicked() {
                        self.ui_state.projector_count += 1;
                    }
                });

                for i in 0..self.ui_state.projector_count {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(format!("Projector {}", i + 1));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("x").clicked() {
                                    // Remove projector
                                }
                            });
                        });
                        ui.horizontal(|ui| {
                            ui.label("Resolution:");
                            ui.add(egui::DragValue::new(&mut self.ui_state.projector_width).speed(10));
                            ui.label("x");
                            ui.add(egui::DragValue::new(&mut self.ui_state.projector_height).speed(10));
                        });
                    });
                }

                ui.add_space(20.0);
                ui.heading("Calibration");
                ui.separator();

                // Sync calibration state from session
                self.calibration_state = self.calibration_session.state.clone();
                ui.label(format!("State: {}", self.calibration_state));

                // Progress bar
                let progress = self.calibration_session.progress();
                if progress > 0.0 && progress < 1.0 {
                    ui.add(egui::ProgressBar::new(progress).show_percentage());
                }

                ui.horizontal(|ui| {
                    let can_start = self.ndi_receiver.is_some()
                        && self.calibration_session.state.is_idle()
                        && self.ui_state.projector_count > 0;

                    if ui.add_enabled(can_start, egui::Button::new("Start Calibration")).clicked() {
                        log::info!("Starting calibration");
                        // Add projectors to session
                        for i in 0..self.ui_state.projector_count {
                            self.calibration_session.add_projector(
                                i as u32,
                                self.ui_state.projector_width,
                                self.ui_state.projector_height,
                            );
                        }
                        if let Err(e) = self.calibration_session.start() {
                            log::error!("Failed to start calibration: {}", e);
                        }
                    }

                    let can_stop = !self.calibration_session.state.is_idle();
                    if ui.add_enabled(can_stop, egui::Button::new("Stop")).clicked() {
                        self.calibration_session.cancel();
                    }
                });

                ui.add_space(20.0);
                ui.heading("Edge Blending");
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Blend Width:");
                    ui.add(egui::Slider::new(&mut self.ui_state.blend_width, 0..=500).suffix(" px"));
                });

                ui.horizontal(|ui| {
                    ui.label("Blend Curve:");
                    egui::ComboBox::from_id_salt("blend_curve")
                        .selected_text(&self.ui_state.blend_curve)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.ui_state.blend_curve, "Linear".to_string(), "Linear");
                            ui.selectable_value(&mut self.ui_state.blend_curve, "Gamma".to_string(), "Gamma");
                            ui.selectable_value(&mut self.ui_state.blend_curve, "Cosine".to_string(), "Cosine");
                            ui.selectable_value(&mut self.ui_state.blend_curve, "Smoothstep".to_string(), "Smoothstep");
                        });
                });

                ui.add_space(10.0);

                // Overlap detection button (enabled after calibration complete)
                let can_detect = self.calibration_session.state.is_complete()
                    && self.calibration_session.projectors.len() > 1;

                if ui.add_enabled(can_detect, egui::Button::new("Detect Overlaps")).clicked() {
                    use crate::blending::{OverlapConfig, OverlapDetector};
                    use crate::config::BlendCurve;

                    let curve = match self.ui_state.blend_curve.as_str() {
                        "Linear" => BlendCurve::Linear,
                        "Gamma" => BlendCurve::Gamma,
                        "Cosine" => BlendCurve::Cosine,
                        _ => BlendCurve::Smoothstep,
                    };

                    let config = OverlapConfig {
                        min_overlap_width: 10,
                        blend_curve: curve,
                        padding: 0,
                    };

                    let detector = OverlapDetector::new(config);
                    let result = detector.detect(&self.calibration_session.projectors);

                    log::info!("Detected {} overlap regions", result.overlaps.len());
                    for overlap in &result.overlaps {
                        log::info!(
                            "  Projector {} <-> {}: {:?} edge, {} pixels",
                            overlap.projector_a,
                            overlap.projector_b,
                            overlap.edge,
                            overlap.overlap_width
                        );
                    }

                    self.ui_state.overlap_result = Some(result);
                }

                // Show detected overlaps
                if let Some(ref result) = self.ui_state.overlap_result {
                    ui.label(format!("{} overlaps detected", result.overlaps.len()));

                    // Export button
                    if ui.button("Export Blend Masks...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Export Blend Masks")
                            .pick_folder()
                        {
                            use crate::export::CalibrationExporter;
                            if let Err(e) = CalibrationExporter::export_all_blend_masks(
                                &result.blend_masks,
                                &path,
                                true, // 16-bit
                            ) {
                                log::error!("Failed to export blend masks: {}", e);
                            } else {
                                log::info!("Blend masks exported to {:?}", path);
                            }
                        }
                    }
                }
            });

        egui::CentralPanel::default().show(&self.egui_ctx, |ui| {
            ui.heading("Camera Preview");
            // Camera preview would go here
            ui.label("Connect to an NDI camera to see preview");
        });

        egui::TopBottomPanel::bottom("status_bar").show(&self.egui_ctx, |ui| {
            ui.horizontal(|ui| {
                let elapsed = self.last_frame_time.elapsed();
                let fps = 1.0 / elapsed.as_secs_f64();
                self.last_frame_time = Instant::now();
                ui.label(format!("FPS: {:.1}", fps));

                ui.separator();

                ui.label(format!("NDI Sources: {}", self.ndi_sources.len()));
            });
        });
    }
}

impl Default for CalibrationApp {
    fn default() -> Self {
        Self::new()
    }
}

impl ApplicationHandler for CalibrationApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_attrs = Window::default_attributes()
                .with_title("ProjMap Calibrator")
                .with_inner_size(PhysicalSize::new(1400, 900));

            let window = Arc::new(
                event_loop
                    .create_window(window_attrs)
                    .expect("Failed to create window"),
            );

            self.initialize_graphics(window);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // Let egui handle the event first
        if let Some(egui_state) = &mut self.egui_state {
            if let Some(window) = &self.window {
                let response = egui_state.on_window_event(window, &event);
                if response.consumed {
                    return;
                }
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                self.handle_resize(size);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_key(event);
            }
            WindowEvent::RedrawRequested => {
                self.render_frame();

                // Request next frame
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
