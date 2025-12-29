//! Layer controls panel
//!
//! Provides controls for layer opacity, blend mode, solo, and bypass.

#![allow(dead_code)]

use crate::composition::{BlendMode, Composition};
use eframe::egui::{self, Color32, ComboBox, Slider, Ui};

/// Layer controls panel
pub struct LayerControls {
    /// Currently selected layer index
    pub selected_layer: Option<usize>,
}

impl Default for LayerControls {
    fn default() -> Self {
        Self::new()
    }
}

impl LayerControls {
    /// Create a new layer controls panel
    pub fn new() -> Self {
        Self {
            selected_layer: None,
        }
    }

    /// Set the selected layer
    pub fn set_selected(&mut self, layer_idx: Option<usize>) {
        self.selected_layer = layer_idx;
    }

    /// Show the layer controls panel
    pub fn show(&mut self, ui: &mut Ui, composition: &mut Composition) {
        ui.vertical(|ui| {
            ui.heading("Layer Controls");
            ui.separator();

            if let Some(layer_idx) = self.selected_layer {
                if layer_idx < composition.layers.len() {
                    self.show_layer_controls(ui, composition, layer_idx);
                } else {
                    ui.label("Invalid layer selection");
                }
            } else {
                ui.label("Select a layer to edit");
                ui.add_space(10.0);

                // Show all layers summary
                ui.label("Layers:");
                for (i, layer) in composition.layers.iter().enumerate() {
                    let visual_idx = composition.layers.len() - 1 - i;
                    ui.horizontal(|ui| {
                        if ui.selectable_label(false, &layer.name).clicked() {
                            self.selected_layer = Some(visual_idx);
                        }
                        ui.label(format!("{:.0}%", layer.opacity * 100.0));
                    });
                }
            }
        });
    }

    /// Show controls for a specific layer
    fn show_layer_controls(&mut self, ui: &mut Ui, composition: &mut Composition, layer_idx: usize) {
        let layer = &mut composition.layers[layer_idx];

        // Layer name
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut layer.name);
        });

        ui.add_space(8.0);

        // Opacity slider
        ui.horizontal(|ui| {
            ui.label("Opacity:");
            ui.add(Slider::new(&mut layer.opacity, 0.0..=1.0).show_value(true));
        });

        ui.add_space(8.0);

        // Blend mode dropdown
        ui.horizontal(|ui| {
            ui.label("Blend Mode:");
            ComboBox::from_id_source("blend_mode")
                .selected_text(layer.blend_mode.name())
                .show_ui(ui, |ui| {
                    for mode in BlendMode::all() {
                        ui.selectable_value(&mut layer.blend_mode, *mode, mode.name());
                    }
                });
        });

        ui.add_space(8.0);

        // Solo and Bypass toggles
        ui.horizontal(|ui| {
            let solo_color = if layer.solo {
                Color32::from_rgb(220, 220, 100)
            } else {
                Color32::from_gray(120)
            };
            if ui
                .add(egui::Button::new(egui::RichText::new("Solo").color(solo_color)))
                .clicked()
            {
                layer.solo = !layer.solo;
            }

            let bypass_color = if layer.bypass {
                Color32::from_rgb(220, 100, 100)
            } else {
                Color32::from_gray(120)
            };
            if ui
                .add(egui::Button::new(egui::RichText::new("Bypass").color(bypass_color)))
                .clicked()
            {
                layer.bypass = !layer.bypass;
            }
        });

        ui.add_space(8.0);

        // Transform controls
        ui.collapsing("Transform", |ui| {
            ui.horizontal(|ui| {
                ui.label("Position X:");
                ui.add(Slider::new(&mut layer.transform.position.0, -1.0..=1.0));
            });
            ui.horizontal(|ui| {
                ui.label("Position Y:");
                ui.add(Slider::new(&mut layer.transform.position.1, -1.0..=1.0));
            });
            ui.horizontal(|ui| {
                ui.label("Scale X:");
                ui.add(Slider::new(&mut layer.transform.scale.0, 0.1..=3.0));
            });
            ui.horizontal(|ui| {
                ui.label("Scale Y:");
                ui.add(Slider::new(&mut layer.transform.scale.1, 0.1..=3.0));
            });
            ui.horizontal(|ui| {
                ui.label("Rotation:");
                ui.add(Slider::new(&mut layer.transform.rotation, -180.0..=180.0).suffix("°"));
            });

            if ui.button("Reset Transform").clicked() {
                layer.transform.reset();
            }
        });

        ui.add_space(8.0);

        // Active clip info
        if let Some(col) = layer.active_column {
            if let Some(clip) = layer.get_clip(col) {
                ui.separator();
                ui.label(format!("Playing: {}", clip.name()));
                ui.label(format!("Progress: {:.1}%", clip.playback.progress() * 100.0));
            }
        }

        ui.add_space(16.0);

        // Navigation
        if ui.button("← Back to all layers").clicked() {
            self.selected_layer = None;
        }
    }

    /// Show master controls
    pub fn show_master_controls(&mut self, ui: &mut Ui, composition: &mut Composition) {
        ui.horizontal(|ui| {
            ui.label("Master:");
            ui.add(Slider::new(&mut composition.master_opacity, 0.0..=1.0).show_value(false));
            ui.label(format!("{:.0}%", composition.master_opacity * 100.0));

            ui.separator();

            ui.label("Speed:");
            ui.add(Slider::new(&mut composition.master_speed, 0.0..=2.0).show_value(false));
            ui.label(format!("{:.1}x", composition.master_speed));
        });
    }
}

/// Show a compact layer strip (for use in main window)
pub fn show_layer_strip(ui: &mut Ui, composition: &mut Composition) {
    let layer_count = composition.layers.len();
    ui.horizontal(|ui| {
        for (i, layer) in composition.layers.iter_mut().enumerate() {
            let visual_idx = layer_count - 1 - i;
            let is_playing = layer.is_playing();

            let color = if layer.bypass {
                Color32::from_gray(60)
            } else if is_playing {
                Color32::from_rgb(80, 160, 80)
            } else {
                Color32::from_gray(100)
            };

            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(format!("L{}", visual_idx + 1))
                        .size(10.0)
                        .color(color),
                );

                // Mini opacity slider
                let response = ui.add(
                    Slider::new(&mut layer.opacity, 0.0..=1.0)
                        .show_value(false)
                        .vertical(),
                );
                if response.changed() {
                    log::debug!("Layer {} opacity: {:.2}", visual_idx, layer.opacity);
                }
            });
        }
    });
}

