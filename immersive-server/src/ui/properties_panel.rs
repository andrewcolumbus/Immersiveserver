//! Properties Panel with Environment/Layer/Clip tabs
//!
//! A tabbed panel for editing properties of the environment, layers, and clips.
//! Includes transform controls, tiling (multiplexing), and other settings.

use crate::compositor::{BlendMode, ClipTransition, Environment, Layer};
use crate::settings::EnvironmentSettings;

/// Which tab is currently active in the properties panel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PropertiesTab {
    /// Environment settings (resolution, background)
    #[default]
    Environment,
    /// Layer settings (transform, opacity, blend mode)
    Layer,
    /// Clip settings (transport, tiling, source info)
    Clip,
}

/// Actions that can be returned from the properties panel
#[derive(Debug, Clone)]
pub enum PropertiesAction {
    /// Environment resolution changed
    SetEnvironmentSize { width: u32, height: u32 },
    /// Target FPS changed
    SetTargetFPS { fps: u32 },
    /// Show FPS toggle changed
    SetShowFPS { show: bool },
    /// Layer opacity changed
    SetLayerOpacity { layer_id: u32, opacity: f32 },
    /// Layer blend mode changed
    SetLayerBlendMode { layer_id: u32, blend_mode: BlendMode },
    /// Layer visibility changed
    SetLayerVisibility { layer_id: u32, visible: bool },
    /// Layer position changed
    SetLayerPosition { layer_id: u32, x: f32, y: f32 },
    /// Layer scale changed
    SetLayerScale { layer_id: u32, scale_x: f32, scale_y: f32 },
    /// Layer rotation changed
    SetLayerRotation { layer_id: u32, degrees: f32 },
    /// Layer tiling changed
    SetLayerTiling { layer_id: u32, tile_x: u32, tile_y: u32 },
    /// Layer transition changed
    SetLayerTransition { layer_id: u32, transition: ClipTransition },
}

/// Properties panel state
pub struct PropertiesPanel {
    /// Currently active tab
    pub active_tab: PropertiesTab,
    /// Currently selected layer ID (for Layer/Clip tabs)
    pub selected_layer_id: Option<u32>,
    /// Whether the panel is open
    pub open: bool,
    /// Temporary values for editing (to avoid per-frame updates)
    env_width_text: String,
    env_height_text: String,
    /// Whether resolution confirmation dialog is open
    show_resolution_confirm: bool,
    /// Pending resolution to apply after confirmation
    pending_resolution: Option<(u32, u32)>,
    /// Temporary FPS value for slider editing
    temp_fps: u32,
}

impl Default for PropertiesPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl PropertiesPanel {
    /// Create a new properties panel
    pub fn new() -> Self {
        Self {
            active_tab: PropertiesTab::Environment,
            selected_layer_id: None,
            open: true,
            env_width_text: String::new(),
            env_height_text: String::new(),
            show_resolution_confirm: false,
            pending_resolution: None,
            temp_fps: 60,
        }
    }

    /// Select a layer to edit
    pub fn select_layer(&mut self, layer_id: u32) {
        self.selected_layer_id = Some(layer_id);
        self.active_tab = PropertiesTab::Layer;
    }

    /// Clear layer selection
    pub fn clear_selection(&mut self) {
        self.selected_layer_id = None;
    }

    /// Render the properties panel
    ///
    /// Returns a list of actions to be processed by the app.
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        environment: &Environment,
        layers: &[Layer],
        settings: &EnvironmentSettings,
    ) -> Vec<PropertiesAction> {
        let mut actions = Vec::new();

        // Tab bar
        ui.horizontal(|ui| {
            if ui
                .selectable_label(self.active_tab == PropertiesTab::Environment, "Environment")
                .clicked()
            {
                self.active_tab = PropertiesTab::Environment;
            }
            if ui
                .selectable_label(self.active_tab == PropertiesTab::Layer, "Layer")
                .clicked()
            {
                self.active_tab = PropertiesTab::Layer;
            }
            if ui
                .selectable_label(self.active_tab == PropertiesTab::Clip, "Clip")
                .clicked()
            {
                self.active_tab = PropertiesTab::Clip;
            }
        });

        ui.separator();

        // Tab content
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                match self.active_tab {
                    PropertiesTab::Environment => {
                        self.render_environment_tab(ui, environment, settings, &mut actions);
                    }
                    PropertiesTab::Layer => {
                        self.render_layer_tab(ui, layers, &mut actions);
                    }
                    PropertiesTab::Clip => {
                        self.render_clip_tab(ui, layers, &mut actions);
                    }
                }
            });

        actions
    }

    /// Render the Environment tab
    fn render_environment_tab(
        &mut self,
        ui: &mut egui::Ui,
        environment: &Environment,
        settings: &EnvironmentSettings,
        actions: &mut Vec<PropertiesAction>,
    ) {
        ui.heading("Environment");
        ui.add_space(8.0);

        // Resolution section
        ui.label("Resolution");
        ui.add_space(4.0);

        // Initialize text fields if empty
        if self.env_width_text.is_empty() {
            self.env_width_text = environment.width().to_string();
        }
        if self.env_height_text.is_empty() {
            self.env_height_text = environment.height().to_string();
        }

        // Current resolution
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
                        "Pending: {}×{} (current: {}×{})",
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
            let apply_enabled = has_pending_change && pending_width.is_some() && pending_height.is_some();
            if ui.add_enabled(apply_enabled, egui::Button::new("Apply Resolution")).clicked() {
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

        // Confirmation dialog
        if self.show_resolution_confirm {
            if let Some((w, h)) = self.pending_resolution {
                egui::Window::new("Confirm Resolution Change")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ui.ctx(), |ui| {
                        ui.label(format!(
                            "Change resolution from {}×{} to {}×{}?",
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
        }

        // Common presets (just update text fields, user must click Apply)
        ui.add_space(8.0);
        ui.label("Presets (click to select, then Apply):");
        ui.horizontal_wrapped(|ui| {
            if ui.small_button("1920×1080").clicked() {
                self.env_width_text = "1920".to_string();
                self.env_height_text = "1080".to_string();
            }
            if ui.small_button("3840×2160").clicked() {
                self.env_width_text = "3840".to_string();
                self.env_height_text = "2160".to_string();
            }
            if ui.small_button("1920×1200").clicked() {
                self.env_width_text = "1920".to_string();
                self.env_height_text = "1200".to_string();
            }
        });

        ui.add_space(16.0);
        ui.separator();

        // Frame Rate section
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
        if ui.checkbox(&mut show_fps, "Show FPS in menu bar").changed() {
            actions.push(PropertiesAction::SetShowFPS { show: show_fps });
        }

        ui.add_space(16.0);
        ui.separator();

        // Layer count info
        ui.add_space(8.0);
        ui.label(format!("Layers: {}", environment.layer_count()));
    }

    /// Render the Layer tab
    fn render_layer_tab(
        &mut self,
        ui: &mut egui::Ui,
        layers: &[Layer],
        actions: &mut Vec<PropertiesAction>,
    ) {
        // Layer selector
        ui.horizontal(|ui| {
            ui.label("Layer:");
            egui::ComboBox::from_id_salt("layer_selector")
                .selected_text(
                    self.selected_layer_id
                        .and_then(|id| layers.iter().find(|l| l.id == id))
                        .map(|l| l.name.as_str())
                        .unwrap_or("Select..."),
                )
                .show_ui(ui, |ui| {
                    for layer in layers {
                        if ui
                            .selectable_value(
                                &mut self.selected_layer_id,
                                Some(layer.id),
                                &layer.name,
                            )
                            .clicked()
                        {
                            // Selection changed
                        }
                    }
                });
        });

        ui.separator();

        // Get selected layer
        let Some(layer_id) = self.selected_layer_id else {
            ui.centered_and_justified(|ui| {
                ui.label("Select a layer to edit");
            });
            return;
        };

        let Some(layer) = layers.iter().find(|l| l.id == layer_id) else {
            ui.label("Layer not found");
            return;
        };

        ui.add_space(8.0);

        // Visibility toggle
        let mut visible = layer.visible;
        if ui.checkbox(&mut visible, "Visible").changed() {
            actions.push(PropertiesAction::SetLayerVisibility {
                layer_id,
                visible,
            });
        }

        ui.add_space(8.0);

        // Opacity
        let mut opacity = layer.opacity;
        ui.horizontal(|ui| {
            ui.label("Opacity:");
            let response = ui.add(egui::Slider::new(&mut opacity, 0.0..=1.0).show_value(true));
            if response.changed() {
                actions.push(PropertiesAction::SetLayerOpacity { layer_id, opacity });
            }
            response.context_menu(|ui| {
                if ui.button("Reset to 100%").clicked() {
                    actions.push(PropertiesAction::SetLayerOpacity { layer_id, opacity: 1.0 });
                    ui.close_menu();
                }
            });
        });

        ui.add_space(8.0);

        // Blend mode
        ui.horizontal(|ui| {
            ui.label("Blend Mode:");
            let mut blend_mode = layer.blend_mode;
            egui::ComboBox::from_id_salt("blend_mode")
                .selected_text(blend_mode.name())
                .show_ui(ui, |ui| {
                    for mode in BlendMode::all() {
                        if ui
                            .selectable_value(&mut blend_mode, *mode, mode.name())
                            .changed()
                        {
                            actions.push(PropertiesAction::SetLayerBlendMode {
                                layer_id,
                                blend_mode,
                            });
                        }
                    }
                });
        });

        ui.add_space(8.0);

        // Transition
        ui.horizontal(|ui| {
            ui.label("Transition:");
            let current = layer.transition;
            let is_cut = matches!(current, ClipTransition::Cut);
            let is_fade = matches!(current, ClipTransition::Fade(_));

            if ui.selectable_label(is_cut, "Cut").clicked() && !is_cut {
                actions.push(PropertiesAction::SetLayerTransition {
                    layer_id,
                    transition: ClipTransition::Cut,
                });
            }
            if ui.selectable_label(is_fade, "Fade").clicked() && !is_fade {
                actions.push(PropertiesAction::SetLayerTransition {
                    layer_id,
                    transition: ClipTransition::fade(),
                });
            }
        });

        ui.add_space(16.0);
        ui.separator();

        // Transform section
        ui.add_space(8.0);
        ui.heading("Transform");
        ui.add_space(8.0);

        // Position
        let mut pos_x = layer.transform.position.0;
        let mut pos_y = layer.transform.position.1;
        ui.horizontal(|ui| {
            ui.label("Position X:");
            let response = ui.add(egui::DragValue::new(&mut pos_x).speed(1.0));
            if response.changed() {
                actions.push(PropertiesAction::SetLayerPosition {
                    layer_id,
                    x: pos_x,
                    y: pos_y,
                });
            }
            response.context_menu(|ui| {
                if ui.button("Reset to 0").clicked() {
                    actions.push(PropertiesAction::SetLayerPosition { layer_id, x: 0.0, y: pos_y });
                    ui.close_menu();
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label("Position Y:");
            let response = ui.add(egui::DragValue::new(&mut pos_y).speed(1.0));
            if response.changed() {
                actions.push(PropertiesAction::SetLayerPosition {
                    layer_id,
                    x: pos_x,
                    y: pos_y,
                });
            }
            response.context_menu(|ui| {
                if ui.button("Reset to 0").clicked() {
                    actions.push(PropertiesAction::SetLayerPosition { layer_id, x: pos_x, y: 0.0 });
                    ui.close_menu();
                }
            });
        });

        ui.add_space(4.0);

        // Scale
        let mut scale_x = layer.transform.scale.0 * 100.0;
        let mut scale_y = layer.transform.scale.1 * 100.0;
        ui.horizontal(|ui| {
            ui.label("Scale:");
            let response_x = ui.add(
                egui::DragValue::new(&mut scale_x)
                    .speed(1.0)
                    .suffix("%")
                    .range(1.0..=1000.0),
            );
            if response_x.changed() {
                actions.push(PropertiesAction::SetLayerScale {
                    layer_id,
                    scale_x: scale_x / 100.0,
                    scale_y: scale_y / 100.0,
                });
            }
            response_x.context_menu(|ui| {
                if ui.button("Reset to 100%").clicked() {
                    actions.push(PropertiesAction::SetLayerScale { layer_id, scale_x: 1.0, scale_y: scale_y / 100.0 });
                    ui.close_menu();
                }
                if ui.button("Reset Both to 100%").clicked() {
                    actions.push(PropertiesAction::SetLayerScale { layer_id, scale_x: 1.0, scale_y: 1.0 });
                    ui.close_menu();
                }
            });
            ui.label("×");
            let response_y = ui.add(
                egui::DragValue::new(&mut scale_y)
                    .speed(1.0)
                    .suffix("%")
                    .range(1.0..=1000.0),
            );
            if response_y.changed() {
                actions.push(PropertiesAction::SetLayerScale {
                    layer_id,
                    scale_x: scale_x / 100.0,
                    scale_y: scale_y / 100.0,
                });
            }
            response_y.context_menu(|ui| {
                if ui.button("Reset to 100%").clicked() {
                    actions.push(PropertiesAction::SetLayerScale { layer_id, scale_x: scale_x / 100.0, scale_y: 1.0 });
                    ui.close_menu();
                }
                if ui.button("Reset Both to 100%").clicked() {
                    actions.push(PropertiesAction::SetLayerScale { layer_id, scale_x: 1.0, scale_y: 1.0 });
                    ui.close_menu();
                }
            });
        });

        ui.add_space(4.0);

        // Rotation
        let mut rotation_deg = layer.transform.rotation.to_degrees();
        ui.horizontal(|ui| {
            ui.label("Rotation:");
            let response = ui.add(
                egui::DragValue::new(&mut rotation_deg)
                    .speed(1.0)
                    .suffix("°")
                    .range(-360.0..=360.0),
            );
            if response.changed() {
                actions.push(PropertiesAction::SetLayerRotation {
                    layer_id,
                    degrees: rotation_deg,
                });
            }
            response.context_menu(|ui| {
                if ui.button("Reset to 0°").clicked() {
                    actions.push(PropertiesAction::SetLayerRotation { layer_id, degrees: 0.0 });
                    ui.close_menu();
                }
            });
        });

        ui.add_space(16.0);
        ui.separator();

        // Tiling section
        ui.add_space(8.0);
        ui.heading("Tiling (Multiplex)");
        ui.add_space(8.0);

        let mut tile_x = layer.tile_x;
        let mut tile_y = layer.tile_y;

        ui.horizontal(|ui| {
            ui.label("Tile X:");
            let response = ui.add(egui::DragValue::new(&mut tile_x).range(1..=16));
            if response.changed() {
                actions.push(PropertiesAction::SetLayerTiling { layer_id, tile_x, tile_y });
            }
            response.context_menu(|ui| {
                if ui.button("Reset to 1").clicked() {
                    actions.push(PropertiesAction::SetLayerTiling { layer_id, tile_x: 1, tile_y });
                    ui.close_menu();
                }
                if ui.button("Reset Both to 1×1").clicked() {
                    actions.push(PropertiesAction::SetLayerTiling { layer_id, tile_x: 1, tile_y: 1 });
                    ui.close_menu();
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label("Tile Y:");
            let response = ui.add(egui::DragValue::new(&mut tile_y).range(1..=16));
            if response.changed() {
                actions.push(PropertiesAction::SetLayerTiling { layer_id, tile_x, tile_y });
            }
            response.context_menu(|ui| {
                if ui.button("Reset to 1").clicked() {
                    actions.push(PropertiesAction::SetLayerTiling { layer_id, tile_x, tile_y: 1 });
                    ui.close_menu();
                }
                if ui.button("Reset Both to 1×1").clicked() {
                    actions.push(PropertiesAction::SetLayerTiling { layer_id, tile_x: 1, tile_y: 1 });
                    ui.close_menu();
                }
            });
        });

        // Tiling presets
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if ui.small_button("1×1").clicked() {
                actions.push(PropertiesAction::SetLayerTiling {
                    layer_id,
                    tile_x: 1,
                    tile_y: 1,
                });
            }
            if ui.small_button("2×2").clicked() {
                actions.push(PropertiesAction::SetLayerTiling {
                    layer_id,
                    tile_x: 2,
                    tile_y: 2,
                });
            }
            if ui.small_button("3×3").clicked() {
                actions.push(PropertiesAction::SetLayerTiling {
                    layer_id,
                    tile_x: 3,
                    tile_y: 3,
                });
            }
            if ui.small_button("4×4").clicked() {
                actions.push(PropertiesAction::SetLayerTiling {
                    layer_id,
                    tile_x: 4,
                    tile_y: 4,
                });
            }
        });
    }

    /// Render the Clip tab
    fn render_clip_tab(
        &mut self,
        ui: &mut egui::Ui,
        layers: &[Layer],
        _actions: &mut Vec<PropertiesAction>,
    ) {
        // Get selected layer
        let Some(layer_id) = self.selected_layer_id else {
            ui.centered_and_justified(|ui| {
                ui.label("Select a layer to see clip info");
            });
            return;
        };

        let Some(layer) = layers.iter().find(|l| l.id == layer_id) else {
            ui.label("Layer not found");
            return;
        };

        // Get active clip
        let Some(active_slot) = layer.active_clip else {
            ui.centered_and_justified(|ui| {
                ui.label("No clip playing on this layer");
            });
            return;
        };

        let Some(clip) = layer.get_clip(active_slot) else {
            ui.label("Active clip not found");
            return;
        };

        ui.heading("Clip Info");
        ui.add_space(8.0);

        // Source info
        ui.label(format!("Name: {}", clip.display_name()));
        ui.add_space(4.0);

        ui.label("Path:");
        ui.add_space(2.0);
        let path_str = clip.source_path.display().to_string();
        ui.add(
            egui::TextEdit::singleline(&mut path_str.clone())
                .interactive(false)
                .desired_width(f32::INFINITY),
        );

        ui.add_space(8.0);
        ui.label(format!("Slot: {}", active_slot + 1));

        ui.add_space(16.0);
        ui.separator();

        // Transport controls placeholder
        ui.add_space(8.0);
        ui.heading("Transport");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("(Transport controls will be implemented in a future update)");
        });
    }
}

