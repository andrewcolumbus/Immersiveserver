//! Menu bar for Immersive Server
//!
//! Provides the main menu bar with File, Edit, View, and Tools menus.
//! Uses the shared MenuBarDefinition for the left side (menus), while the right side
//! (FPS/BPM status area) remains egui-only.

use super::layout_preset::LayoutPresetManager;
use super::menu_definition::{MenuBarDefinition, MenuDefinition, MenuItem, MenuItemAction, MenuItemId, SettingId};
use crate::settings::EnvironmentSettings;
use egui::PointerButton;
use std::path::PathBuf;

/// UI state for the menu bar and panels
#[derive(Default)]
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
    /// Open the Advanced Output window
    OpenAdvancedOutput,
    /// Apply a layout preset by index
    ApplyLayoutPreset { index: usize },
    /// Save the current layout
    SaveLayout,
    /// Load a layout from file
    LoadLayout,
    /// Reset to default layout
    ResetLayout,
    /// Set BPM value
    SetBpm { bpm: f32 },
    /// Tap tempo
    TapTempo,
    /// Resync to bar start
    ResyncBpm,
    /// Breakout environment viewport to separate window
    BreakoutEnvironment,
    /// Redock environment viewport back to main window
    RedockEnvironment,
}

/// BPM clock info for display
#[derive(Debug, Clone, Copy, Default)]
pub struct BpmInfo {
    /// Current BPM
    pub bpm: f32,
    /// Beats per bar (time signature numerator)
    pub beats_per_bar: u32,
    /// Current beat phase (0.0-1.0)
    pub beat_phase: f32,
    /// Current beat within bar (0 to beats_per_bar-1)
    pub current_beat_in_bar: u32,
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
    /// `bpm_info` is optional BPM clock information for the tempo display.
    /// `audio_levels` is the (low, mid, high) band levels for the audio meter.
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        settings: &mut EnvironmentSettings,
        current_file: &Option<PathBuf>,
        fps: f64,
        frame_time_ms: f64,
        panel_states: &[(&str, &str, bool)],
        layout_manager: Option<&LayoutPresetManager>,
        bpm_info: Option<BpmInfo>,
        audio_levels: (f32, f32, f32),
    ) -> bool {
        let mut settings_changed = false;
        let definition = MenuBarDefinition::build();

        // Top menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                // LEFT SIDE: Render menus from shared definition
                for menu_def in &definition.menus {
                    // Skip empty menus (e.g., Edit menu on macOS)
                    if menu_def.items.is_empty() {
                        continue;
                    }

                    self.render_menu(
                        ui,
                        menu_def,
                        settings,
                        current_file,
                        panel_states,
                        layout_manager,
                        &mut settings_changed,
                    );
                }

                // RIGHT SIDE: egui-only status area (FPS, BPM, audio, status messages)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.render_status_area(ui, settings, fps, frame_time_ms, bpm_info, audio_levels, &mut settings_changed);
                });
            });
        });

        settings_changed
    }

    /// Render a menu from definition
    fn render_menu(
        &mut self,
        ui: &mut egui::Ui,
        menu_def: &MenuDefinition,
        settings: &mut EnvironmentSettings,
        current_file: &Option<PathBuf>,
        panel_states: &[(&str, &str, bool)],
        layout_manager: Option<&LayoutPresetManager>,
        settings_changed: &mut bool,
    ) {
        ui.menu_button(&menu_def.label, |ui| {
            for item in &menu_def.items {
                self.render_menu_item(
                    ui,
                    item,
                    settings,
                    current_file,
                    panel_states,
                    layout_manager,
                    settings_changed,
                );
            }

            // Special case: File menu shows current file at the bottom
            if menu_def.label == "File" {
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
            }
        });
    }

    /// Render a single menu item
    fn render_menu_item(
        &mut self,
        ui: &mut egui::Ui,
        item: &MenuItem,
        settings: &mut EnvironmentSettings,
        _current_file: &Option<PathBuf>,
        panel_states: &[(&str, &str, bool)],
        layout_manager: Option<&LayoutPresetManager>,
        settings_changed: &mut bool,
    ) {
        match item {
            MenuItem::Action {
                id,
                label,
                shortcut,
                enabled,
            } => {
                let shortcut_text = shortcut.as_ref().map(|s| s.to_display_string()).unwrap_or_default();

                let button_text = if shortcut_text.is_empty() {
                    label.clone()
                } else {
                    format!("{}  {}", label, shortcut_text)
                };

                if ui.add_enabled(*enabled, egui::Button::new(&button_text)).clicked() {
                    self.handle_action(id);
                    ui.close_menu();
                }
            }

            MenuItem::Check { id, label, .. } => {
                let mut checked = self.get_check_state(id, panel_states, settings);
                if ui.checkbox(&mut checked, label.as_str()).clicked() {
                    self.handle_check_toggle(id, settings, settings_changed);
                }
            }

            MenuItem::Separator => {
                ui.separator();
            }

            MenuItem::Submenu { label, items } => {
                // Special handling for dynamic submenus
                match label.as_str() {
                    "Layout" => {
                        self.render_layout_submenu(ui, layout_manager, items, settings_changed);
                    }
                    _ => {
                        // Regular submenu
                        ui.menu_button(label, |ui| {
                            for sub_item in items {
                                self.render_menu_item(
                                    ui,
                                    sub_item,
                                    settings,
                                    _current_file,
                                    panel_states,
                                    layout_manager,
                                    settings_changed,
                                );
                            }
                        });
                    }
                }
            }

            MenuItem::Label { text } => {
                ui.label(egui::RichText::new(text).weak().small());
            }

            MenuItem::Predefined(_) => {
                // Predefined items are native-only, skip in egui
            }
        }
    }

    /// Render the Layout submenu with dynamic presets
    fn render_layout_submenu(
        &mut self,
        ui: &mut egui::Ui,
        layout_manager: Option<&LayoutPresetManager>,
        static_items: &[MenuItem],
        _settings_changed: &mut bool,
    ) {
        ui.menu_button("Layout", |ui| {
            if let Some(manager) = layout_manager {
                let active_index = manager.active_preset_index();

                // Built-in presets
                ui.label(egui::RichText::new("Presets").weak().small());
                for (index, preset) in manager.builtin_presets() {
                    let is_active = active_index == Some(index);
                    let label = if is_active {
                        format!("\u{2713} {}", preset.name)
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
                            format!("\u{2713} {}", preset.name)
                        } else {
                            format!("   {}", preset.name)
                        };
                        if ui.button(&label).clicked() {
                            self.pending_menu_action = Some(MenuAction::ApplyLayoutPreset { index });
                            ui.close_menu();
                        }
                    }
                }
            }

            // Static items from definition (separator, Save Layout, Reset Layout)
            for item in static_items {
                match item {
                    MenuItem::Separator => {
                        ui.separator();
                    }
                    MenuItem::Action { id, label, .. } => {
                        if ui.button(label.as_str()).clicked() {
                            self.handle_action(id);
                            ui.close_menu();
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    /// Render the right-side status area (egui-only)
    fn render_status_area(
        &mut self,
        ui: &mut egui::Ui,
        settings: &EnvironmentSettings,
        fps: f64,
        frame_time_ms: f64,
        bpm_info: Option<BpmInfo>,
        audio_levels: (f32, f32, f32),
        _settings_changed: &mut bool,
    ) {
        // Audio level meter (compact horizontal bar)
        {
            let (low, mid, high) = audio_levels;
            let meter_width = 60.0;
            let meter_height = 10.0;

            let (rect, _) = ui.allocate_exact_size(egui::vec2(meter_width, meter_height), egui::Sense::hover());

            if ui.is_rect_visible(rect) {
                let painter = ui.painter();

                // Background
                painter.rect_filled(rect, 2.0, egui::Color32::from_gray(30));

                let segment_width = rect.width() / 3.0;

                // Low band (orange)
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
            }
            ui.separator();
        }

        if settings.show_fps {
            ui.label(
                egui::RichText::new(format!("{:.1} fps | {:.2}ms", fps, frame_time_ms))
                    .monospace()
                    .color(egui::Color32::from_rgb(120, 200, 120)),
            );
            ui.separator();
        }

        // BPM display with beat indicators
        if settings.show_bpm {
            if let Some(info) = bpm_info {
                // Beat indicator dots (4/4 time signature visualization)
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;

                    // Beat dots - show current beat in bar
                    for beat in 0..info.beats_per_bar {
                        let is_current = beat == info.current_beat_in_bar;
                        let is_downbeat = beat == 0;

                        // Pulse effect for current beat
                        let pulse = if is_current { 1.0 - info.beat_phase * 0.5 } else { 0.5 };

                        let color = if is_current {
                            if is_downbeat {
                                // Downbeat - bright orange/gold
                                egui::Color32::from_rgb((255.0 * pulse) as u8, (180.0 * pulse) as u8, 50)
                            } else {
                                // Regular beat - bright cyan
                                egui::Color32::from_rgb(50, (200.0 * pulse) as u8, (255.0 * pulse) as u8)
                            }
                        } else {
                            // Inactive beat
                            egui::Color32::from_gray(60)
                        };

                        let size = if is_current { 8.0 } else { 6.0 };
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), size / 2.0, color);
                    }
                });

                ui.add_space(4.0);

                // Time signature
                ui.label(egui::RichText::new(format!("{}/4", info.beats_per_bar)).small().weak());

                ui.add_space(2.0);

                // Editable BPM value
                let mut bpm = info.bpm;
                let response = ui.add(
                    egui::DragValue::new(&mut bpm)
                        .speed(0.5)
                        .range(20.0..=300.0)
                        .suffix(" BPM")
                        .custom_formatter(|n, _| format!("{:.1}", n)),
                );

                if response.changed() {
                    self.pending_menu_action = Some(MenuAction::SetBpm { bpm });
                }

                // Right-click instantly resets to 120 BPM
                if response.clicked_by(PointerButton::Secondary) {
                    self.pending_menu_action = Some(MenuAction::SetBpm { bpm: 120.0 });
                }

                // Tap tempo button
                if ui
                    .add(egui::Button::new("TAP").small())
                    .on_hover_text("Tap to set tempo")
                    .clicked()
                {
                    self.pending_menu_action = Some(MenuAction::TapTempo);
                }

                // Resync button
                if ui
                    .add(egui::Button::new("\u{27F2}").small())
                    .on_hover_text("Resync to bar start")
                    .clicked()
                {
                    self.pending_menu_action = Some(MenuAction::ResyncBpm);
                }
            }
            ui.separator();
        }

        // Status message (fades after 3 seconds)
        if let Some((msg, time)) = &self.status_message {
            let elapsed = time.elapsed().as_secs_f32();
            if elapsed < 3.0 {
                let alpha = if elapsed > 2.0 { ((3.0 - elapsed) * 255.0) as u8 } else { 255 };
                ui.label(egui::RichText::new(msg).color(egui::Color32::from_rgba_unmultiplied(180, 180, 255, alpha)));
            } else {
                self.status_message = None;
            }
        }
    }

    /// Get the checked state for a menu item
    fn get_check_state(&self, id: &MenuItemId, panel_states: &[(&str, &str, bool)], settings: &EnvironmentSettings) -> bool {
        match id {
            MenuItemId::ShowFps => settings.show_fps,
            MenuItemId::ShowBpm => settings.show_bpm,
            _ => {
                // Panel toggle - look up in panel_states
                if let Some(panel_id) = id.to_panel_id() {
                    panel_states
                        .iter()
                        .find(|(pid, _, _)| *pid == panel_id)
                        .map(|(_, _, is_open)| *is_open)
                        .unwrap_or(false)
                } else {
                    false
                }
            }
        }
    }

    /// Handle an action menu item click
    fn handle_action(&mut self, id: &MenuItemId) {
        match id.to_action() {
            MenuItemAction::File(file_action) => {
                self.pending_action = Some(file_action);
            }
            MenuItemAction::Menu(menu_action) => {
                self.pending_menu_action = Some(menu_action);
            }
            _ => {}
        }
    }

    /// Handle a check menu item toggle
    fn handle_check_toggle(&mut self, id: &MenuItemId, settings: &mut EnvironmentSettings, settings_changed: &mut bool) {
        match id.to_action() {
            MenuItemAction::ToggleSetting(setting) => match setting {
                SettingId::ShowFps => {
                    settings.show_fps = !settings.show_fps;
                    *settings_changed = true;
                }
                SettingId::ShowBpm => {
                    settings.show_bpm = !settings.show_bpm;
                    *settings_changed = true;
                }
            },
            MenuItemAction::Menu(menu_action) => {
                // Panel toggles
                self.pending_menu_action = Some(menu_action);
            }
            _ => {}
        }
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
