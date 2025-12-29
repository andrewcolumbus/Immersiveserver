//! Clip Matrix UI - Resolume-style grid for triggering clips
//!
//! Displays a grid of clip slots organized by layers (rows) and columns (decks).

#![allow(dead_code)]

use crate::composition::{Clip, ClipSlot, Composition, SolidColorClip, TriggerMode};
use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

/// Clip matrix panel showing layers and columns
pub struct ClipMatrix {
    /// Selected layer index
    pub selected_layer: Option<usize>,
    /// Selected column index
    pub selected_column: Option<usize>,
    /// Cell size in pixels
    pub cell_size: f32,
    /// Show clip names
    pub show_names: bool,
    /// Show progress bars
    pub show_progress: bool,
}

impl Default for ClipMatrix {
    fn default() -> Self {
        Self {
            selected_layer: None,
            selected_column: None,
            cell_size: 80.0,
            show_names: true,
            show_progress: true,
        }
    }
}

impl ClipMatrix {
    /// Create a new clip matrix
    pub fn new() -> Self {
        Self::default()
    }

    /// Show the clip matrix panel
    pub fn show(&mut self, ui: &mut Ui, composition: &mut Composition) -> ClipMatrixResponse {
        let mut response = ClipMatrixResponse::default();

        ui.vertical(|ui| {
            // Header with column numbers
            ui.horizontal(|ui| {
                // Spacer for layer controls column
                ui.add_space(100.0);

                for col in 0..composition.columns {
                    let rect = ui.allocate_exact_size(Vec2::new(self.cell_size, 24.0), Sense::hover());
                    ui.painter().text(
                        rect.0.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("D{}", col + 1),
                        egui::FontId::proportional(12.0),
                        Color32::from_gray(150),
                    );
                }
            });

            ui.separator();

            // Layers (from top to bottom, but render top layer first in visual order)
            let layer_count = composition.layers.len();
            for layer_idx in 0..layer_count {
                // Render from top of visual stack (last layer) to bottom (first layer)
                let visual_idx = layer_count - 1 - layer_idx;

                ui.horizontal(|ui| {
                    // Layer info column
                    let layer = &composition.layers[visual_idx];
                    let layer_rect =
                        ui.allocate_exact_size(Vec2::new(100.0, self.cell_size), Sense::click());

                    self.draw_layer_header(ui, layer_rect.0, layer, visual_idx, &mut response);

                    // Clip cells
                    for col in 0..composition.columns {
                        let is_selected = self.selected_layer == Some(visual_idx)
                            && self.selected_column == Some(col);

                        let cell_response = self.draw_clip_cell(
                            ui,
                            composition,
                            visual_idx,
                            col,
                            is_selected,
                        );

                        if cell_response.clicked() {
                            response.clip_triggered = Some((visual_idx, col));
                            self.selected_layer = Some(visual_idx);
                            self.selected_column = Some(col);
                        }

                        if cell_response.secondary_clicked() {
                            response.clip_context_menu = Some((visual_idx, col));
                        }
                    }
                });

                ui.add_space(2.0);
            }
        });

        response
    }

    /// Draw the layer header (name, solo, bypass indicators)
    fn draw_layer_header(
        &self,
        ui: &mut Ui,
        rect: Rect,
        layer: &crate::composition::Layer,
        _layer_idx: usize,
        _response: &mut ClipMatrixResponse,
    ) {
        let painter = ui.painter();

        // Background
        let bg_color = if layer.solo {
            Color32::from_rgb(60, 60, 30)
        } else if layer.bypass {
            Color32::from_rgb(40, 30, 30)
        } else {
            Color32::from_rgb(35, 38, 42)
        };
        painter.rect_filled(rect, 4.0, bg_color);

        // Layer name
        painter.text(
            rect.left_center() + Vec2::new(8.0, 0.0),
            egui::Align2::LEFT_CENTER,
            &layer.name,
            egui::FontId::proportional(11.0),
            if layer.bypass {
                Color32::from_gray(80)
            } else {
                Color32::from_gray(180)
            },
        );

        // Opacity indicator
        let opacity_bar_rect = Rect::from_min_size(
            Pos2::new(rect.right() - 30.0, rect.top() + 4.0),
            Vec2::new(4.0, rect.height() - 8.0),
        );
        painter.rect_filled(opacity_bar_rect, 2.0, Color32::from_gray(50));

        let filled_height = opacity_bar_rect.height() * layer.opacity;
        let filled_rect = Rect::from_min_size(
            Pos2::new(
                opacity_bar_rect.left(),
                opacity_bar_rect.bottom() - filled_height,
            ),
            Vec2::new(4.0, filled_height),
        );
        painter.rect_filled(filled_rect, 2.0, Color32::from_rgb(80, 160, 80));

        // Solo/Bypass indicators
        if layer.solo {
            painter.text(
                Pos2::new(rect.right() - 16.0, rect.center().y - 8.0),
                egui::Align2::CENTER_CENTER,
                "S",
                egui::FontId::proportional(10.0),
                Color32::from_rgb(220, 220, 100),
            );
        }

        if layer.bypass {
            painter.text(
                Pos2::new(rect.right() - 16.0, rect.center().y + 8.0),
                egui::Align2::CENTER_CENTER,
                "B",
                egui::FontId::proportional(10.0),
                Color32::from_rgb(220, 100, 100),
            );
        }
    }

    /// Draw a single clip cell
    fn draw_clip_cell(
        &self,
        ui: &mut Ui,
        composition: &Composition,
        layer_idx: usize,
        col: usize,
        is_selected: bool,
    ) -> Response {
        let size = Vec2::new(self.cell_size, self.cell_size);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());

        let layer = &composition.layers[layer_idx];
        let clip_slot = layer.get_clip(col);
        let is_active = layer.active_column == Some(col);
        let is_playing = is_active && layer.is_playing();

        // Background color based on state
        let bg_color = if is_playing {
            Color32::from_rgb(60, 100, 60)
        } else if is_active {
            Color32::from_rgb(50, 80, 50)
        } else if clip_slot.is_some() {
            Color32::from_rgb(45, 48, 52)
        } else {
            Color32::from_rgb(30, 33, 37)
        };

        ui.painter().rect_filled(rect, 4.0, bg_color);

        // Selection border
        if is_selected {
            ui.painter().rect_stroke(rect, 4.0, Stroke::new(2.0, Color32::from_rgb(100, 150, 255)));
        }

        // Clip content
        if let Some(slot) = clip_slot {
            self.draw_clip_content(ui, rect, slot, is_playing);
        } else {
            // Empty cell indicator
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "+",
                egui::FontId::proportional(20.0),
                Color32::from_gray(60),
            );
        }

        // Hover effect
        if response.hovered() {
            ui.painter().rect_stroke(
                rect.shrink(1.0),
                4.0,
                Stroke::new(1.0, Color32::from_gray(100)),
            );
        }

        response
    }

    /// Draw the clip content (thumbnail, name, progress)
    fn draw_clip_content(&self, ui: &mut Ui, rect: Rect, slot: &ClipSlot, is_playing: bool) {
        let painter = ui.painter();

        // Clip type indicator / thumbnail area
        let thumb_rect = Rect::from_min_size(rect.min + Vec2::new(4.0, 4.0), Vec2::new(rect.width() - 8.0, rect.height() - 24.0));

        let (icon, color) = match &slot.clip {
            Clip::Video(_) => ("🎬", Color32::from_rgb(80, 120, 180)),
            Clip::Image(_) => ("🖼", Color32::from_rgb(180, 120, 80)),
            Clip::SolidColor(solid) => {
                // Show color preview
                let color = Color32::from_rgba_unmultiplied(
                    (solid.color[0] * 255.0) as u8,
                    (solid.color[1] * 255.0) as u8,
                    (solid.color[2] * 255.0) as u8,
                    (solid.color[3] * 255.0) as u8,
                );
                painter.rect_filled(thumb_rect, 2.0, color);
                ("", Color32::TRANSPARENT)
            }
            Clip::Generator(gen) => {
                let icon = match &gen.generator_type {
                    crate::composition::GeneratorType::Noise { .. } => "◌",
                    crate::composition::GeneratorType::Gradient { .. } => "▤",
                    crate::composition::GeneratorType::TestPattern(_) => "▦",
                    crate::composition::GeneratorType::Plasma { .. } => "◉",
                    crate::composition::GeneratorType::ColorBars => "▥",
                };
                (icon, Color32::from_rgb(120, 180, 120))
            }
        };

        if !icon.is_empty() {
            painter.rect_filled(thumb_rect, 2.0, Color32::from_gray(40));
            painter.text(
                thumb_rect.center(),
                egui::Align2::CENTER_CENTER,
                icon,
                egui::FontId::proportional(24.0),
                color,
            );
        }

        // Clip name
        if self.show_names {
            let name = slot.name();
            let name_short = if name.len() > 10 {
                format!("{}…", &name[..9])
            } else {
                name
            };

            painter.text(
                Pos2::new(rect.center().x, rect.bottom() - 12.0),
                egui::Align2::CENTER_CENTER,
                name_short,
                egui::FontId::proportional(9.0),
                Color32::from_gray(160),
            );
        }

        // Progress bar
        if self.show_progress && is_playing {
            let progress = slot.playback.progress() as f32;
            let progress_rect = Rect::from_min_size(
                Pos2::new(rect.left() + 4.0, rect.bottom() - 4.0),
                Vec2::new((rect.width() - 8.0) * progress, 2.0),
            );
            painter.rect_filled(progress_rect, 1.0, Color32::from_rgb(100, 200, 100));
        }

        // Trigger mode indicator
        let mode_char = match slot.trigger_mode {
            TriggerMode::Toggle => "",
            TriggerMode::Flash => "⚡",
            TriggerMode::OneShot => "→",
        };
        if !mode_char.is_empty() {
            painter.text(
                Pos2::new(rect.right() - 8.0, rect.top() + 8.0),
                egui::Align2::CENTER_CENTER,
                mode_char,
                egui::FontId::proportional(8.0),
                Color32::from_gray(120),
            );
        }
    }

    /// Add a test clip to a slot
    pub fn add_test_clip(composition: &mut Composition, layer_idx: usize, col: usize) {
        // Add a random solid color for testing
        let colors = [
            [1.0, 0.0, 0.0, 1.0], // Red
            [0.0, 1.0, 0.0, 1.0], // Green
            [0.0, 0.0, 1.0, 1.0], // Blue
            [1.0, 1.0, 0.0, 1.0], // Yellow
            [1.0, 0.0, 1.0, 1.0], // Magenta
            [0.0, 1.0, 1.0, 1.0], // Cyan
        ];
        let columns = composition.columns;
        let color_idx = (layer_idx * columns + col) % colors.len();
        let clip = Clip::SolidColor(SolidColorClip::new(colors[color_idx]));
        let slot = ClipSlot::new(clip);

        if let Some(layer) = composition.get_layer_by_index_mut(layer_idx) {
            layer.set_clip(col, slot);
        }
    }
}

/// Response from the clip matrix interaction
#[derive(Default)]
pub struct ClipMatrixResponse {
    /// Clip was triggered at (layer, column)
    pub clip_triggered: Option<(usize, usize)>,
    /// Context menu requested at (layer, column)
    pub clip_context_menu: Option<(usize, usize)>,
    /// Layer header was clicked
    pub layer_clicked: Option<usize>,
}

