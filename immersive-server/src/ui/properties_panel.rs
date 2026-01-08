//! Properties Panel with Environment/Layer/Clip tabs
//!
//! A tabbed panel for editing properties of the environment, layers, and clips.
//! Includes transform controls, effects, and other settings.

use std::collections::HashMap;

use crate::audio::AudioBand;
use crate::compositor::{BlendMode, ClipSource, ClipTransition, Environment, Layer, LoopMode};
use crate::effects::{AutomationSource, EffectRegistry, EffectStack, FftSource, LfoSource, LfoShape, BeatSource, BeatTrigger, ParameterValue};
use crate::layer_runtime::LayerVideoInfo;
use crate::settings::{EnvironmentSettings, ThumbnailMode};
use crate::ui::effects_browser_panel::DraggableEffect;
use egui_widgets::{video_scrubber, ScrubberAction, ScrubberState};

/// Payload for dragging effects within the stack for reordering
#[derive(Clone)]
struct DraggedEffectReorder {
    effect_id: u32,
    source_index: usize,
}

/// Which tab is currently active in the properties panel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PropertiesTab {
    /// Environment settings (resolution, background)
    #[default]
    Environment,
    /// Layer settings (transform, opacity, blend mode)
    Layer,
    /// Clip settings (transport, source info)
    Clip,
}

/// Actions that can be returned from the properties panel
#[derive(Debug, Clone)]
pub enum PropertiesAction {
    /// Environment resolution changed
    SetEnvironmentSize { width: u32, height: u32 },
    /// Target FPS changed
    SetTargetFPS { fps: u32 },
    /// VSYNC toggle changed
    SetVsyncEnabled { enabled: bool },
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
    /// NDI receive buffer capacity changed
    SetNdiBufferCapacity { capacity: usize },
    /// OMT discovery toggle changed
    SetOmtDiscovery { enabled: bool },
    /// NDI discovery toggle changed
    SetNdiDiscovery { enabled: bool },
    /// Syphon/Spout texture sharing toggle changed
    SetTextureShare { enabled: bool },
    /// REST API server toggle changed
    SetApiServer { enabled: bool },
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

    // Parameter automation actions
    /// Set automation for layer effect parameter
    SetLayerEffectParameterAutomation {
        layer_id: u32,
        effect_id: u32,
        param_name: String,
        automation: Option<AutomationSource>,
    },
    /// Set automation for clip effect parameter
    SetClipEffectParameterAutomation {
        layer_id: u32,
        slot: usize,
        effect_id: u32,
        param_name: String,
        automation: Option<AutomationSource>,
    },
    /// Set automation for environment effect parameter
    SetEnvironmentEffectParameterAutomation {
        effect_id: u32,
        param_name: String,
        automation: Option<AutomationSource>,
    },

    // Clip transform actions
    /// Clip position changed
    SetClipPosition { layer_id: u32, slot: usize, x: f32, y: f32 },
    /// Clip scale changed
    SetClipScale { layer_id: u32, slot: usize, scale_x: f32, scale_y: f32 },
    /// Clip rotation changed
    SetClipRotation { layer_id: u32, slot: usize, degrees: f32 },

    // Clip transport actions (for active/playing clips)
    /// Toggle clip playback (pause/resume)
    ToggleClipPlayback { layer_id: u32 },
    /// Start scrubbing (pauses and stores play state)
    StartScrub { layer_id: u32 },
    /// End scrubbing (seeks to final position and restores previous play state)
    EndScrub { layer_id: u32, time_secs: f64 },
    /// Restart clip from beginning
    RestartClip { layer_id: u32 },
    /// Seek clip to specific time
    SeekClip { layer_id: u32, time_secs: f64 },
    /// Preview this clip in the preview monitor
    PreviewClip { layer_id: u32, slot: usize },
    /// Set clip loop mode
    SetClipLoopMode { layer_id: u32, slot: usize, mode: LoopMode },

    // Performance mode actions
    /// Floor sync enabled changed
    SetFloorSyncEnabled { enabled: bool },
    /// Floor layer index changed
    SetFloorLayerIndex { index: usize },
    /// Low latency mode changed (trades stability for reduced input lag)
    SetLowLatencyMode { enabled: bool },
    /// Test pattern mode changed (replaces composition with calibration pattern)
    SetTestPattern { enabled: bool },
    /// BGRA pipeline mode changed (requires restart)
    SetBgraPipelineEnabled { enabled: bool },
}

/// Context for rendering effect stacks (determines which PropertiesAction variants to emit)
#[derive(Debug, Clone, Copy, PartialEq)]
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
    /// Scrubber states for each layer (for timeline scrubber)
    scrubber_states: HashMap<u32, ScrubberState>,
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
            scrubber_states: HashMap::new(),
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
        api_server_running: bool,
        effect_registry: &EffectRegistry,
        // Automation evaluation context (for real-time modulation visualization)
        bpm_clock: &crate::effects::BpmClock,
        effect_time: f32,
        audio_manager: Option<&crate::audio::AudioManager>,
        // Video info for each layer (for transport controls)
        layer_video_info: &HashMap<u32, LayerVideoInfo>,
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
                        self.render_environment_tab(ui, environment, settings, omt_broadcasting, ndi_broadcasting, texture_sharing_active, api_server_running, effect_registry, &mut actions, bpm_clock, effect_time, audio_manager);
                    }
                    PropertiesTab::Layer => {
                        self.render_layer_tab(ui, layers, effect_registry, &mut actions, bpm_clock, effect_time, audio_manager);
                    }
                    PropertiesTab::Clip => {
                        self.render_clip_tab(ui, layers, effect_registry, &mut actions, bpm_clock, effect_time, audio_manager, layer_video_info);
                    }
                }
            });

        actions
    }

    /// Render the Environment tab (Master Effects only - other settings moved to Preferences)
    fn render_environment_tab(
        &mut self,
        ui: &mut egui::Ui,
        environment: &Environment,
        _settings: &EnvironmentSettings,
        _omt_broadcasting: bool,
        _ndi_broadcasting: bool,
        _texture_sharing_active: bool,
        _api_server_running: bool,
        effect_registry: &EffectRegistry,
        actions: &mut Vec<PropertiesAction>,
        bpm_clock: &crate::effects::BpmClock,
        effect_time: f32,
        audio_manager: Option<&crate::audio::AudioManager>,
    ) {
        ui.heading("Master Effects");
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Effects applied to the entire composition")
                .small()
                .weak(),
        );
        ui.add_space(8.0);

        self.render_effect_stack_generic(ui, EffectContext::Environment, environment.effects(), effect_registry, actions, bpm_clock, effect_time, audio_manager);

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);

        // Info about where other settings moved
        ui.label(
            egui::RichText::new("Other environment settings have moved to Preferences")
                .small()
                .weak(),
        );
        #[cfg(target_os = "macos")]
        ui.label(
            egui::RichText::new("Immersive Server → Preferences (⌘,)")
                .small()
                .weak(),
        );
        #[cfg(not(target_os = "macos"))]
        ui.label(
            egui::RichText::new("Edit → Preferences")
                .small()
                .weak(),
        );
    }

    /// Render the Layer tab
    fn render_layer_tab(
        &mut self,
        ui: &mut egui::Ui,
        layers: &[Layer],
        effect_registry: &EffectRegistry,
        actions: &mut Vec<PropertiesAction>,
        bpm_clock: &crate::effects::BpmClock,
        effect_time: f32,
        audio_manager: Option<&crate::audio::AudioManager>,
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
                if ui.button("Reset to center (0)").clicked() {
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
                if ui.button("Reset to center (0)").clicked() {
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

        // Effects section
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);
        ui.heading("Effects");
        ui.add_space(8.0);

        self.render_effect_stack_generic(ui, EffectContext::Layer { layer_id }, &layer.effects, effect_registry, actions, bpm_clock, effect_time, audio_manager);
    }

    /// Render an effect stack for any context (layer, clip, or environment)
    ///
    /// Clean design with collapsible effect sections and aligned parameter rows.
    /// Supports drag-and-drop reordering within the stack.
    fn render_effect_stack_generic(
        &mut self,
        ui: &mut egui::Ui,
        context: EffectContext,
        effects: &EffectStack,
        effect_registry: &EffectRegistry,
        actions: &mut Vec<PropertiesAction>,
        // Automation evaluation context
        bpm_clock: &crate::effects::BpmClock,
        effect_time: f32,
        audio_manager: Option<&crate::audio::AudioManager>,
    ) {
        // Check what type of drag is happening
        let is_dragging_new_effect = egui::DragAndDrop::payload::<DraggableEffect>(ui.ctx()).is_some();
        let dragged_reorder = egui::DragAndDrop::payload::<DraggedEffectReorder>(ui.ctx());
        let is_dragging_reorder = dragged_reorder.is_some();

        // Visual feedback for drop zone only when dragging a new effect from browser
        let frame = if is_dragging_new_effect {
            egui::Frame::new()
                .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 149, 237)))
                .inner_margin(4.0)
                .corner_radius(4.0)
        } else {
            egui::Frame::NONE
        };

        // Collect reorder actions to emit after the loop (to avoid borrow issues)
        let mut reorder_action: Option<(u32, usize)> = None;

        // Wrap effects list in a drop zone for adding new effects
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

                    // Get source index if we're dragging for reorder
                    let source_idx = dragged_reorder.as_ref().map(|p| p.source_index);

                    // Render each effect as a collapsible section with drag-and-drop
                    for (index, effect) in effects.effects.iter().enumerate() {
                        // Drop zone BEFORE this effect (for reordering)
                        if is_dragging_reorder {
                            // Check if this is a valid drop target
                            let is_valid_drop = source_idx.map(|src| index != src && index != src + 1).unwrap_or(false);

                            if is_valid_drop {
                                // Use dnd_drop_zone for proper drop detection
                                let drop_frame = egui::Frame::NONE;
                                let inner_response = ui.dnd_drop_zone::<DraggedEffectReorder, ()>(drop_frame, |ui| {
                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::vec2(ui.available_width(), 8.0),
                                        egui::Sense::hover(),
                                    );

                                    // Draw indicator line when hovered
                                    if response.hovered() {
                                        let painter = ui.painter();
                                        let line_y = rect.center().y;
                                        painter.line_segment(
                                            [egui::pos2(rect.left(), line_y), egui::pos2(rect.right(), line_y)],
                                            egui::Stroke::new(3.0, egui::Color32::from_rgb(100, 149, 237)),
                                        );
                                    }
                                });

                                // Handle drop
                                if let Some(dropped) = inner_response.1 {
                                    let target_index = if index > dropped.source_index { index - 1 } else { index };
                                    reorder_action = Some((dropped.effect_id, target_index));
                                }
                            }
                        }

                        ui.push_id(effect.id, |ui| {
                            // Header background color - darker for section headers
                            let is_being_dragged = source_idx == Some(index);
                            let header_bg = if is_being_dragged {
                                egui::Color32::from_rgb(60, 80, 100) // Highlight when being dragged
                            } else if effect.bypassed {
                                egui::Color32::from_gray(35)
                            } else {
                                egui::Color32::from_rgb(45, 65, 55) // Subtle green tint when active
                            };

                            // Track button clicks using Cell for interior mutability
                            let delete_clicked = std::cell::Cell::new(false);

                            // Wrap effect header in drag source for reordering
                            let effect_id = effect.id;
                            let drag_id = egui::Id::new(("effect_drag", effect_id));

                            let drag_response = ui.dnd_drag_source(drag_id, DraggedEffectReorder { effect_id, source_index: index }, |ui| {
                                // Effect header with collapsible state
                                let header_response = egui::Frame::new()
                                    .fill(header_bg)
                                    .inner_margin(egui::Margin::symmetric(6, 4))
                                    .corner_radius(2.0)
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            // Delete button on the left
                                            let delete_btn = ui.small_button("×").on_hover_text("Remove effect");
                                            if delete_btn.clicked() {
                                                delete_clicked.set(true);
                                            }

                                            // Effect name
                                            let name_color = if effect.bypassed {
                                                egui::Color32::from_gray(100)
                                            } else {
                                                egui::Color32::from_gray(200)
                                            };
                                            let name_text = egui::RichText::new(&effect.name).color(name_color);
                                            ui.label(name_text);

                                            // Indicators on the right
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
                                        })
                                    });

                                header_response
                            });

                            // Handle delete button click (outside drag source closure)
                            if delete_clicked.get() {
                                self.push_remove_action(actions, context, effect.id);
                            }

                            // Handle click on header to toggle expanded state (only if not dragging)
                            if !is_dragging_reorder && !is_dragging_new_effect && !delete_clicked.get() {
                                if drag_response.response.clicked() {
                                    self.push_expanded_action(actions, context, effect.id, !effect.expanded);
                                }
                            }

                            // Right-click context menu for effect controls
                            drag_response.response.context_menu(|ui| {
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
                                    self.render_effect_parameter_generic(ui, context, effect.id, param, actions, bpm_clock, effect_time, audio_manager);
                                }
                            }

                            ui.add_space(2.0);
                        });
                    }

                    // Drop zone AFTER last effect (for reordering to the end)
                    if is_dragging_reorder {
                        let is_valid_drop = source_idx.map(|src| src != effect_count - 1).unwrap_or(false);

                        if is_valid_drop {
                            let drop_frame = egui::Frame::NONE;
                            let inner_response = ui.dnd_drop_zone::<DraggedEffectReorder, ()>(drop_frame, |ui| {
                                let (rect, response) = ui.allocate_exact_size(
                                    egui::vec2(ui.available_width(), 8.0),
                                    egui::Sense::hover(),
                                );

                                // Draw indicator line when hovered
                                if response.hovered() {
                                    let painter = ui.painter();
                                    let line_y = rect.center().y;
                                    painter.line_segment(
                                        [egui::pos2(rect.left(), line_y), egui::pos2(rect.right(), line_y)],
                                        egui::Stroke::new(3.0, egui::Color32::from_rgb(100, 149, 237)),
                                    );
                                }
                            });

                            // Handle drop
                            if let Some(dropped) = inner_response.1 {
                                reorder_action = Some((dropped.effect_id, effect_count - 1));
                            }
                        }
                    }
                }
            })
        });

        // Emit reorder action if a drop occurred
        if let Some((effect_id, new_index)) = reorder_action {
            self.push_reorder_action(actions, context, effect_id, new_index);
        }

        // Handle dropped new effect from browser
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
    /// Layout: [Gear icon] [Label (right-aligned, fixed width)] [Value] [Slider]
    /// When automation is active, shows real-time modulated value marker on slider.
    fn render_effect_parameter_generic(
        &mut self,
        ui: &mut egui::Ui,
        context: EffectContext,
        effect_id: u32,
        param: &crate::effects::Parameter,
        actions: &mut Vec<PropertiesAction>,
        bpm_clock: &crate::effects::BpmClock,
        effect_time: f32,
        audio_manager: Option<&crate::audio::AudioManager>,
    ) {
        const GEAR_WIDTH: f32 = 20.0;
        const LABEL_WIDTH: f32 = 76.0; // Reduced from 80 to accommodate gear

        ui.horizontal(|ui| {
            // Only show gear icon for automatable parameters
            if param.meta.automatable {
                // Gear icon for modulation
                let gear_color = match &param.automation {
                    Some(AutomationSource::Lfo(_)) => egui::Color32::from_rgb(255, 200, 50), // Gold for LFO
                    Some(AutomationSource::Beat(_)) => egui::Color32::from_rgb(50, 200, 255), // Cyan for Beat
                    Some(AutomationSource::Fft(_)) => egui::Color32::from_rgb(255, 80, 200), // Magenta for FFT
                    None => egui::Color32::from_gray(100), // Gray when inactive
                };

                // Create a unique popup ID for this parameter
                let popup_id = ui.make_persistent_id(format!("mod_popup_{}_{}", effect_id, &param.meta.name));

                // Gear button with popup
                let gear_response = ui.allocate_ui_with_layout(
                    egui::vec2(GEAR_WIDTH, ui.spacing().interact_size.y),
                    egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                    |ui| {
                        let response = ui.add(
                            egui::Button::new(
                                egui::RichText::new("\u{2699}") // Unicode gear: ⚙
                                    .size(12.0)
                                    .color(gear_color)
                            )
                            .frame(false)
                            .min_size(egui::vec2(16.0, 16.0))
                        );

                        // Hover tooltip
                        let tooltip = match &param.automation {
                            Some(AutomationSource::Lfo(_)) => "LFO modulation active (click to edit)",
                            Some(AutomationSource::Beat(_)) => "Beat modulation active (click to edit)",
                            Some(AutomationSource::Fft(_)) => "FFT modulation active (click to edit)",
                            None => "Click to add modulation",
                        };
                        response.clone().on_hover_text(tooltip);

                        // Left-click to open modulation popup
                        if response.clicked() {
                            ui.memory_mut(|mem| mem.toggle_popup(popup_id));
                        }

                        response
                    },
                ).inner;

                // Render popup below the gear icon
                egui::popup_below_widget(ui, popup_id, &gear_response, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
                    ui.set_min_width(180.0);
                    self.render_modulation_menu(ui, context, effect_id, param, actions);
                });
            } else {
                // Add spacing to align with automatable params
                ui.add_space(GEAR_WIDTH);
            }

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

                    // Value display
                    ui.label(
                        egui::RichText::new(format!("{:.1}", val))
                            .color(egui::Color32::from_gray(200))
                            .monospace()
                    );

                    // Custom slider with visible track
                    let slider_width = ui.available_width() - 20.0; // Leave room for × button
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(slider_width, 18.0),
                        egui::Sense::click_and_drag()
                    );

                    if ui.is_rect_visible(rect) {
                        let painter = ui.painter();

                        // Draw track (the line)
                        let track_y = rect.center().y;
                        let track_left = rect.left() + 6.0;
                        let track_right = rect.right() - 6.0;
                        painter.line_segment(
                            [egui::pos2(track_left, track_y), egui::pos2(track_right, track_y)],
                            egui::Stroke::new(2.0, egui::Color32::from_gray(60))
                        );

                        // Calculate display value (base value + modulation)
                        let display_val = if let Some(automation) = &param.automation {
                            let mod_value = match automation {
                                AutomationSource::Lfo(lfo) => lfo.evaluate(bpm_clock, effect_time),
                                AutomationSource::Beat(_) => 0.0,
                                AutomationSource::Fft(fft) => {
                                    if let Some(am) = audio_manager {
                                        am.get_band_value(fft.band) * fft.gain
                                    } else {
                                        0.0
                                    }
                                }
                            };
                            (val + mod_value * (max - min)).clamp(min, max)
                        } else {
                            val
                        };

                        // Draw filled portion (left of handle)
                        let norm = (display_val - min) / (max - min);
                        let handle_x = track_left + norm * (track_right - track_left);
                        painter.line_segment(
                            [egui::pos2(track_left, track_y), egui::pos2(handle_x, track_y)],
                            egui::Stroke::new(2.0, egui::Color32::from_gray(120))
                        );

                        // Draw handle (circle)
                        let handle_color = if response.dragged() || response.hovered() {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::from_gray(200)
                        };
                        painter.circle_filled(egui::pos2(handle_x, track_y), 6.0, handle_color);
                        painter.circle_stroke(egui::pos2(handle_x, track_y), 6.0, egui::Stroke::new(1.0, egui::Color32::from_gray(80)));
                    }

                    // Handle drag interaction
                    if response.dragged() {
                        if let Some(pos) = response.interact_pointer_pos() {
                            let track_left = rect.left() + 6.0;
                            let track_right = rect.right() - 6.0;
                            let norm = ((pos.x - track_left) / (track_right - track_left)).clamp(0.0, 1.0);
                            val = min + norm * (max - min);
                            self.push_param_action(actions, context, effect_id, param.meta.name.clone(), ParameterValue::Float(val));
                        }
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

        // Inline modulation controls when automation is active
        self.render_inline_modulation_controls(ui, context, effect_id, param, actions, GEAR_WIDTH + LABEL_WIDTH + 8.0);
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

    fn push_automation_action(&self, actions: &mut Vec<PropertiesAction>, context: EffectContext, effect_id: u32, param_name: String, automation: Option<AutomationSource>) {
        match context {
            EffectContext::Layer { layer_id } => {
                actions.push(PropertiesAction::SetLayerEffectParameterAutomation { layer_id, effect_id, param_name, automation });
            }
            EffectContext::Clip { layer_id, slot } => {
                actions.push(PropertiesAction::SetClipEffectParameterAutomation { layer_id, slot, effect_id, param_name, automation });
            }
            EffectContext::Environment => {
                actions.push(PropertiesAction::SetEnvironmentEffectParameterAutomation { effect_id, param_name, automation });
            }
        }
    }

    /// Render the modulation menu for a parameter
    fn render_modulation_menu(
        &mut self,
        ui: &mut egui::Ui,
        context: EffectContext,
        effect_id: u32,
        param: &crate::effects::Parameter,
        actions: &mut Vec<PropertiesAction>,
    ) {
        ui.set_min_width(120.0);

        // Simple type selection - no presets
        // Active type shown with checkmark, clicking switches type or clears

        let is_lfo = matches!(&param.automation, Some(AutomationSource::Lfo(_)));
        let is_beat = matches!(&param.automation, Some(AutomationSource::Beat(_)));
        let is_fft = matches!(&param.automation, Some(AutomationSource::Fft(_)));

        // None option
        if ui.selectable_label(param.automation.is_none(), "None").clicked() {
            self.push_automation_action(actions, context, effect_id, param.meta.name.clone(), None);
        }

        ui.separator();

        // LFO option
        let lfo_label = egui::RichText::new("LFO").color(egui::Color32::from_rgb(255, 200, 50));
        if ui.selectable_label(is_lfo, lfo_label).clicked() {
            if !is_lfo {
                self.push_automation_action(actions, context, effect_id, param.meta.name.clone(), Some(AutomationSource::Lfo(LfoSource::default())));
            }
        }

        // Beat option
        let beat_label = egui::RichText::new("Beat").color(egui::Color32::from_rgb(50, 200, 255));
        if ui.selectable_label(is_beat, beat_label).clicked() {
            if !is_beat {
                self.push_automation_action(actions, context, effect_id, param.meta.name.clone(), Some(AutomationSource::Beat(BeatSource::default())));
            }
        }

        // FFT option
        let fft_label = egui::RichText::new("FFT").color(egui::Color32::from_rgb(255, 80, 200));
        if ui.selectable_label(is_fft, fft_label).clicked() {
            if !is_fft {
                self.push_automation_action(actions, context, effect_id, param.meta.name.clone(), Some(AutomationSource::Fft(FftSource::default())));
            }
        }
    }

    /// Render inline modulation controls below a parameter slider
    fn render_inline_modulation_controls(
        &mut self,
        ui: &mut egui::Ui,
        context: EffectContext,
        effect_id: u32,
        param: &crate::effects::Parameter,
        actions: &mut Vec<PropertiesAction>,
        indent: f32,
    ) {
        let Some(automation) = &param.automation else { return };

        match automation {
            AutomationSource::Lfo(lfo) => {
                let mut lfo = lfo.clone();
                let default_lfo = LfoSource::default();
                let mut changed = false;

                // Row 1: Shape, Sync, Rate
                ui.horizontal(|ui| {
                    ui.add_space(indent);
                    ui.label(egui::RichText::new("LFO").small().color(egui::Color32::from_rgb(255, 200, 50)));
                    ui.separator();

                    // Shape
                    let shape_response = egui::ComboBox::from_id_salt(format!("lfo_shape_{}_{}", effect_id, &param.meta.name))
                        .selected_text(lfo.shape.name())
                        .width(70.0)
                        .show_ui(ui, |ui| {
                            for shape in LfoShape::all() {
                                if ui.selectable_value(&mut lfo.shape, *shape, shape.name()).changed() {
                                    changed = true;
                                }
                            }
                        });
                    shape_response.response.context_menu(|ui| {
                        if ui.button("Reset to Default").clicked() {
                            lfo.shape = default_lfo.shape;
                            changed = true;
                            ui.close_menu();
                        }
                    });

                    ui.separator();

                    // Sync checkbox
                    let sync_response = ui.checkbox(&mut lfo.sync_to_bpm, "Sync");
                    if sync_response.changed() {
                        changed = true;
                    }
                    sync_response.context_menu(|ui| {
                        if ui.button("Reset to Default").clicked() {
                            lfo.sync_to_bpm = default_lfo.sync_to_bpm;
                            changed = true;
                            ui.close_menu();
                        }
                    });

                    ui.separator();

                    // Rate
                    if lfo.sync_to_bpm {
                        ui.label(egui::RichText::new("Rate:").small());
                        let rate_response = ui.add(egui::DragValue::new(&mut lfo.beats).speed(0.1).range(0.25..=16.0).suffix("b"));
                        if rate_response.changed() {
                            changed = true;
                        }
                        rate_response.context_menu(|ui| {
                            if ui.button("Reset to Default").clicked() {
                                lfo.beats = default_lfo.beats;
                                changed = true;
                                ui.close_menu();
                            }
                        });
                    } else {
                        ui.label(egui::RichText::new("Rate:").small());
                        let rate_response = ui.add(egui::DragValue::new(&mut lfo.frequency).speed(0.01).range(0.01..=20.0).suffix("Hz"));
                        if rate_response.changed() {
                            changed = true;
                        }
                        rate_response.context_menu(|ui| {
                            if ui.button("Reset to Default").clicked() {
                                lfo.frequency = default_lfo.frequency;
                                changed = true;
                                ui.close_menu();
                            }
                        });
                    }

                    ui.separator();

                    // Depth
                    ui.label(egui::RichText::new("Depth:").small());
                    let depth_response = ui.add(egui::DragValue::new(&mut lfo.amplitude).speed(0.01).range(0.0..=1.0));
                    if depth_response.changed() {
                        changed = true;
                    }
                    depth_response.context_menu(|ui| {
                        if ui.button("Reset to Default").clicked() {
                            lfo.amplitude = default_lfo.amplitude;
                            changed = true;
                            ui.close_menu();
                        }
                    });

                    ui.separator();

                    // Phase
                    ui.label(egui::RichText::new("Phase:").small());
                    let phase_response = ui.add(egui::DragValue::new(&mut lfo.phase).speed(0.01).range(0.0..=1.0));
                    if phase_response.changed() {
                        changed = true;
                    }
                    phase_response.context_menu(|ui| {
                        if ui.button("Reset to Default").clicked() {
                            lfo.phase = default_lfo.phase;
                            changed = true;
                            ui.close_menu();
                        }
                    });

                });

                if changed {
                    self.push_automation_action(actions, context, effect_id, param.meta.name.clone(), Some(AutomationSource::Lfo(lfo)));
                }
            }

            AutomationSource::Beat(beat) => {
                let mut beat = beat.clone();
                let default_beat = BeatSource::default();
                let mut changed = false;

                ui.horizontal(|ui| {
                    ui.add_space(indent);
                    ui.label(egui::RichText::new("Beat").small().color(egui::Color32::from_rgb(50, 200, 255)));
                    ui.separator();

                    // Trigger
                    ui.label(egui::RichText::new("@").small());
                    let trigger_response = egui::ComboBox::from_id_salt(format!("beat_trigger_{}_{}", effect_id, &param.meta.name))
                        .selected_text(beat.trigger_on.name())
                        .width(60.0)
                        .show_ui(ui, |ui| {
                            for trigger in BeatTrigger::all() {
                                if ui.selectable_value(&mut beat.trigger_on, *trigger, trigger.name()).changed() {
                                    changed = true;
                                }
                            }
                        });
                    trigger_response.response.context_menu(|ui| {
                        if ui.button("Reset to Default").clicked() {
                            beat.trigger_on = default_beat.trigger_on;
                            changed = true;
                            ui.close_menu();
                        }
                    });

                    ui.separator();

                    // Attack
                    ui.label(egui::RichText::new("Atk:").small());
                    let atk_response = ui.add(egui::DragValue::new(&mut beat.attack_ms).speed(1.0).range(0.0..=500.0).suffix("ms"));
                    if atk_response.changed() {
                        changed = true;
                    }
                    atk_response.context_menu(|ui| {
                        if ui.button("Reset to Default").clicked() {
                            beat.attack_ms = default_beat.attack_ms;
                            changed = true;
                            ui.close_menu();
                        }
                    });

                    ui.separator();

                    // Decay
                    ui.label(egui::RichText::new("Dec:").small());
                    let dec_response = ui.add(egui::DragValue::new(&mut beat.decay_ms).speed(10.0).range(0.0..=2000.0).suffix("ms"));
                    if dec_response.changed() {
                        changed = true;
                    }
                    dec_response.context_menu(|ui| {
                        if ui.button("Reset to Default").clicked() {
                            beat.decay_ms = default_beat.decay_ms;
                            changed = true;
                            ui.close_menu();
                        }
                    });

                    ui.separator();

                    // Sustain
                    ui.label(egui::RichText::new("Sus:").small());
                    let sus_response = ui.add(egui::DragValue::new(&mut beat.sustain).speed(0.01).range(0.0..=1.0));
                    if sus_response.changed() {
                        changed = true;
                    }
                    sus_response.context_menu(|ui| {
                        if ui.button("Reset to Default").clicked() {
                            beat.sustain = default_beat.sustain;
                            changed = true;
                            ui.close_menu();
                        }
                    });

                    ui.separator();

                    // Release
                    ui.label(egui::RichText::new("Rel:").small());
                    let rel_response = ui.add(egui::DragValue::new(&mut beat.release_ms).speed(10.0).range(0.0..=2000.0).suffix("ms"));
                    if rel_response.changed() {
                        changed = true;
                    }
                    rel_response.context_menu(|ui| {
                        if ui.button("Reset to Default").clicked() {
                            beat.release_ms = default_beat.release_ms;
                            changed = true;
                            ui.close_menu();
                        }
                    });

                });

                if changed {
                    self.push_automation_action(actions, context, effect_id, param.meta.name.clone(), Some(AutomationSource::Beat(beat)));
                }
            }

            AutomationSource::Fft(fft) => {
                let mut fft = fft.clone();
                let default_fft = FftSource::default();
                let mut changed = false;

                // Row 1: FFT label, Band, Gain, Smoothing
                ui.horizontal(|ui| {
                    ui.add_space(indent);
                    ui.label(egui::RichText::new("FFT").small().color(egui::Color32::from_rgb(255, 80, 200)));
                    ui.separator();

                    // Band
                    let band_response = egui::ComboBox::from_id_salt(format!("fft_band_{}_{}", effect_id, &param.meta.name))
                        .selected_text(match fft.band {
                            AudioBand::Low => "Low",
                            AudioBand::Mid => "Mid",
                            AudioBand::High => "High",
                            AudioBand::Full => "Full",
                        })
                        .width(50.0)
                        .show_ui(ui, |ui| {
                            for (band, name) in [
                                (AudioBand::Low, "Low"),
                                (AudioBand::Mid, "Mid"),
                                (AudioBand::High, "High"),
                                (AudioBand::Full, "Full"),
                            ] {
                                if ui.selectable_value(&mut fft.band, band, name).changed() {
                                    changed = true;
                                }
                            }
                        });
                    band_response.response.context_menu(|ui| {
                        if ui.button("Reset to Default").clicked() {
                            fft.band = default_fft.band;
                            changed = true;
                            ui.close_menu();
                        }
                    });

                    ui.separator();

                    // Gain
                    ui.label(egui::RichText::new("Gain:").small());
                    let gain_response = ui.add(egui::DragValue::new(&mut fft.gain).speed(0.01).range(0.0..=4.0));
                    if gain_response.changed() {
                        changed = true;
                    }
                    gain_response.context_menu(|ui| {
                        if ui.button("Reset to Default").clicked() {
                            fft.gain = default_fft.gain;
                            changed = true;
                            ui.close_menu();
                        }
                    });

                    ui.separator();

                    // Smoothing
                    ui.label(egui::RichText::new("Smooth:").small());
                    let smooth_response = ui.add(egui::DragValue::new(&mut fft.smoothing).speed(0.01).range(0.0..=1.0));
                    if smooth_response.changed() {
                        changed = true;
                    }
                    smooth_response.context_menu(|ui| {
                        if ui.button("Reset to Default").clicked() {
                            fft.smoothing = default_fft.smoothing;
                            changed = true;
                            ui.close_menu();
                        }
                    });

                });

                // Row 2: Attack and Release (envelope controls)
                ui.horizontal(|ui| {
                    ui.add_space(indent + 40.0);

                    // Attack
                    ui.label(egui::RichText::new("Atk:").small());
                    let atk_response = ui.add(egui::DragValue::new(&mut fft.attack_ms).speed(1.0).range(0.0..=500.0).suffix("ms"));
                    if atk_response.changed() {
                        changed = true;
                    }
                    atk_response.context_menu(|ui| {
                        if ui.button("Reset to Default").clicked() {
                            fft.attack_ms = default_fft.attack_ms;
                            changed = true;
                            ui.close_menu();
                        }
                    });

                    ui.separator();

                    // Release
                    ui.label(egui::RichText::new("Rel:").small());
                    let rel_response = ui.add(egui::DragValue::new(&mut fft.release_ms).speed(10.0).range(0.0..=2000.0).suffix("ms"));
                    if rel_response.changed() {
                        changed = true;
                    }
                    rel_response.context_menu(|ui| {
                        if ui.button("Reset to Default").clicked() {
                            fft.release_ms = default_fft.release_ms;
                            changed = true;
                            ui.close_menu();
                        }
                    });
                });

                if changed {
                    self.push_automation_action(actions, context, effect_id, param.meta.name.clone(), Some(AutomationSource::Fft(fft)));
                }
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
        // Automation evaluation context
        bpm_clock: &crate::effects::BpmClock,
        effect_time: f32,
        audio_manager: Option<&crate::audio::AudioManager>,
        // Video info for transport controls
        layer_video_info: &HashMap<u32, LayerVideoInfo>,
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
        let path_str = match &clip.source {
            ClipSource::File { path } => path.display().to_string(),
            ClipSource::Omt { address, name } => format!("omt://{}/{}", address, name),
            ClipSource::Ndi { ndi_name, url_address } => {
                match url_address {
                    Some(addr) => format!("ndi://{} ({})", ndi_name, addr),
                    None => format!("ndi://{}", ndi_name),
                }
            }
        };
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

        // Transport controls
        ui.add_space(8.0);
        ui.heading("Transport");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            // Preview button - works for any selected clip
            if ui
                .button("👁 Preview")
                .on_hover_text("Preview this clip in the Preview Monitor")
                .clicked()
            {
                actions.push(PropertiesAction::PreviewClip { layer_id, slot });
            }

            ui.add_space(8.0);

            // These controls only work if this clip is currently playing
            ui.add_enabled_ui(is_playing, |ui| {
                // Restart button
                if ui
                    .button("⏮")
                    .on_hover_text("Restart clip from beginning")
                    .clicked()
                {
                    actions.push(PropertiesAction::RestartClip { layer_id });
                }

                // Play/Pause button - reactive based on current state
                let is_paused = layer_video_info
                    .get(&layer_id)
                    .map(|info| info.is_paused)
                    .unwrap_or(false);
                let (icon, tooltip) = if is_paused {
                    ("▶", "Resume playback")
                } else {
                    ("⏸", "Pause playback")
                };
                if ui
                    .button(icon)
                    .on_hover_text(tooltip)
                    .clicked()
                {
                    actions.push(PropertiesAction::ToggleClipPlayback { layer_id });
                }
            });
        });

        // Timeline scrubber (only when playing and has video info)
        if let Some(info) = layer_video_info.get(&layer_id) {
            ui.add_space(8.0);

            // Get or create scrubber state for this layer
            let scrubber_state = self.scrubber_states.entry(layer_id).or_insert_with(ScrubberState::new);

            let (scrub_actions, _display_pos) = video_scrubber(
                ui,
                scrubber_state,
                info.position,
                info.duration,
            );

            // Convert scrubber actions to properties actions
            for action in scrub_actions {
                match action {
                    ScrubberAction::StartScrub => {
                        actions.push(PropertiesAction::StartScrub { layer_id });
                    }
                    ScrubberAction::Seek { time_secs } => {
                        actions.push(PropertiesAction::SeekClip { layer_id, time_secs });
                    }
                    ScrubberAction::EndScrub { time_secs } => {
                        actions.push(PropertiesAction::EndScrub { layer_id, time_secs });
                    }
                }
            }

            ui.add_space(4.0);

            // Video info line
            ui.label(
                egui::RichText::new(format!(
                    "{}x{} @ {:.1}fps",
                    info.width, info.height, info.frame_rate
                ))
                .small()
                .weak(),
            );
        } else if !is_playing {
            ui.label(
                egui::RichText::new("Playback controls available when clip is playing")
                    .small()
                    .weak(),
            );
        }

        // Loop mode dropdown
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Loop Mode:");
            let mut current_mode = clip.loop_mode;
            egui::ComboBox::from_id_salt(format!("loop_mode_{}", slot))
                .selected_text(match current_mode {
                    LoopMode::Loop => "Loop",
                    LoopMode::PlayOnce => "Play Once",
                })
                .width(100.0)
                .show_ui(ui, |ui| {
                    if ui.selectable_value(&mut current_mode, LoopMode::Loop, "Loop").changed() {
                        actions.push(PropertiesAction::SetClipLoopMode { layer_id, slot, mode: LoopMode::Loop });
                    }
                    if ui.selectable_value(&mut current_mode, LoopMode::PlayOnce, "Play Once").changed() {
                        actions.push(PropertiesAction::SetClipLoopMode { layer_id, slot, mode: LoopMode::PlayOnce });
                    }
                });
        });

        // ========== CLIP TRANSFORM ==========
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);
        ui.heading("Clip Transform");
        ui.add_space(8.0);

        // Position
        let mut pos_x = clip.transform.position.0;
        let mut pos_y = clip.transform.position.1;
        ui.horizontal(|ui| {
            ui.label("Position X:");
            let response = ui.add(egui::DragValue::new(&mut pos_x).speed(1.0));
            if response.changed() {
                actions.push(PropertiesAction::SetClipPosition {
                    layer_id,
                    slot,
                    x: pos_x,
                    y: pos_y,
                });
            }
            response.context_menu(|ui| {
                if ui.button("Reset to center (0)").clicked() {
                    actions.push(PropertiesAction::SetClipPosition { layer_id, slot, x: 0.0, y: pos_y });
                    ui.close_menu();
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label("Position Y:");
            let response = ui.add(egui::DragValue::new(&mut pos_y).speed(1.0));
            if response.changed() {
                actions.push(PropertiesAction::SetClipPosition {
                    layer_id,
                    slot,
                    x: pos_x,
                    y: pos_y,
                });
            }
            response.context_menu(|ui| {
                if ui.button("Reset to center (0)").clicked() {
                    actions.push(PropertiesAction::SetClipPosition { layer_id, slot, x: pos_x, y: 0.0 });
                    ui.close_menu();
                }
            });
        });

        ui.add_space(4.0);

        // Scale
        let mut scale_x = clip.transform.scale.0 * 100.0;
        let mut scale_y = clip.transform.scale.1 * 100.0;
        let mut uniform_scale = scale_x;
        ui.horizontal(|ui| {
            ui.label("Scale:");
            let response_uniform = ui.add(
                egui::DragValue::new(&mut uniform_scale)
                    .speed(1.0)
                    .suffix("%")
                    .range(1.0..=1000.0),
            );
            if response_uniform.changed() {
                actions.push(PropertiesAction::SetClipScale {
                    layer_id,
                    slot,
                    scale_x: uniform_scale / 100.0,
                    scale_y: uniform_scale / 100.0,
                });
            }
            response_uniform.context_menu(|ui| {
                if ui.button("Reset to 100%").clicked() {
                    actions.push(PropertiesAction::SetClipScale { layer_id, slot, scale_x: 1.0, scale_y: 1.0 });
                    ui.close_menu();
                }
            });
            ui.add_space(8.0);
            let response_x = ui.add(
                egui::DragValue::new(&mut scale_x)
                    .speed(1.0)
                    .suffix("%")
                    .range(1.0..=1000.0),
            );
            if response_x.changed() {
                actions.push(PropertiesAction::SetClipScale {
                    layer_id,
                    slot,
                    scale_x: scale_x / 100.0,
                    scale_y: scale_y / 100.0,
                });
            }
            ui.label("×");
            let response_y = ui.add(
                egui::DragValue::new(&mut scale_y)
                    .speed(1.0)
                    .suffix("%")
                    .range(1.0..=1000.0),
            );
            if response_y.changed() {
                actions.push(PropertiesAction::SetClipScale {
                    layer_id,
                    slot,
                    scale_x: scale_x / 100.0,
                    scale_y: scale_y / 100.0,
                });
            }
        });

        ui.add_space(4.0);

        // Rotation
        let mut rotation_deg = clip.transform.rotation.to_degrees();
        ui.horizontal(|ui| {
            ui.label("Rotation:");
            let response = ui.add(
                egui::DragValue::new(&mut rotation_deg)
                    .speed(1.0)
                    .suffix("°")
                    .range(-360.0..=360.0),
            );
            if response.changed() {
                actions.push(PropertiesAction::SetClipRotation {
                    layer_id,
                    slot,
                    degrees: rotation_deg,
                });
            }
            response.context_menu(|ui| {
                if ui.button("Reset to 0°").clicked() {
                    actions.push(PropertiesAction::SetClipRotation { layer_id, slot, degrees: 0.0 });
                    ui.close_menu();
                }
            });
        });

        // ========== CLIP EFFECTS ==========
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);
        ui.heading("Clip Effects");
        ui.add_space(8.0);

        self.render_effect_stack_generic(ui, EffectContext::Clip { layer_id, slot }, &clip.effects, effect_registry, actions, bpm_clock, effect_time, audio_manager);
    }
}


