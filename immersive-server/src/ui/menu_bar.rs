//! Menu bar for Immersive Server
//!
//! Provides the main menu bar with File and View menus.

use crate::settings::EnvironmentSettings;
use super::layout_preset::LayoutPresetManager;
use std::path::PathBuf;

/// UI state for the menu bar and panels
pub struct MenuBar {
    /// Pending file dialog action
    pub pending_action: Option<FileAction>,

    /// Pending menu action (e.g., toggle panel)
    pub pending_menu_action: Option<MenuAction>,

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

/// Menu actions that need to be handled by the app
#[derive(Debug, Clone)]
pub enum MenuAction {
    /// Toggle a panel's visibility
    TogglePanel { panel_id: String },
    /// Open the HAP Converter window
    OpenHAPConverter,
    /// Open the Preferences window
    OpenPreferences,
    /// Apply a layout preset by index
    ApplyLayoutPreset { index: usize },
    /// Save the current layout
    SaveLayout,
    /// Load a layout from file
    LoadLayout,
    /// Reset to default layout
    ResetLayout,
}

impl Default for MenuBar {
    fn default() -> Self {
        Self {
            pending_action: None,
            pending_menu_action: None,
            status_message: None,
        }
    }
}

impl MenuBar {
    /// Create a new MenuBar with settings
    pub fn new(_settings: &EnvironmentSettings) -> Self {
        Self::default()
    }

    /// Set a status message that will display for a few seconds
    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some((message.into(), std::time::Instant::now()));
    }

    /// Render the menu bar and panels
    /// Returns true if settings were modified
    ///
    /// `panel_states` is a list of (panel_id, title, is_open) for View menu toggles.
    /// `layout_manager` is optional for rendering the Layout submenu.
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        settings: &mut EnvironmentSettings,
        current_file: &Option<PathBuf>,
        fps: f64,
        frame_time_ms: f64,
        panel_states: &[(&str, &str, bool)],
        layout_manager: Option<&LayoutPresetManager>,
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

                // Edit menu (for Preferences)
                ui.menu_button("Edit", |ui| {
                    if ui.button("Preferences...").clicked() {
                        self.pending_menu_action = Some(MenuAction::OpenPreferences);
                        ui.close_menu();
                    }
                });

                // View menu
                ui.menu_button("View", |ui| {
                    // Panel visibility toggles
                    ui.label(egui::RichText::new("Panels").weak().small());
                    for (panel_id, title, is_open) in panel_states {
                        let mut open = *is_open;
                        if ui.checkbox(&mut open, *title).clicked() {
                            self.pending_menu_action = Some(MenuAction::TogglePanel {
                                panel_id: panel_id.to_string(),
                            });
                        }
                    }

                    ui.separator();

                    if ui
                        .checkbox(&mut settings.show_fps, "Show FPS")
                        .changed()
                    {
                        settings_changed = true;
                    }

                    // Layout submenu
                    if let Some(manager) = layout_manager {
                        ui.separator();
                        ui.menu_button("Layout", |ui| {
                            let active_index = manager.active_preset_index();

                            // Built-in presets
                            ui.label(egui::RichText::new("Presets").weak().small());
                            for (index, preset) in manager.builtin_presets() {
                                let is_active = active_index == Some(index);
                                let label = if is_active {
                                    format!("✓ {}", preset.name)
                                } else {
                                    format!("   {}", preset.name)
                                };
                                if ui.button(&label).clicked() {
                                    self.pending_menu_action = Some(MenuAction::ApplyLayoutPreset { index });
                                    ui.close_menu();
                                }
                            }

                            // User presets (if any)
                            let user_presets: Vec<_> = manager.user_presets().collect();
                            if !user_presets.is_empty() {
                                ui.separator();
                                ui.label(egui::RichText::new("Custom").weak().small());
                                for (index, preset) in user_presets {
                                    let is_active = active_index == Some(index);
                                    let label = if is_active {
                                        format!("✓ {}", preset.name)
                                    } else {
                                        format!("   {}", preset.name)
                                    };
                                    if ui.button(&label).clicked() {
                                        self.pending_menu_action = Some(MenuAction::ApplyLayoutPreset { index });
                                        ui.close_menu();
                                    }
                                }
                            }

                            ui.separator();

                            if ui.button("Save Layout...").clicked() {
                                self.pending_menu_action = Some(MenuAction::SaveLayout);
                                ui.close_menu();
                            }

                            if ui.button("Reset Layout").clicked() {
                                self.pending_menu_action = Some(MenuAction::ResetLayout);
                                ui.close_menu();
                            }
                        });
                    }
                });

                // Tools menu
                ui.menu_button("Tools", |ui| {
                    if ui.button("HAP Converter...").clicked() {
                        self.pending_menu_action = Some(MenuAction::OpenHAPConverter);
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

        settings_changed
    }

    /// Take pending file action (consumes it)
    pub fn take_pending_action(&mut self) -> Option<FileAction> {
        self.pending_action.take()
    }

    /// Take pending menu action (consumes it)
    pub fn take_menu_action(&mut self) -> Option<MenuAction> {
        self.pending_menu_action.take()
    }
}

