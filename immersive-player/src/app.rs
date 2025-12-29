//! Main application state and UI
//!
//! Provides the main application window and state management.
//! The main window is simplified to focus on composition/clips.
//! Screen/output configuration is handled in the Advanced Output window.

#![allow(dead_code)]

use crate::composition::Composition;
use crate::converter::ConverterWindow;
use crate::output::OutputManager;
use crate::project::ProjectPreset;
use crate::render::Compositor;
use crate::ui::{AdvancedOutputWindow, ClipMatrix, LayerControls, PreviewMonitor};
use crate::video::VideoPlayer;
use eframe::egui::{self, Color32};
use std::collections::HashMap;
use std::time::Instant;

/// Main application state
pub struct ImmersivePlayerApp {
    // Core components
    pub composition: Composition,
    pub video_player: VideoPlayer,
    pub output_manager: OutputManager,
    pub compositor: Compositor,
    pub project: ProjectPreset,

    // UI panels
    pub clip_matrix: ClipMatrix,
    pub layer_controls: LayerControls,
    pub preview_monitor: PreviewMonitor,
    pub converter_window: ConverterWindow,
    pub advanced_output_window: AdvancedOutputWindow,

    // Settings
    pub show_composition_settings: bool,
    pub show_layer_controls: bool,

    // Timing
    last_update: Instant,
    animation_time: f32,
}

impl Default for ImmersivePlayerApp {
    fn default() -> Self {
        Self {
            composition: Composition::default(),
            video_player: VideoPlayer::new(),
            output_manager: OutputManager::new(),
            compositor: Compositor::new(),
            project: ProjectPreset::default(),

            clip_matrix: ClipMatrix::new(),
            layer_controls: LayerControls::new(),
            preview_monitor: PreviewMonitor::new(),
            converter_window: ConverterWindow::new(),
            advanced_output_window: AdvancedOutputWindow::new(),

            show_composition_settings: false,
            show_layer_controls: true,

            last_update: Instant::now(),
            animation_time: 0.0,
        }
    }
}

impl ImmersivePlayerApp {
    /// Create a new application instance
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        log::info!("Initializing Immersive Player...");

        let mut app = Self::default();

        // Initialize compositor with WGPU handles from eframe
        if let Some(render_state) = _cc.wgpu_render_state.clone() {
            app.compositor
                .initialize(render_state.device.clone(), render_state.queue.clone());
            log::info!("Compositor initialized with WGPU");
        } else {
            log::warn!("WGPU render state not available - compositor will not be active");
        }

        // Load a test pattern for preview
        app.video_player.load_test_pattern(1920, 1080);

        // Enumerate available displays
        app.output_manager.enumerate_displays();

        log::info!("Immersive Player initialized");
        app
    }

    /// Load a video file
    pub fn load_video(&mut self, path: &std::path::Path) {
        if let Err(e) = self.video_player.load(path) {
            log::error!("Failed to load video: {}", e);
        }
    }

    /// Get composition width
    pub fn composition_width(&self) -> u32 {
        self.composition.settings.width
    }

    /// Get composition height
    pub fn composition_height(&self) -> u32 {
        self.composition.settings.height
    }

    /// Get composition FPS
    pub fn composition_fps(&self) -> f32 {
        self.composition.settings.fps
    }

    /// Show output viewports for all active windows
    fn show_output_viewports(&mut self, ctx: &egui::Context) {
        use crate::output::render_output_content;
        use std::sync::atomic::{AtomicBool, Ordering};
        
        // Shared flag to signal escape was pressed in any viewport
        static ESCAPE_PRESSED: AtomicBool = AtomicBool::new(false);
        
        // Check if escape was pressed in a previous frame's viewport
        if ESCAPE_PRESSED.swap(false, Ordering::SeqCst) {
            if self.output_manager.is_live {
                log::info!("Escape pressed in output viewport - stopping all outputs");
                self.output_manager.stop_outputs();
                return; // Don't show viewports this frame
            }
        }

        // Collect active window info first to avoid borrow conflicts
        let active_windows: Vec<_> = self
            .output_manager
            .window_manager
            .active_windows()
            .map(|(id, w)| {
                (
                    *id,
                    w.viewport_id,
                    w.title.clone(),
                    w.width,
                    w.height,
                    w.fullscreen,
                    w.position,
                    w.show_test_pattern,
                )
            })
            .collect();

        let animation_time = self.animation_time;
        let show_test_pattern = self.output_manager.show_test_pattern;

        for (
            _screen_id,
            viewport_id,
            title,
            width,
            height,
            fullscreen,
            position,
            window_test_pattern,
        ) in active_windows
        {
            // Build viewport properties
            // Create an independent output window
            let mut viewport_builder = egui::ViewportBuilder::default()
                .with_title(&title)
                .with_inner_size([width as f32, height as f32])
                .with_decorations(true); // Keep decorations for proper macOS window management

            // Position on target display
            if let Some((x, y)) = position {
                viewport_builder = viewport_builder.with_position([x as f32, y as f32]);
            }

            let should_fullscreen = fullscreen;

            // Show the viewport
            ctx.show_viewport_immediate(viewport_id, viewport_builder, |ctx, _class| {
                // Check for Escape key in this viewport's context
                if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    ESCAPE_PRESSED.store(true, Ordering::SeqCst);
                }
                
                // Request fullscreen via viewport command (after window is created)
                // This should make THIS window go fullscreen independently
                if should_fullscreen {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
                }
                
                egui::CentralPanel::default()
                    .frame(egui::Frame::none().fill(egui::Color32::BLACK))
                    .show(ctx, |ui| {
                        // Create a temporary WindowOutput for rendering
                        let window = crate::output::WindowOutput {
                            title: title.clone(),
                            width,
                            height,
                            target_display: None,
                            fullscreen,
                            position,
                            active: true,
                            viewport_id,
                            show_test_pattern: show_test_pattern || window_test_pattern,
                        };
                        render_output_content(ui, &window, animation_time);
                    });
            });
        }

        // Request repaint if we have active outputs
        if self.output_manager.is_live {
            ctx.request_repaint();
        }
    }
}

impl eframe::App for ImmersivePlayerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Calculate delta time
        let now = Instant::now();
        let delta_time = now.duration_since(self.last_update).as_secs_f64();
        self.last_update = now;

        // Get composition texture for preview
        let mut composition_texture_id = None;
        if let Some(render_state) = _frame.wgpu_render_state() {
            if let Some(view) = self.compositor.composition_view() {
                composition_texture_id = Some(render_state.renderer.write().register_native_texture(
                    &render_state.device,
                    view,
                    wgpu::FilterMode::Linear,
                ));
            }
        }

        // Update composition playback
        self.composition.update(delta_time);

        // Update video player (for legacy support)
        self.video_player.update();

        // Update animation time
        self.animation_time += ctx.input(|i| i.predicted_dt);

        // Update preview monitor aspect ratio
        self.preview_monitor
            .set_aspect_ratio(self.composition_width(), self.composition_height());

        // Sync compositor size
        self.compositor
            .resize(self.composition_width(), self.composition_height());

        // Collect layer textures from active clips
        let mut layer_textures: HashMap<usize, wgpu::TextureView> = HashMap::new();
        for (layer_idx, layer) in self.composition.layers.iter().enumerate() {
            if layer.is_playing() {
                if let Some(clip) = layer.active_clip() {
                    // Try to get a frame from the clip's video player
                    if let Some(frame) = clip.current_frame() {
                        // Upload the frame to a GPU texture
                        if let Some(texture) = self.compositor.upload_frame(frame) {
                            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                            layer_textures.insert(layer_idx, view);
                        }
                    }
                }
            }
        }

        // Render the composition with layer textures
        self.compositor.render_composition(&self.composition, &layer_textures);

        // Show converter window if open
        self.converter_window.show(ctx);

        // Sync composition size to advanced output window before showing
        self.advanced_output_window
            .set_composition_size(self.composition_width(), self.composition_height());

        // Track if window was open before
        let was_open = self.advanced_output_window.is_open;

        // Show advanced output window if open
        self.advanced_output_window
            .show(ctx, &mut self.output_manager, &self.video_player);

        // Handle window close - check if user clicked Save & Close or Cancel
        if was_open && !self.advanced_output_window.is_open {
            if self.advanced_output_window.should_save {
                // User clicked "Save & Close" - save all settings
                let (w, h) = self.advanced_output_window.get_composition_size();
                self.composition.settings.width = w;
                self.composition.settings.height = h;
                log::info!("Saved composition settings: {}×{}", w, h);
                
                // Auto-start outputs if there are any fullscreen screens configured
                let has_fullscreen = self.output_manager.screens.iter().any(|s| {
                    s.enabled && matches!(s.device, crate::output::OutputDevice::Fullscreen { .. })
                });
                
                if has_fullscreen && !self.output_manager.is_live {
                    log::info!("Auto-starting outputs (fullscreen screens detected)");
                    self.output_manager.go_live();
                }
                
                // Reset flag for next time
                self.advanced_output_window.should_save = false;
            } else {
                // User clicked "Cancel" - discard changes
                // Re-sync the output editor with the current (unchanged) composition settings
                self.advanced_output_window
                    .set_composition_size(self.composition_width(), self.composition_height());
                log::info!("Discarded output changes, reverted to: {}×{}", 
                    self.composition_width(), self.composition_height());
            }
        }
        
        // Handle Escape key to stop all outputs and return to main window
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.output_manager.is_live {
                log::info!("Escape pressed - stopping all outputs");
                self.output_manager.stop_outputs();
            }
        }

        // Show output viewports for active windows
        self.show_output_viewports(ctx);

        // Composition settings window
        if self.show_composition_settings {
            egui::Window::new("⚙ Composition Settings")
                .resizable(false)
                .collapsible(false)
                .open(&mut self.show_composition_settings)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Resolution:");
                        ui.add(
                            egui::DragValue::new(&mut self.composition.settings.width)
                                .speed(1.0)
                                .suffix("px"),
                        );
                        ui.label("×");
                        ui.add(
                            egui::DragValue::new(&mut self.composition.settings.height)
                                .speed(1.0)
                                .suffix("px"),
                        );
                    });

                    ui.horizontal(|ui| {
                        ui.label("Presets:");
                        if ui.button("720p").clicked() {
                            self.composition.settings.width = 1280;
                            self.composition.settings.height = 720;
                        }
                        if ui.button("1080p").clicked() {
                            self.composition.settings.width = 1920;
                            self.composition.settings.height = 1080;
                        }
                        if ui.button("4K").clicked() {
                            self.composition.settings.width = 3840;
                            self.composition.settings.height = 2160;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Frame Rate:");
                        ui.add(
                            egui::DragValue::new(&mut self.composition.settings.fps)
                                .speed(1.0)
                                .suffix(" fps"),
                        );
                    });

                    ui.horizontal(|ui| {
                        ui.label("Layers:");
                        ui.label(format!("{}", self.composition.layers.len()));
                        if ui.button("+").clicked() {
                            self.composition.add_layer();
                        }
                        if ui.button("-").clicked() && self.composition.layers.len() > 1 {
                            let last_id = self.composition.layers.last().map(|l| l.id);
                            if let Some(id) = last_id {
                                self.composition.remove_layer(id);
                            }
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Columns:");
                        ui.label(format!("{}", self.composition.columns));
                        if ui.button("+").clicked() {
                            self.composition.add_column();
                        }
                        if ui.button("-").clicked() {
                            self.composition.remove_column();
                        }
                    });
                });
        }

        // Menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Composition").clicked() {
                        self.composition = Composition::default();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Open Composition...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Immersive Composition", &["immersive"])
                            .add_filter("All Files", &["*"])
                            .pick_file()
                        {
                            match Composition::load_from_file(&path) {
                                Ok(comp) => {
                                    self.composition = comp;
                                    log::info!("Loaded composition from {:?}", path);
                                }
                                Err(e) => {
                                    log::error!("Failed to load composition: {}", e);
                                }
                            }
                        }
                        ui.close_menu();
                    }
                    if ui.button("Save Composition...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Immersive Composition", &["immersive"])
                            .set_file_name("composition.immersive")
                            .save_file()
                        {
                            if let Err(e) = self.composition.save_to_file(&path) {
                                log::error!("Failed to save composition: {}", e);
                            } else {
                                log::info!("Saved composition to {:?}", path);
                            }
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Open Video...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Video Files", &["mov", "mp4", "avi", "mkv"])
                            .pick_file()
                        {
                            self.load_video(&path);
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        std::process::exit(0);
                    }
                });

                ui.menu_button("Composition", |ui| {
                    if ui.button("⚙ Settings...").clicked() {
                        self.show_composition_settings = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Add Layer").clicked() {
                        self.composition.add_layer();
                        ui.close_menu();
                    }
                    if ui.button("Add Column").clicked() {
                        self.composition.add_column();
                        ui.close_menu();
                    }
                });

                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.show_layer_controls, "Layer Controls");
                    ui.checkbox(&mut self.clip_matrix.show_names, "Clip Names");
                    ui.checkbox(&mut self.clip_matrix.show_progress, "Progress Bars");
                });

                ui.menu_button("Tools", |ui| {
                    if ui.button("🎬 HAP Converter").clicked() {
                        self.converter_window.toggle();
                        ui.close_menu();
                    }
                    if ui.button("🖥 Advanced Output").clicked() {
                        self.advanced_output_window.toggle();
                        ui.close_menu();
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        ui.close_menu();
                    }
                });
            });
        });

        // Transport controls panel
        egui::TopBottomPanel::top("transport_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Master controls
                ui.label("Master:");
                ui.add(
                    egui::Slider::new(&mut self.composition.master_opacity, 0.0..=1.0)
                        .show_value(false),
                );
                ui.label(format!("{:.0}%", self.composition.master_opacity * 100.0));

                ui.separator();

                // Composition info
                ui.label(format!(
                    "{}×{} @ {}fps",
                    self.composition_width(),
                    self.composition_height(),
                    self.composition_fps()
                ));

                ui.separator();

                // Layer count
                ui.label(format!(
                    "Layers: {} | Columns: {}",
                    self.composition.layers.len(),
                    self.composition.columns
                ));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Active clips count
                    let active_count = self
                        .composition
                        .layers
                        .iter()
                        .filter(|l| l.is_playing())
                        .count();
                    ui.label(format!("Active: {}", active_count));
                });
            });
        });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Screens: {}", self.output_manager.screens.len()));
                ui.separator();
                ui.label(format!(
                    "Composition: {}×{} @ {}fps",
                    self.composition_width(),
                    self.composition_height(),
                    self.composition_fps()
                ));
                ui.separator();

                // Show playing layers
                let playing: Vec<_> = self
                    .composition
                    .layers
                    .iter()
                    .filter(|l| l.is_playing())
                    .map(|l| l.name.clone())
                    .collect();
                if !playing.is_empty() {
                    ui.label(format!("Playing: {}", playing.join(", ")));
                }
            });
        });

        // Right side panel with Layer Controls and Preview Monitor
        egui::SidePanel::right("right_panel")
            .resizable(true)
            .default_width(300.0)
            .min_width(250.0)
            .show(ctx, |ui| {
                // Preview Monitor
                ui.collapsing("Preview", |ui| {
                    self.preview_monitor.show(
                        ui,
                        &self.video_player,
                        self.output_manager.screens.len(),
                        composition_texture_id,
                    );
                });

                ui.separator();

                // Layer Controls
                if self.show_layer_controls {
                    // Sync selection from clip matrix
                    self.layer_controls.selected_layer = self.clip_matrix.selected_layer;
                    self.layer_controls.show(ui, &mut self.composition);
                }
            });

        // Central panel: Clip Matrix
        egui::CentralPanel::default().show(ctx, |ui| {
            // Dark background
            let available = ui.available_size();
            let (rect, _) = ui.allocate_exact_size(available, egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, 0.0, Color32::from_rgb(25, 28, 32));

            // Reset allocation for actual content
            ui.allocate_ui_at_rect(rect, |ui| {
                ui.add_space(10.0);

                // Scrollable clip matrix
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let response = self.clip_matrix.show(ui, &mut self.composition);

                        // Handle clip triggers
                        if let Some((layer_idx, col)) = response.clip_triggered {
                            self.composition.trigger_clip(layer_idx, col);
                            log::info!("Triggered clip at layer {}, column {}", layer_idx, col);
                        }

                        // Handle context menu
                        if let Some((layer_idx, col)) = response.clip_context_menu {
                            // For now, add a test clip if slot is empty
                            if let Some(layer) = self.composition.get_layer_by_index(layer_idx) {
                                if layer.get_clip(col).is_none() {
                                    ClipMatrix::add_test_clip(
                                        &mut self.composition,
                                        layer_idx,
                                        col,
                                    );
                                    log::info!(
                                        "Added test clip at layer {}, column {}",
                                        layer_idx,
                                        col
                                    );
                                }
                            }
                        }
                    });
            });
        });

        // Request continuous repaint when playing
        let has_playing = self.composition.layers.iter().any(|l| l.is_playing());
        if has_playing || self.video_player.is_playing() {
            ctx.request_repaint();
        }
    }
}
