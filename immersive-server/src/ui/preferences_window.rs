//! Preferences Window
//!
//! A floating window for editing application-wide environment settings.
//! Accessible via Immersive Server → Preferences (macOS) or Edit → Preferences (Windows).

use crate::compositor::Environment;
use crate::settings::{EnvironmentSettings, ThumbnailMode};
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
        }
    }

    /// Toggle the window open/closed
    pub fn toggle(&mut self) {
        self.open = !self.open;
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

        // ========== FRAME RATE ==========
        ui.add_space(8.0);
        ui.heading("Frame Rate");
        ui.add_space(4.0);

        // Sync temp_fps from settings if it drifted
        if self.temp_fps != settings.target_fps {
            self.temp_fps = settings.target_fps;
        }

        // FPS slider
        ui.horizontal(|ui| {
            ui.label("Target FPS:");
            let response = ui.add(
                egui::Slider::new(&mut self.temp_fps, 24..=240)
                    .suffix(" fps")
                    .clamping(egui::SliderClamping::Always),
            );
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

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(format!("Targeting {} fps", self.temp_fps))
                .small()
                .weak(),
        );

        ui.add_space(8.0);

        // Show FPS checkbox
        let mut show_fps = settings.show_fps;
        if ui
            .checkbox(&mut show_fps, "Show FPS in menu bar")
            .changed()
        {
            actions.push(PropertiesAction::SetShowFPS { show: show_fps });
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
                let mut layer_idx = settings.floor_layer_index;
                if ui
                    .add(egui::DragValue::new(&mut layer_idx).range(0..=15).speed(0.1))
                    .changed()
                {
                    actions.push(PropertiesAction::SetFloorLayerIndex { index: layer_idx });
                }
                ui.label(egui::RichText::new("(0 = first layer)").small().weak());
            });
        });

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
