//! Preferences Window
//!
//! A floating window for editing application-wide environment settings.
//! Accessible via Immersive Server → Preferences (macOS) or Edit → Preferences (Windows).

use crate::audio::AudioManager;
use crate::compositor::Environment;
use crate::network::discovery::{DiscoveredSource, SourceType};
use crate::settings::{AudioSourceType, EnvironmentSettings, ThumbnailMode};
use crate::ui::properties_panel::PropertiesAction;

/// Preferences window for editing environment settings
pub struct PreferencesWindow {
    /// Whether the window is open
    pub open: bool,
    /// Temporary values for resolution editing
    env_width_text: String,
    env_height_text: String,
    /// Whether resolution confirmation dialog is open
    show_resolution_confirm: bool,
    /// Pending resolution to apply after confirmation
    pending_resolution: Option<(u32, u32)>,
    /// Temporary FPS value for slider editing
    temp_fps: u32,
    /// Cached list of system audio devices (to avoid slow enumeration every frame)
    cached_audio_devices: Vec<String>,
    /// Whether audio devices need to be refreshed
    audio_devices_dirty: bool,
}

impl Default for PreferencesWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl PreferencesWindow {
    /// Create a new preferences window (closed by default)
    pub fn new() -> Self {
        Self {
            open: false,
            env_width_text: String::new(),
            env_height_text: String::new(),
            show_resolution_confirm: false,
            pending_resolution: None,
            temp_fps: 60,
            cached_audio_devices: Vec::new(),
            audio_devices_dirty: true,
        }
    }

    /// Toggle the window open/closed
    pub fn toggle(&mut self) {
        self.open = !self.open;
        // Refresh audio devices when opening
        if self.open {
            self.audio_devices_dirty = true;
        }
    }

    /// Render the preferences window
    ///
    /// Returns a list of actions to be processed by the app.
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        environment: &Environment,
        settings: &EnvironmentSettings,
        omt_broadcasting: bool,
        ndi_broadcasting: bool,
        texture_sharing_active: bool,
        api_server_running: bool,
        omt_discovery_active: bool,
        ndi_discovery_active: bool,
        audio_manager: Option<&AudioManager>,
        discovered_sources: &[DiscoveredSource],
    ) -> Vec<PropertiesAction> {
        let mut actions = Vec::new();

        if !self.open {
            return actions;
        }

        let mut open = self.open;
        egui::Window::new("Preferences")
            .id(egui::Id::new("preferences_window"))
            .open(&mut open)
            .default_size([400.0, 550.0])
            .resizable(true)
            .collapsible(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.render_contents(
                            ui,
                            environment,
                            settings,
                            omt_broadcasting,
                            ndi_broadcasting,
                            texture_sharing_active,
                            api_server_running,
                            omt_discovery_active,
                            ndi_discovery_active,
                            audio_manager,
                            discovered_sources,
                            &mut actions,
                        );
                    });
            });
        self.open = open;

        // Handle resolution confirmation dialog (rendered outside the main window)
        self.render_resolution_confirm_dialog(ctx, environment, &mut actions);

        actions
    }

    /// Render the window contents
    fn render_contents(
        &mut self,
        ui: &mut egui::Ui,
        environment: &Environment,
        settings: &EnvironmentSettings,
        omt_broadcasting: bool,
        ndi_broadcasting: bool,
        texture_sharing_active: bool,
        api_server_running: bool,
        omt_discovery_active: bool,
        ndi_discovery_active: bool,
        audio_manager: Option<&AudioManager>,
        discovered_sources: &[DiscoveredSource],
        actions: &mut Vec<PropertiesAction>,
    ) {
        // ========== RESOLUTION ==========
        ui.heading("Resolution");
        ui.add_space(4.0);

        // Initialize text fields if empty
        if self.env_width_text.is_empty() {
            self.env_width_text = environment.width().to_string();
        }
        if self.env_height_text.is_empty() {
            self.env_height_text = environment.height().to_string();
        }

        let current_width = environment.width();
        let current_height = environment.height();

        ui.horizontal(|ui| {
            ui.label("Width:");
            ui.add(
                egui::TextEdit::singleline(&mut self.env_width_text)
                    .desired_width(60.0)
                    .hint_text("1920"),
            );
        });

        ui.horizontal(|ui| {
            ui.label("Height:");
            ui.add(
                egui::TextEdit::singleline(&mut self.env_height_text)
                    .desired_width(60.0)
                    .hint_text("1080"),
            );
        });

        // Parse pending values
        let pending_width = self.env_width_text.parse::<u32>().ok();
        let pending_height = self.env_height_text.parse::<u32>().ok();
        let has_pending_change = match (pending_width, pending_height) {
            (Some(w), Some(h)) => w != current_width || h != current_height,
            _ => false,
        };

        // Show warning if resolution differs
        if has_pending_change {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Pending: {}x{} (current: {}x{})",
                        pending_width.unwrap_or(0),
                        pending_height.unwrap_or(0),
                        current_width,
                        current_height
                    ))
                    .color(egui::Color32::YELLOW)
                    .small(),
                );
            });
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let apply_enabled =
                has_pending_change && pending_width.is_some() && pending_height.is_some();
            if ui
                .add_enabled(apply_enabled, egui::Button::new("Apply Resolution"))
                .clicked()
            {
                if let (Some(w), Some(h)) = (pending_width, pending_height) {
                    if w > 0 && h > 0 {
                        self.pending_resolution = Some((w, h));
                        self.show_resolution_confirm = true;
                    }
                }
            }
            if ui.button("Reset").clicked() {
                self.env_width_text = current_width.to_string();
                self.env_height_text = current_height.to_string();
            }
        });

        // Common presets
        ui.add_space(8.0);
        ui.label("Presets:");
        ui.horizontal_wrapped(|ui| {
            if ui.small_button("1920x1080").clicked() {
                self.env_width_text = "1920".to_string();
                self.env_height_text = "1080".to_string();
            }
            if ui.small_button("3840x2160").clicked() {
                self.env_width_text = "3840".to_string();
                self.env_height_text = "2160".to_string();
            }
            if ui.small_button("1920x1200").clicked() {
                self.env_width_text = "1920".to_string();
                self.env_height_text = "1200".to_string();
            }
        });

        ui.add_space(16.0);
        ui.separator();

        // ========== NETWORK DISCOVERY ==========
        ui.add_space(8.0);
        ui.heading("Network Discovery");
        ui.add_space(4.0);

        // OMT Discovery toggle
        let mut omt_discovery = settings.omt_discovery_enabled;
        if ui
            .checkbox(&mut omt_discovery, "OMT Discovery")
            .on_hover_text("Automatically discover OMT sources on the network")
            .changed()
        {
            actions.push(PropertiesAction::SetOmtDiscovery {
                enabled: omt_discovery,
            });
        }
        if omt_discovery_active {
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.label(
                    egui::RichText::new("Discovering...")
                        .small()
                        .color(egui::Color32::GREEN),
                );
            });
        }

        ui.add_space(4.0);

        // NDI Discovery toggle
        let mut ndi_discovery = settings.ndi_discovery_enabled;
        if ui
            .checkbox(&mut ndi_discovery, "NDI Discovery")
            .on_hover_text("Automatically discover NDI sources on the network")
            .changed()
        {
            actions.push(PropertiesAction::SetNdiDiscovery {
                enabled: ndi_discovery,
            });
        }
        if ndi_discovery_active {
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.label(
                    egui::RichText::new("Discovering...")
                        .small()
                        .color(egui::Color32::from_rgb(100, 149, 237)), // Cornflower blue for NDI
                );
            });
        }

        ui.add_space(16.0);
        ui.separator();

        // ========== FRAME RATE ==========
        ui.add_space(8.0);
        ui.heading("Frame Rate");
        ui.add_space(4.0);

        // VSYNC checkbox
        let mut vsync = settings.vsync_enabled;
        if ui
            .checkbox(&mut vsync, "VSYNC")
            .on_hover_text("Sync to display refresh rate (disables manual FPS control)")
            .changed()
        {
            actions.push(PropertiesAction::SetVsyncEnabled { enabled: vsync });
        }

        ui.add_space(4.0);

        // Sync temp_fps from settings if it drifted
        if self.temp_fps != settings.target_fps {
            self.temp_fps = settings.target_fps;
        }

        // FPS controls (disabled when VSYNC is enabled)
        ui.add_enabled_ui(!vsync, |ui| {
            // FPS slider
            ui.horizontal(|ui| {
                ui.label("Target FPS:");
                let mut response = ui.add(
                    egui::Slider::new(&mut self.temp_fps, 24..=240)
                        .suffix(" fps")
                        .clamping(egui::SliderClamping::Always),
                );
                // Right-click instantly resets to 60 fps
                super::widgets::add_reset_u32(&mut response, &mut self.temp_fps, 60);
                if response.changed() {
                    actions.push(PropertiesAction::SetTargetFPS { fps: self.temp_fps });
                }
            });

            // FPS presets
            ui.horizontal_wrapped(|ui| {
                ui.label("Presets:");
                for &fps in &[24u32, 30, 60, 120, 144, 240] {
                    if ui.small_button(format!("{}", fps)).clicked() {
                        self.temp_fps = fps;
                        actions.push(PropertiesAction::SetTargetFPS { fps });
                    }
                }
            });
        });

        ui.add_space(4.0);
        if vsync {
            ui.label(
                egui::RichText::new("Synced to display refresh rate")
                    .small()
                    .weak(),
            );
        } else {
            ui.label(
                egui::RichText::new(format!("Targeting {} fps", self.temp_fps))
                    .small()
                    .weak(),
            );
        }

        ui.add_space(8.0);

        // Show FPS checkbox
        let mut show_fps = settings.show_fps;
        if ui
            .checkbox(&mut show_fps, "Show FPS in menu bar")
            .changed()
        {
            actions.push(PropertiesAction::SetShowFPS { show: show_fps });
        }

        ui.add_space(8.0);

        // Test pattern checkbox
        let mut test_pattern = settings.test_pattern_enabled;
        if ui
            .checkbox(&mut test_pattern, "Show Test Pattern")
            .on_hover_text("Replace composition with calibration test pattern")
            .changed()
        {
            actions.push(PropertiesAction::SetTestPattern { enabled: test_pattern });
        }

        ui.add_space(16.0);
        ui.separator();

        // ========== CLIP GRID ==========
        ui.add_space(8.0);
        ui.heading("Clip Grid");
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Thumbnail Mode:");
            let mut current_mode = settings.thumbnail_mode;
            egui::ComboBox::from_id_salt("pref_thumbnail_mode")
                .selected_text(current_mode.display_name())
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(
                            &mut current_mode,
                            ThumbnailMode::Fit,
                            ThumbnailMode::Fit.display_name(),
                        )
                        .changed()
                    {
                        actions.push(PropertiesAction::SetThumbnailMode {
                            mode: ThumbnailMode::Fit,
                        });
                    }
                    if ui
                        .selectable_value(
                            &mut current_mode,
                            ThumbnailMode::Fill,
                            ThumbnailMode::Fill.display_name(),
                        )
                        .changed()
                    {
                        actions.push(PropertiesAction::SetThumbnailMode {
                            mode: ThumbnailMode::Fill,
                        });
                    }
                });
        });

        ui.add_space(16.0);
        ui.separator();

        // ========== PERFORMANCE MODE ==========
        ui.add_space(8.0);
        ui.heading("Performance Mode");
        ui.add_space(4.0);

        let mut floor_sync = settings.floor_sync_enabled;
        if ui
            .checkbox(&mut floor_sync, "Floor Sync")
            .on_hover_text("When enabled, triggering clips on any layer also triggers the corresponding column clip on the floor layer")
            .changed()
        {
            actions.push(PropertiesAction::SetFloorSyncEnabled { enabled: floor_sync });
        }

        ui.add_enabled_ui(floor_sync, |ui| {
            ui.horizontal(|ui| {
                ui.label("Floor Layer:");
                let mut layer_idx = settings.floor_layer_index as u32;
                let mut response = ui
                    .add(egui::DragValue::new(&mut layer_idx).range(0..=15).speed(0.1));
                // Right-click instantly resets to 0
                super::widgets::add_reset_u32(&mut response, &mut layer_idx, 0);
                if response.changed() {
                    actions.push(PropertiesAction::SetFloorLayerIndex { index: layer_idx as usize });
                }
                ui.label(egui::RichText::new("(0 = first layer)").small().weak());
            });
        });

        ui.add_space(8.0);

        // Low Latency Mode toggle
        let mut low_latency = settings.low_latency_mode;
        if ui
            .checkbox(&mut low_latency, "Low Latency Mode")
            .on_hover_text("Reduces input lag by ~16ms but may cause stuttering under heavy GPU load. Disable for smoother playback.")
            .changed()
        {
            actions.push(PropertiesAction::SetLowLatencyMode { enabled: low_latency });
        }

        ui.add_space(8.0);

        // BGRA Pipeline Mode toggle
        let mut bgra_pipeline = settings.bgra_pipeline_enabled;
        if ui
            .checkbox(&mut bgra_pipeline, "BGRA Pipeline Mode")
            .on_hover_text("Use BGRA format throughout video pipeline. Matches NDI/OMT native format, reduces CPU color conversion. Requires restart.")
            .changed()
        {
            actions.push(PropertiesAction::SetBgraPipelineEnabled { enabled: bgra_pipeline });
        }

        ui.add_space(16.0);
        ui.separator();

        // ========== OMT BROADCAST ==========
        ui.add_space(8.0);
        ui.heading("OMT Broadcast");
        ui.add_space(4.0);

        let mut broadcast_enabled = settings.omt_broadcast_enabled;
        if ui
            .checkbox(&mut broadcast_enabled, "Broadcast Output via OMT")
            .changed()
        {
            actions.push(PropertiesAction::SetOmtBroadcast {
                enabled: broadcast_enabled,
            });
        }

        if omt_broadcasting {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Broadcasting...")
                    .small()
                    .color(egui::Color32::GREEN),
            );
        }

        ui.add_space(16.0);
        ui.separator();

        // ========== NDI BROADCAST ==========
        ui.add_space(8.0);
        ui.heading("NDI Broadcast");
        ui.add_space(4.0);

        let mut ndi_enabled = settings.ndi_broadcast_enabled;
        if ui
            .checkbox(&mut ndi_enabled, "Broadcast Output via NDI")
            .changed()
        {
            actions.push(PropertiesAction::SetNdiBroadcast {
                enabled: ndi_enabled,
            });
        }

        if ndi_broadcasting {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Broadcasting...")
                    .small()
                    .color(egui::Color32::from_rgb(100, 149, 237)), // Cornflower blue for NDI
            );
        }

        ui.add_space(16.0);
        ui.separator();

        // ========== NDI RECEIVE ==========
        ui.add_space(8.0);
        ui.heading("NDI Receive");
        ui.add_space(4.0);

        let mut buffer_capacity = settings.ndi_buffer_capacity as i32;
        let mut response = ui.add(
            egui::Slider::new(&mut buffer_capacity, 1..=10)
                .text("Buffer Size")
                .suffix(" frames"),
        );
        // Right-click instantly resets to 3 frames
        super::widgets::add_reset_i32(&mut response, &mut buffer_capacity, 3);
        if response.changed() {
            actions.push(PropertiesAction::SetNdiBufferCapacity {
                capacity: buffer_capacity as usize,
            });
        }
        ui.label(
            egui::RichText::new("Higher values reduce drops but add latency")
                .small()
                .weak(),
        );

        ui.add_space(16.0);
        ui.separator();

        // ========== SYPHON/SPOUT OUTPUT ==========
        ui.add_space(8.0);
        #[cfg(target_os = "macos")]
        let tech_name = "Syphon";
        #[cfg(target_os = "windows")]
        let tech_name = "Spout";
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let tech_name = "Texture Share";

        ui.heading(format!("{} Output", tech_name));
        ui.add_space(4.0);

        let mut share_enabled = settings.texture_share_enabled;
        if ui
            .checkbox(&mut share_enabled, format!("Share via {}", tech_name))
            .changed()
        {
            actions.push(PropertiesAction::SetTextureShare {
                enabled: share_enabled,
            });
        }

        if texture_sharing_active {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Sharing...")
                    .small()
                    .color(egui::Color32::GREEN),
            );
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Not available on this platform")
                    .small()
                    .color(egui::Color32::GRAY),
            );
        }

        ui.add_space(16.0);
        ui.separator();

        // ========== REST API SERVER ==========
        ui.add_space(8.0);
        ui.heading("REST API Server");
        ui.add_space(4.0);

        let mut api_enabled = settings.api_server_enabled;
        if ui
            .checkbox(&mut api_enabled, "Enable REST API")
            .changed()
        {
            actions.push(PropertiesAction::SetApiServer {
                enabled: api_enabled,
            });
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Port:");
            ui.label(egui::RichText::new(format!("{}", settings.api_port)).weak());
        });

        if api_server_running {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!(
                    "Running on http://localhost:{}",
                    settings.api_port
                ))
                .small()
                .color(egui::Color32::GREEN),
            );
        }

        ui.add_space(16.0);
        ui.separator();

        // ========== AUDIO INPUT ==========
        ui.add_space(8.0);
        ui.heading("Audio Input");
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Audio source for FFT analysis and reactive effects")
                .small()
                .weak(),
        );
        ui.add_space(8.0);

        // Refresh cached audio devices if needed (only when dirty, not every frame)
        if self.audio_devices_dirty {
            self.cached_audio_devices = AudioManager::list_audio_devices();
            self.audio_devices_dirty = false;
        }

        // Clone cached devices for use in closure
        let system_devices = self.cached_audio_devices.clone();

        // Audio source dropdown
        let current_source = &settings.audio_source;
        egui::ComboBox::from_id_salt("audio_source_selector")
            .selected_text(current_source.display_name())
            .width(250.0)
            .show_ui(ui, |ui| {
                // Disabled option
                if ui
                    .selectable_label(matches!(current_source, AudioSourceType::None), "Disabled")
                    .clicked()
                {
                    actions.push(PropertiesAction::SetAudioSource {
                        source_type: AudioSourceType::None,
                    });
                }

                // System Default
                if ui
                    .selectable_label(
                        matches!(current_source, AudioSourceType::SystemDefault),
                        "System Default",
                    )
                    .clicked()
                {
                    actions.push(PropertiesAction::SetAudioSource {
                        source_type: AudioSourceType::SystemDefault,
                    });
                }

                // System Devices (flat list)
                if !system_devices.is_empty() {
                    ui.separator();
                    ui.label(egui::RichText::new("System Devices").small().weak());
                    for device in &system_devices {
                        let is_selected = matches!(
                            current_source,
                            AudioSourceType::SystemDevice(d) if d == device
                        );
                        if ui.selectable_label(is_selected, device).clicked() {
                            actions.push(PropertiesAction::SetAudioSource {
                                source_type: AudioSourceType::SystemDevice(device.clone()),
                            });
                        }
                    }
                }

                // NDI sources (flat list)
                let ndi_sources: Vec<_> = discovered_sources
                    .iter()
                    .filter(|s| s.source_type == SourceType::Ndi)
                    .collect();
                if !ndi_sources.is_empty() {
                    ui.separator();
                    ui.label(egui::RichText::new("NDI Sources").small().weak());
                    for source in &ndi_sources {
                        let full_name = source.id.clone();
                        let is_selected =
                            matches!(current_source, AudioSourceType::Ndi(n) if n == &full_name);
                        if ui.selectable_label(is_selected, &source.name).clicked() {
                            actions.push(PropertiesAction::SetAudioSource {
                                source_type: AudioSourceType::Ndi(full_name),
                            });
                        }
                    }
                }

                // OMT sources (flat list)
                let omt_sources: Vec<_> = discovered_sources
                    .iter()
                    .filter(|s| s.source_type == SourceType::Omt)
                    .collect();
                if !omt_sources.is_empty() {
                    ui.separator();
                    ui.label(egui::RichText::new("OMT Sources").small().weak());
                    for source in &omt_sources {
                        let address = source.id.clone();
                        let is_selected =
                            matches!(current_source, AudioSourceType::Omt(a) if a == &address);
                        if ui.selectable_label(is_selected, &source.name).clicked() {
                            actions.push(PropertiesAction::SetAudioSource {
                                source_type: AudioSourceType::Omt(address),
                            });
                        }
                    }
                }
            });

        // Refresh button for audio devices
        ui.horizontal(|ui| {
            if ui.small_button("Refresh Devices").clicked() {
                self.audio_devices_dirty = true;
            }
            ui.label(
                egui::RichText::new(format!("{} device(s) found", self.cached_audio_devices.len()))
                    .small()
                    .weak(),
            );
        });

        ui.add_space(8.0);

        // Level meter
        if let Some(manager) = audio_manager {
            let (low, mid, high) = manager.get_band_levels();

            ui.horizontal(|ui| {
                ui.label("Level:");

                let meter_width = ui.available_width().min(200.0);
                let meter_height = 16.0;

                let (rect, _response) =
                    ui.allocate_exact_size(egui::vec2(meter_width, meter_height), egui::Sense::hover());

                if ui.is_rect_visible(rect) {
                    let painter = ui.painter();

                    // Background
                    painter.rect_filled(rect, 2.0, egui::Color32::from_gray(30));

                    let segment_width = rect.width() / 3.0;

                    // Low band (red/orange)
                    let low_width = segment_width * low.clamp(0.0, 1.0);
                    let low_rect = egui::Rect::from_min_size(rect.min, egui::vec2(low_width, meter_height));
                    painter.rect_filled(low_rect, 2.0, egui::Color32::from_rgb(255, 100, 50));

                    // Mid band (green)
                    let mid_start = rect.min.x + segment_width;
                    let mid_width = segment_width * mid.clamp(0.0, 1.0);
                    let mid_rect = egui::Rect::from_min_size(
                        egui::pos2(mid_start, rect.min.y),
                        egui::vec2(mid_width, meter_height),
                    );
                    painter.rect_filled(mid_rect, 0.0, egui::Color32::from_rgb(50, 255, 100));

                    // High band (blue)
                    let high_start = rect.min.x + 2.0 * segment_width;
                    let high_width = segment_width * high.clamp(0.0, 1.0);
                    let high_rect = egui::Rect::from_min_size(
                        egui::pos2(high_start, rect.min.y),
                        egui::vec2(high_width, meter_height),
                    );
                    painter.rect_filled(high_rect, 2.0, egui::Color32::from_rgb(50, 150, 255));

                    // Segment dividers
                    painter.vline(
                        rect.min.x + segment_width,
                        rect.y_range(),
                        egui::Stroke::new(1.0, egui::Color32::from_gray(60)),
                    );
                    painter.vline(
                        rect.min.x + 2.0 * segment_width,
                        rect.y_range(),
                        egui::Stroke::new(1.0, egui::Color32::from_gray(60)),
                    );
                }
            });

            // Request repaint for animation
            if settings.audio_source.is_enabled() {
                ui.ctx().request_repaint();
            }
        }

        ui.add_space(4.0);

        // Status text
        if settings.audio_source.is_enabled() {
            ui.label(
                egui::RichText::new(format!("Active: {}", settings.audio_source.display_name()))
                    .small()
                    .color(egui::Color32::GREEN),
            );
        } else {
            ui.label(
                egui::RichText::new("No audio source selected")
                    .small()
                    .color(egui::Color32::GRAY),
            );
        }

        ui.add_space(16.0);
    }

    /// Render the resolution confirmation dialog
    fn render_resolution_confirm_dialog(
        &mut self,
        ctx: &egui::Context,
        environment: &Environment,
        actions: &mut Vec<PropertiesAction>,
    ) {
        if !self.show_resolution_confirm {
            return;
        }

        let Some((w, h)) = self.pending_resolution else {
            return;
        };

        let current_width = environment.width();
        let current_height = environment.height();

        egui::Window::new("Confirm Resolution Change")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!(
                    "Change resolution from {}x{} to {}x{}?",
                    current_width, current_height, w, h
                ));
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("This may affect performance with large resolutions.")
                        .small()
                        .color(egui::Color32::GRAY),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Apply").clicked() {
                        actions.push(PropertiesAction::SetEnvironmentSize { width: w, height: h });
                        self.show_resolution_confirm = false;
                        self.pending_resolution = None;
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_resolution_confirm = false;
                        self.pending_resolution = None;
                        // Reset text fields to current values
                        self.env_width_text = current_width.to_string();
                        self.env_height_text = current_height.to_string();
                    }
                });
            });
    }

    /// Sync resolution text fields with current environment values
    /// Call this when the environment resolution changes externally
    pub fn sync_resolution(&mut self, width: u32, height: u32) {
        self.env_width_text = width.to_string();
        self.env_height_text = height.to_string();
    }
}
