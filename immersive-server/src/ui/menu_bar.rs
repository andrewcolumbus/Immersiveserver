//! Menu bar and panels for Immersive Server
//!
//! Provides the main menu bar with File, View, and Environment panels.

use crate::settings::EnvironmentSettings;
use std::path::PathBuf;

/// UI state for the menu bar and panels
pub struct MenuBar {
    /// Whether the Environment panel is open
    pub environment_panel_open: bool,

    /// Pending file dialog action
    pub pending_action: Option<FileAction>,

    /// Temporary FPS value while editing (for slider)
    pub temp_fps: u32,

    /// Temporary environment width while editing
    pub temp_environment_width: u32,

    /// Temporary environment height while editing
    pub temp_environment_height: u32,

    /// Status message to display
    pub status_message: Option<(String, std::time::Instant)>,
}

/// File-related actions
#[derive(Debug, Clone)]
pub enum FileAction {
    /// Open a .immersive file
    Open,
    /// Save current settings
    Save,
    /// Save settings to a new file
    SaveAs,
    /// Open a video file
    OpenVideo,
}

impl Default for MenuBar {
    fn default() -> Self {
        Self {
            environment_panel_open: false,
            pending_action: None,
            temp_fps: 60,
            temp_environment_width: 1920,
            temp_environment_height: 1080,
            status_message: None,
        }
    }
}

impl MenuBar {
    /// Create a new MenuBar with settings
    pub fn new(settings: &EnvironmentSettings) -> Self {
        Self {
            temp_fps: settings.target_fps,
            temp_environment_width: settings.environment_width,
            temp_environment_height: settings.environment_height,
            ..Default::default()
        }
    }

    /// Sync temp values from settings
    pub fn sync_from_settings(&mut self, settings: &EnvironmentSettings) {
        self.temp_fps = settings.target_fps;
        self.temp_environment_width = settings.environment_width;
        self.temp_environment_height = settings.environment_height;
    }

    /// Set a status message that will display for a few seconds
    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some((message.into(), std::time::Instant::now()));
    }

    /// Render the menu bar and panels
    /// Returns true if settings were modified
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        settings: &mut EnvironmentSettings,
        current_file: &Option<PathBuf>,
        fps: f64,
        frame_time_ms: f64,
    ) -> bool {
        let mut settings_changed = false;

        // Top menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                // File menu
                ui.menu_button("File", |ui| {
                    if ui.button("Open Environment...").clicked() {
                        self.pending_action = Some(FileAction::Open);
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui.button("Save").clicked() {
                        self.pending_action = Some(FileAction::Save);
                        ui.close_menu();
                    }

                    if ui.button("Save As...").clicked() {
                        self.pending_action = Some(FileAction::SaveAs);
                        ui.close_menu();
                    }

                    ui.separator();

                    if let Some(path) = current_file {
                        ui.label(format!(
                            "Current: {}",
                            path.file_name()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_else(|| "Unknown".to_string())
                        ));
                    } else {
                        ui.label("No file loaded");
                    }
                });

                // View menu
                ui.menu_button("View", |ui| {
                    if ui
                        .checkbox(&mut settings.show_fps, "Show FPS")
                        .changed()
                    {
                        settings_changed = true;
                    }

                    ui.separator();

                    if ui.button("Environment...").clicked() {
                        self.environment_panel_open = true;
                        self.sync_from_settings(settings);
                        ui.close_menu();
                    }
                });

                // Show current file name in menu bar
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if settings.show_fps {
                        ui.label(
                            egui::RichText::new(format!("{:.1} fps | {:.2}ms", fps, frame_time_ms))
                                .monospace()
                                .color(egui::Color32::from_rgb(120, 200, 120)),
                        );
                        ui.separator();
                    }

                    // Status message (fades after 3 seconds)
                    if let Some((msg, time)) = &self.status_message {
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
                            self.status_message = None;
                        }
                    }
                });
            });
        });

        // Environment panel (floating window)
        if self.environment_panel_open {
            let mut open = self.environment_panel_open;
            egui::Window::new("Environment")
                .open(&mut open)
                .resizable(false)
                .default_width(350.0)
                .show(ctx, |ui| {
                    ui.heading("Environment Settings");
                    ui.separator();

                    // Environment resolution (composition canvas)
                    ui.horizontal(|ui| {
                        ui.label("Environment:");
                        ui.add(
                            egui::DragValue::new(&mut self.temp_environment_width)
                                .range(1..=16384)
                                .suffix(" px"),
                        );
                        ui.label("×");
                        ui.add(
                            egui::DragValue::new(&mut self.temp_environment_height)
                                .range(1..=16384)
                                .suffix(" px"),
                        );
                    });

                    ui.label(
                        egui::RichText::new("The window previews the environment scaled to fit.")
                            .small()
                            .weak(),
                    );

                    ui.separator();

                    // FPS slider
                    ui.horizontal(|ui| {
                        ui.label("Target FPS:");
                        ui.add(
                            egui::Slider::new(&mut self.temp_fps, 24..=240)
                                .suffix(" fps")
                                .clamping(egui::SliderClamping::Always),
                        );
                    });

                    // Common FPS presets
                    ui.horizontal(|ui| {
                        ui.label("Presets:");
                        if ui.button("24").clicked() {
                            self.temp_fps = 24;
                        }
                        if ui.button("30").clicked() {
                            self.temp_fps = 30;
                        }
                        if ui.button("60").clicked() {
                            self.temp_fps = 60;
                        }
                        if ui.button("120").clicked() {
                            self.temp_fps = 120;
                        }
                        if ui.button("144").clicked() {
                            self.temp_fps = 144;
                        }
                        if ui.button("240").clicked() {
                            self.temp_fps = 240;
                        }
                    });

                    ui.label(
                        egui::RichText::new(format!("Targeting {} fps", self.temp_fps))
                            .small()
                            .weak(),
                    );

                    ui.separator();

                    // Show FPS checkbox
                    if ui.checkbox(&mut settings.show_fps, "Show FPS in menu bar").changed() {
                        settings_changed = true;
                    }

                    ui.separator();

                    // Apply / Cancel buttons
                    ui.horizontal(|ui| {
                        if ui.button("Apply").clicked() {
                            let mut any_changes = false;

                            if self.temp_environment_width != settings.environment_width
                                || self.temp_environment_height != settings.environment_height
                            {
                                settings.environment_width = self.temp_environment_width;
                                settings.environment_height = self.temp_environment_height;
                                settings_changed = true;
                                any_changes = true;
                                self.set_status(format!(
                                    "Environment set to {}x{}",
                                    self.temp_environment_width, self.temp_environment_height
                                ));
                            }

                            if self.temp_fps != settings.target_fps {
                                settings.target_fps = self.temp_fps;
                                settings_changed = true;
                                any_changes = true;
                                self.set_status(format!("Target FPS set to {}", self.temp_fps));
                            }

                            if !any_changes {
                                self.set_status("No changes");
                            }
                        }

                        if ui.button("Cancel").clicked() {
                            self.sync_from_settings(settings);
                            self.environment_panel_open = false;
                        }
                    });
                });
            self.environment_panel_open = open;
        }

        settings_changed
    }

    /// Take pending file action (consumes it)
    pub fn take_pending_action(&mut self) -> Option<FileAction> {
        self.pending_action.take()
    }
}

