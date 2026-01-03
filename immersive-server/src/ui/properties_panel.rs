//! Properties Panel with Environment/Layer/Clip tabs
//!
//! A tabbed panel for editing properties of the environment, layers, and clips.
//! Includes transform controls, tiling (multiplexing), effects, and other settings.

use crate::compositor::{BlendMode, ClipTransition, Environment, Layer};
use crate::effects::{EffectRegistry, EffectStack, ParameterValue};
use crate::settings::{EnvironmentSettings, ThumbnailMode};
use crate::ui::effects_browser_panel::DraggableEffect;

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
    /// OMT broadcast toggle changed
    SetOmtBroadcast { enabled: bool },
    /// OMT capture FPS changed
    SetOmtCaptureFps { fps: u32 },
    /// NDI broadcast toggle changed
    SetNdiBroadcast { enabled: bool },
    /// NDI capture FPS changed
    SetNdiCaptureFps { fps: u32 },
    /// Syphon/Spout texture sharing toggle changed
    SetTextureShare { enabled: bool },
    /// Thumbnail mode changed
    SetThumbnailMode { mode: ThumbnailMode },

    // Layer effect-related actions
    /// Add effect to layer
    AddLayerEffect { layer_id: u32, effect_type: String },
    /// Remove effect from layer
    RemoveLayerEffect { layer_id: u32, effect_id: u32 },
    /// Toggle effect bypass
    SetLayerEffectBypassed { layer_id: u32, effect_id: u32, bypassed: bool },
    /// Toggle effect solo
    SetLayerEffectSoloed { layer_id: u32, effect_id: u32, soloed: bool },
    /// Set effect parameter value
    SetLayerEffectParameter { layer_id: u32, effect_id: u32, param_name: String, value: ParameterValue },
    /// Reorder effect (move up/down)
    ReorderLayerEffect { layer_id: u32, effect_id: u32, new_index: usize },
    /// Toggle effect expanded/collapsed state
    SetLayerEffectExpanded { layer_id: u32, effect_id: u32, expanded: bool },

    // Clip effect-related actions
    /// Add effect to clip
    AddClipEffect { layer_id: u32, slot: usize, effect_type: String },
    /// Remove effect from clip
    RemoveClipEffect { layer_id: u32, slot: usize, effect_id: u32 },
    /// Toggle clip effect bypass
    SetClipEffectBypassed { layer_id: u32, slot: usize, effect_id: u32, bypassed: bool },
    /// Toggle clip effect solo
    SetClipEffectSoloed { layer_id: u32, slot: usize, effect_id: u32, soloed: bool },
    /// Set clip effect parameter value
    SetClipEffectParameter { layer_id: u32, slot: usize, effect_id: u32, param_name: String, value: ParameterValue },
    /// Reorder clip effect (move up/down)
    ReorderClipEffect { layer_id: u32, slot: usize, effect_id: u32, new_index: usize },
    /// Toggle clip effect expanded/collapsed state
    SetClipEffectExpanded { layer_id: u32, slot: usize, effect_id: u32, expanded: bool },

    // Environment effect-related actions
    /// Add effect to environment (master effects)
    AddEnvironmentEffect { effect_type: String },
    /// Remove effect from environment
    RemoveEnvironmentEffect { effect_id: u32 },
    /// Toggle environment effect bypass
    SetEnvironmentEffectBypassed { effect_id: u32, bypassed: bool },
    /// Toggle environment effect solo
    SetEnvironmentEffectSoloed { effect_id: u32, soloed: bool },
    /// Set environment effect parameter value
    SetEnvironmentEffectParameter { effect_id: u32, param_name: String, value: ParameterValue },
    /// Reorder environment effect (move up/down)
    ReorderEnvironmentEffect { effect_id: u32, new_index: usize },
    /// Toggle environment effect expanded/collapsed state
    SetEnvironmentEffectExpanded { effect_id: u32, expanded: bool },
}

/// Context for rendering effect stacks (determines which PropertiesAction variants to emit)
#[derive(Debug, Clone, Copy)]
enum EffectContext {
    /// Effects on a layer
    Layer { layer_id: u32 },
    /// Effects on a clip within a layer
    Clip { layer_id: u32, slot: usize },
    /// Effects on the environment (master effects)
    Environment,
}

/// Properties panel state
pub struct PropertiesPanel {
    /// Currently active tab
    pub active_tab: PropertiesTab,
    /// Currently selected layer ID (for Layer/Clip tabs)
    pub selected_layer_id: Option<u32>,
    /// Currently selected clip slot index (for Clip tab)
    pub selected_clip_slot: Option<usize>,
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
            selected_clip_slot: None,
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
        self.selected_clip_slot = None; // Clear clip selection when changing layers
        self.active_tab = PropertiesTab::Layer;
    }

    /// Select a clip to edit (also selects the containing layer)
    pub fn select_clip(&mut self, layer_id: u32, slot: usize) {
        self.selected_layer_id = Some(layer_id);
        self.selected_clip_slot = Some(slot);
        self.active_tab = PropertiesTab::Clip;
    }

    /// Clear layer and clip selection
    pub fn clear_selection(&mut self) {
        self.selected_layer_id = None;
        self.selected_clip_slot = None;
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
        omt_broadcasting: bool,
        ndi_broadcasting: bool,
        texture_sharing_active: bool,
        effect_registry: &EffectRegistry,
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
                        self.render_environment_tab(ui, environment, settings, omt_broadcasting, ndi_broadcasting, texture_sharing_active, effect_registry, &mut actions);
                    }
                    PropertiesTab::Layer => {
                        self.render_layer_tab(ui, layers, effect_registry, &mut actions);
                    }
                    PropertiesTab::Clip => {
                        self.render_clip_tab(ui, layers, effect_registry, &mut actions);
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
        omt_broadcasting: bool,
        ndi_broadcasting: bool,
        texture_sharing_active: bool,
        effect_registry: &EffectRegistry,
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

        // Clip Grid section
        ui.add_space(8.0);
        ui.heading("Clip Grid");
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Thumbnail Mode:");
            let mut current_mode = settings.thumbnail_mode;
            egui::ComboBox::from_id_salt("thumbnail_mode")
                .selected_text(current_mode.display_name())
                .show_ui(ui, |ui| {
                    if ui.selectable_value(&mut current_mode, ThumbnailMode::Fit, ThumbnailMode::Fit.display_name()).changed() {
                        actions.push(PropertiesAction::SetThumbnailMode { mode: ThumbnailMode::Fit });
                    }
                    if ui.selectable_value(&mut current_mode, ThumbnailMode::Fill, ThumbnailMode::Fill.display_name()).changed() {
                        actions.push(PropertiesAction::SetThumbnailMode { mode: ThumbnailMode::Fill });
                    }
                });
        });

        ui.add_space(16.0);
        ui.separator();

        // OMT Broadcast section
        ui.add_space(8.0);
        ui.heading("OMT Broadcast");
        ui.add_space(4.0);

        let mut broadcast_enabled = settings.omt_broadcast_enabled;
        if ui.checkbox(&mut broadcast_enabled, "Broadcast Output via OMT").changed() {
            actions.push(PropertiesAction::SetOmtBroadcast { enabled: broadcast_enabled });
        }

        if omt_broadcasting {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Broadcasting...")
                    .small()
                    .color(egui::Color32::GREEN),
            );
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Capture FPS:");
            let mut fps = settings.omt_capture_fps;
            let slider = egui::Slider::new(&mut fps, 10..=60)
                .suffix(" fps")
                .clamping(egui::SliderClamping::Always);
            if ui.add(slider).changed() {
                actions.push(PropertiesAction::SetOmtCaptureFps { fps });
            }
        });

        ui.add_space(16.0);
        ui.separator();

        // NDI Broadcast section
        ui.add_space(8.0);
        ui.heading("NDI Broadcast");
        ui.add_space(4.0);

        let mut ndi_enabled = settings.ndi_broadcast_enabled;
        if ui.checkbox(&mut ndi_enabled, "Broadcast Output via NDI").changed() {
            actions.push(PropertiesAction::SetNdiBroadcast { enabled: ndi_enabled });
        }

        if ndi_broadcasting {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Broadcasting...")
                    .small()
                    .color(egui::Color32::from_rgb(100, 149, 237)), // Cornflower blue for NDI
            );
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Capture FPS:");
            let mut fps = settings.ndi_capture_fps;
            let slider = egui::Slider::new(&mut fps, 10..=60)
                .suffix(" fps")
                .clamping(egui::SliderClamping::Always);
            if ui.add(slider).changed() {
                actions.push(PropertiesAction::SetNdiCaptureFps { fps });
            }
        });

        ui.add_space(16.0);
        ui.separator();

        // Syphon/Spout Texture Sharing section
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

        // Layer count info
        ui.add_space(8.0);
        ui.label(format!("Layers: {}", environment.layer_count()));

        // ========== MASTER EFFECTS ==========
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);
        ui.heading("Master Effects");
        ui.add_space(8.0);

        self.render_effect_stack_generic(ui, EffectContext::Environment, environment.effects(), effect_registry, actions);
    }

    /// Render the Layer tab
    fn render_layer_tab(
        &mut self,
        ui: &mut egui::Ui,
        layers: &[Layer],
        effect_registry: &EffectRegistry,
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
        let mut uniform_scale = scale_x; // Use X as the uniform value
        ui.horizontal(|ui| {
            ui.label("Scale:");
            // Uniform scale (controls both X and Y)
            let response_uniform = ui.add(
                egui::DragValue::new(&mut uniform_scale)
                    .speed(1.0)
                    .suffix("%")
                    .range(1.0..=1000.0),
            );
            if response_uniform.changed() {
                actions.push(PropertiesAction::SetLayerScale {
                    layer_id,
                    scale_x: uniform_scale / 100.0,
                    scale_y: uniform_scale / 100.0,
                });
            }
            response_uniform.context_menu(|ui| {
                if ui.button("Reset to 100%").clicked() {
                    actions.push(PropertiesAction::SetLayerScale { layer_id, scale_x: 1.0, scale_y: 1.0 });
                    ui.close_menu();
                }
            });
            ui.add_space(8.0);
            // Independent X scale
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
            // Independent Y scale
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

        // Effects section
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);
        ui.heading("Effects");
        ui.add_space(8.0);

        self.render_effect_stack_generic(ui, EffectContext::Layer { layer_id }, &layer.effects, effect_registry, actions);
    }

    /// Render an effect stack for any context (layer, clip, or environment)
    ///
    /// Clean design with collapsible effect sections and aligned parameter rows.
    fn render_effect_stack_generic(
        &self,
        ui: &mut egui::Ui,
        context: EffectContext,
        effects: &EffectStack,
        effect_registry: &EffectRegistry,
        actions: &mut Vec<PropertiesAction>,
    ) {
        // Check if a DraggableEffect is specifically being dragged
        let is_dragging_effect = egui::DragAndDrop::payload::<DraggableEffect>(ui.ctx()).is_some();

        // Visual feedback for drop zone only when dragging an effect
        let frame = if is_dragging_effect {
            egui::Frame::new()
                .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 149, 237)))
                .inner_margin(4.0)
                .corner_radius(4.0)
        } else {
            egui::Frame::NONE
        };

        // Wrap effects list in a drop zone
        let drop_response = frame.show(ui, |ui| {
            ui.dnd_drop_zone::<DraggableEffect, ()>(egui::Frame::NONE, |ui| {
                if effects.is_empty() {
                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new("Drop effect or mask. Double click to search.")
                            .color(egui::Color32::from_gray(100)),
                    );
                    ui.add_space(12.0);
                } else {
                    let effect_count = effects.effects.len();

                    // Render each effect as a collapsible section
                    for (index, effect) in effects.effects.iter().enumerate() {
                        ui.push_id(effect.id, |ui| {
                            // Header background color - darker for section headers
                            let header_bg = if effect.bypassed {
                                egui::Color32::from_gray(35)
                            } else {
                                egui::Color32::from_rgb(45, 65, 55) // Subtle green tint when active
                            };

                            // Effect header with collapsible state
                            let header_response = egui::Frame::new()
                                .fill(header_bg)
                                .inner_margin(egui::Margin::symmetric(6, 4))
                                .corner_radius(2.0)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        // Disclosure triangle
                                        let arrow = if effect.expanded { "▼" } else { "▶" };
                                        let arrow_response = ui.add(
                                            egui::Label::new(egui::RichText::new(arrow).size(10.0).color(egui::Color32::GRAY))
                                                .sense(egui::Sense::click())
                                        );

                                        // Effect name
                                        let name_color = if effect.bypassed {
                                            egui::Color32::from_gray(100)
                                        } else {
                                            egui::Color32::from_gray(200)
                                        };
                                        let name_text = egui::RichText::new(&effect.name).color(name_color);
                                        ui.label(name_text);

                                        // Status indicators on the right
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            // Bypass indicator
                                            if effect.bypassed {
                                                ui.label(egui::RichText::new("B").size(10.0).color(egui::Color32::from_gray(80)));
                                            }
                                            // Solo indicator
                                            if effect.soloed {
                                                ui.label(egui::RichText::new("S").size(10.0).color(egui::Color32::YELLOW));
                                            }
                                        });

                                        arrow_response
                                    })
                                });

                            // Handle click on arrow or header to toggle expanded state
                            if header_response.inner.inner.clicked() || header_response.response.clicked() {
                                self.push_expanded_action(actions, context, effect.id, !effect.expanded);
                            }

                            // Right-click context menu for effect controls
                            header_response.response.context_menu(|ui| {
                                // Bypass toggle
                                let bypass_label = if effect.bypassed { "Enable" } else { "Bypass" };
                                if ui.button(bypass_label).clicked() {
                                    self.push_bypass_action(actions, context, effect.id, !effect.bypassed);
                                    ui.close_menu();
                                }

                                // Solo toggle
                                let solo_label = if effect.soloed { "Unsolo" } else { "Solo" };
                                if ui.button(solo_label).clicked() {
                                    self.push_solo_action(actions, context, effect.id, !effect.soloed);
                                    ui.close_menu();
                                }

                                ui.separator();

                                // Reorder options
                                if index > 0 {
                                    if ui.button("Move Up").clicked() {
                                        self.push_reorder_action(actions, context, effect.id, index - 1);
                                        ui.close_menu();
                                    }
                                }
                                if index < effect_count - 1 {
                                    if ui.button("Move Down").clicked() {
                                        self.push_reorder_action(actions, context, effect.id, index + 1);
                                        ui.close_menu();
                                    }
                                }

                                ui.separator();

                                // Remove
                                if ui.button(egui::RichText::new("Remove").color(egui::Color32::from_rgb(200, 80, 80))).clicked() {
                                    self.push_remove_action(actions, context, effect.id);
                                    ui.close_menu();
                                }
                            });

                            // Render parameters if expanded and not bypassed
                            if effect.expanded && !effect.bypassed {
                                for param in &effect.parameters {
                                    self.render_effect_parameter_generic(ui, context, effect.id, param, actions);
                                }
                            }

                            ui.add_space(2.0);
                        });
                    }
                }
            })
        });

        // Handle dropped effect
        if let Some(dragged_effect) = drop_response.inner.1 {
            self.push_add_action(actions, context, dragged_effect.effect_type.clone());
        }

        ui.add_space(4.0);

        // Add effect dropdown - cleaner style
        ui.horizontal(|ui| {
            ui.menu_button("+ Add Effect", |ui| {
                for category in effect_registry.categories() {
                    ui.menu_button(category, |ui| {
                        if let Some(effect_types) = effect_registry.effects_in_category(category) {
                            for effect_type in effect_types {
                                if let Some(def) = effect_registry.get(effect_type) {
                                    if ui.button(def.display_name()).clicked() {
                                        self.push_add_action(actions, context, effect_type.clone());
                                        ui.close_menu();
                                    }
                                }
                            }
                        }
                    });
                }
            });
        });
    }

    /// Render a single effect parameter with clean aligned layout
    ///
    /// Layout: [Label (right-aligned, fixed width)] [Value] [Slider]
    fn render_effect_parameter_generic(
        &self,
        ui: &mut egui::Ui,
        context: EffectContext,
        effect_id: u32,
        param: &crate::effects::Parameter,
        actions: &mut Vec<PropertiesAction>,
    ) {
        const LABEL_WIDTH: f32 = 80.0;

        ui.horizontal(|ui| {
            // Fixed-width right-aligned label
            ui.allocate_ui_with_layout(
                egui::vec2(LABEL_WIDTH, ui.spacing().interact_size.y),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    ui.label(egui::RichText::new(&param.meta.label).color(egui::Color32::from_gray(180)));
                },
            );

            ui.add_space(8.0);

            match &param.value {
                ParameterValue::Float(value) => {
                    let mut val = *value;
                    let min = param.meta.min.unwrap_or(0.0);
                    let max = param.meta.max.unwrap_or(1.0);

                    // Value display with +/- would be nice, but slider is cleaner
                    // Show value as text
                    ui.label(
                        egui::RichText::new(format!("{:.1}", val))
                            .color(egui::Color32::from_gray(200))
                            .monospace()
                    );

                    // Slider takes remaining width
                    let response = ui.add(
                        egui::Slider::new(&mut val, min..=max)
                            .show_value(false)
                            .clamping(egui::SliderClamping::Always)
                    );

                    if response.changed() {
                        self.push_param_action(actions, context, effect_id, param.meta.name.clone(), ParameterValue::Float(val));
                    }

                    // Right-click to reset to default
                    response.context_menu(|ui| {
                        if ui.button("Reset to Default").clicked() {
                            if let ParameterValue::Float(default_val) = param.meta.default {
                                self.push_param_action(actions, context, effect_id, param.meta.name.clone(), ParameterValue::Float(default_val));
                            }
                            ui.close_menu();
                        }
                    });
                }
                ParameterValue::Int(value) => {
                    let mut val = *value;
                    let min = param.meta.min.unwrap_or(0.0) as i32;
                    let max = param.meta.max.unwrap_or(100.0) as i32;

                    // Value display
                    ui.label(
                        egui::RichText::new(format!("{}", val))
                            .color(egui::Color32::from_gray(200))
                            .monospace()
                    );

                    // Slider
                    let response = ui.add(
                        egui::Slider::new(&mut val, min..=max)
                            .show_value(false)
                            .clamping(egui::SliderClamping::Always)
                    );

                    if response.changed() {
                        self.push_param_action(actions, context, effect_id, param.meta.name.clone(), ParameterValue::Int(val));
                    }

                    response.context_menu(|ui| {
                        if ui.button("Reset to Default").clicked() {
                            if let ParameterValue::Int(default_val) = param.meta.default {
                                self.push_param_action(actions, context, effect_id, param.meta.name.clone(), ParameterValue::Int(default_val));
                            }
                            ui.close_menu();
                        }
                    });
                }
                ParameterValue::Bool(value) => {
                    let mut val = *value;

                    // Value display
                    let val_text = if val { "On" } else { "Off" };
                    ui.label(
                        egui::RichText::new(val_text)
                            .color(egui::Color32::from_gray(200))
                            .monospace()
                    );

                    // Checkbox
                    let response = ui.checkbox(&mut val, "");

                    if response.changed() {
                        self.push_param_action(actions, context, effect_id, param.meta.name.clone(), ParameterValue::Bool(val));
                    }

                    response.context_menu(|ui| {
                        if ui.button("Reset to Default").clicked() {
                            if let ParameterValue::Bool(default_val) = param.meta.default {
                                self.push_param_action(actions, context, effect_id, param.meta.name.clone(), ParameterValue::Bool(default_val));
                            }
                            ui.close_menu();
                        }
                    });
                }
                ParameterValue::Enum { index, options } => {
                    let mut current_index = *index;
                    let selected_text = options.get(current_index).cloned().unwrap_or_default();

                    egui::ComboBox::from_id_salt(format!("enum_{}_{}", effect_id, param.meta.name))
                        .selected_text(&selected_text)
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            for (i, option) in options.iter().enumerate() {
                                if ui.selectable_value(&mut current_index, i, option).changed() {
                                    self.push_param_action(
                                        actions,
                                        context,
                                        effect_id,
                                        param.meta.name.clone(),
                                        ParameterValue::Enum {
                                            index: current_index,
                                            options: options.clone(),
                                        },
                                    );
                                }
                            }
                        });
                }
                ParameterValue::String(value) => {
                    let mut text = value.clone();

                    // Show filename only if path exists
                    let display_text = if text.is_empty() {
                        "(none)".to_string()
                    } else {
                        std::path::Path::new(&text)
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or(&text)
                            .to_string()
                    };

                    ui.label(
                        egui::RichText::new(&display_text)
                            .color(egui::Color32::from_gray(200))
                    );

                    let response = ui.add(
                        egui::TextEdit::singleline(&mut text)
                            .desired_width(100.0)
                            .hint_text("path...")
                    );

                    if response.changed() {
                        self.push_param_action(
                            actions,
                            context,
                            effect_id,
                            param.meta.name.clone(),
                            ParameterValue::String(text.clone()),
                        );
                    }

                    // Clear button
                    if !value.is_empty() && ui.small_button("×").on_hover_text("Clear").clicked() {
                        self.push_param_action(
                            actions,
                            context,
                            effect_id,
                            param.meta.name.clone(),
                            ParameterValue::String(String::new()),
                        );
                    }
                }
                _ => {
                    ui.label(egui::RichText::new("—").color(egui::Color32::from_gray(100)));
                }
            }
        });
    }

    // Helper methods for generating context-specific actions

    fn push_add_action(&self, actions: &mut Vec<PropertiesAction>, context: EffectContext, effect_type: String) {
        match context {
            EffectContext::Layer { layer_id } => {
                actions.push(PropertiesAction::AddLayerEffect { layer_id, effect_type });
            }
            EffectContext::Clip { layer_id, slot } => {
                actions.push(PropertiesAction::AddClipEffect { layer_id, slot, effect_type });
            }
            EffectContext::Environment => {
                actions.push(PropertiesAction::AddEnvironmentEffect { effect_type });
            }
        }
    }

    fn push_remove_action(&self, actions: &mut Vec<PropertiesAction>, context: EffectContext, effect_id: u32) {
        match context {
            EffectContext::Layer { layer_id } => {
                actions.push(PropertiesAction::RemoveLayerEffect { layer_id, effect_id });
            }
            EffectContext::Clip { layer_id, slot } => {
                actions.push(PropertiesAction::RemoveClipEffect { layer_id, slot, effect_id });
            }
            EffectContext::Environment => {
                actions.push(PropertiesAction::RemoveEnvironmentEffect { effect_id });
            }
        }
    }

    fn push_bypass_action(&self, actions: &mut Vec<PropertiesAction>, context: EffectContext, effect_id: u32, bypassed: bool) {
        match context {
            EffectContext::Layer { layer_id } => {
                actions.push(PropertiesAction::SetLayerEffectBypassed { layer_id, effect_id, bypassed });
            }
            EffectContext::Clip { layer_id, slot } => {
                actions.push(PropertiesAction::SetClipEffectBypassed { layer_id, slot, effect_id, bypassed });
            }
            EffectContext::Environment => {
                actions.push(PropertiesAction::SetEnvironmentEffectBypassed { effect_id, bypassed });
            }
        }
    }

    fn push_solo_action(&self, actions: &mut Vec<PropertiesAction>, context: EffectContext, effect_id: u32, soloed: bool) {
        match context {
            EffectContext::Layer { layer_id } => {
                actions.push(PropertiesAction::SetLayerEffectSoloed { layer_id, effect_id, soloed });
            }
            EffectContext::Clip { layer_id, slot } => {
                actions.push(PropertiesAction::SetClipEffectSoloed { layer_id, slot, effect_id, soloed });
            }
            EffectContext::Environment => {
                actions.push(PropertiesAction::SetEnvironmentEffectSoloed { effect_id, soloed });
            }
        }
    }

    fn push_reorder_action(&self, actions: &mut Vec<PropertiesAction>, context: EffectContext, effect_id: u32, new_index: usize) {
        match context {
            EffectContext::Layer { layer_id } => {
                actions.push(PropertiesAction::ReorderLayerEffect { layer_id, effect_id, new_index });
            }
            EffectContext::Clip { layer_id, slot } => {
                actions.push(PropertiesAction::ReorderClipEffect { layer_id, slot, effect_id, new_index });
            }
            EffectContext::Environment => {
                actions.push(PropertiesAction::ReorderEnvironmentEffect { effect_id, new_index });
            }
        }
    }

    fn push_param_action(&self, actions: &mut Vec<PropertiesAction>, context: EffectContext, effect_id: u32, param_name: String, value: ParameterValue) {
        match context {
            EffectContext::Layer { layer_id } => {
                actions.push(PropertiesAction::SetLayerEffectParameter { layer_id, effect_id, param_name, value });
            }
            EffectContext::Clip { layer_id, slot } => {
                actions.push(PropertiesAction::SetClipEffectParameter { layer_id, slot, effect_id, param_name, value });
            }
            EffectContext::Environment => {
                actions.push(PropertiesAction::SetEnvironmentEffectParameter { effect_id, param_name, value });
            }
        }
    }

    fn push_expanded_action(&self, actions: &mut Vec<PropertiesAction>, context: EffectContext, effect_id: u32, expanded: bool) {
        match context {
            EffectContext::Layer { layer_id } => {
                actions.push(PropertiesAction::SetLayerEffectExpanded { layer_id, effect_id, expanded });
            }
            EffectContext::Clip { layer_id, slot } => {
                actions.push(PropertiesAction::SetClipEffectExpanded { layer_id, slot, effect_id, expanded });
            }
            EffectContext::Environment => {
                actions.push(PropertiesAction::SetEnvironmentEffectExpanded { effect_id, expanded });
            }
        }
    }

    /// Render the Clip tab
    fn render_clip_tab(
        &mut self,
        ui: &mut egui::Ui,
        layers: &[Layer],
        effect_registry: &EffectRegistry,
        actions: &mut Vec<PropertiesAction>,
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

        // Use selected clip if set, otherwise fall back to active clip
        let clip_slot = self.selected_clip_slot.or(layer.active_clip);

        let Some(slot) = clip_slot else {
            ui.centered_and_justified(|ui| {
                ui.label("No clip selected");
            });
            return;
        };

        let Some(clip) = layer.get_clip(slot) else {
            ui.centered_and_justified(|ui| {
                ui.label("Selected clip not found");
            });
            return;
        };

        // Check if this clip is currently playing
        let is_playing = layer.active_clip == Some(slot);

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
        let status = if is_playing { " (playing)" } else { "" };
        ui.label(format!("Slot: {}{}", slot + 1, status));

        ui.add_space(16.0);
        ui.separator();

        // Transport controls placeholder
        ui.add_space(8.0);
        ui.heading("Transport");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("(Transport controls will be implemented in a future update)");
        });

        // ========== CLIP EFFECTS ==========
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);
        ui.heading("Clip Effects");
        ui.add_space(8.0);

        self.render_effect_stack_generic(ui, EffectContext::Clip { layer_id, slot }, &clip.effects, effect_registry, actions);
    }
}

