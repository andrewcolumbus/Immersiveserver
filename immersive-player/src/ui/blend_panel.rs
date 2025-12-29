//! Blend panel for edge blending configuration
//!
//! Provides UI for configuring edge blend parameters.

use super::widgets::{defaults, resettable_drag_value_u32, resettable_slider};
use crate::output::{BlendConfig, BlendPreset, EdgeBlend};
use eframe::egui::{self, Color32, Pos2, Stroke, Vec2};

/// Blend panel UI state
#[derive(Default)]
pub struct BlendPanel {
    /// Currently selected edge
    selected_edge: SelectedEdge,
    /// Preview curve resolution
    curve_resolution: usize,
}

/// Which edge is selected
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum SelectedEdge {
    #[default]
    Left,
    Right,
    Top,
    Bottom,
}

impl BlendPanel {
    pub fn new() -> Self {
        Self {
            selected_edge: SelectedEdge::Left,
            curve_resolution: 100,
        }
    }

    /// Show the blend panel
    pub fn show(&mut self, ui: &mut egui::Ui, config: &mut BlendConfig) {
        ui.heading("Edge Blending");
        
        // Edge selector
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.selected_edge, SelectedEdge::Left, "Left");
            ui.selectable_value(&mut self.selected_edge, SelectedEdge::Right, "Right");
            ui.selectable_value(&mut self.selected_edge, SelectedEdge::Top, "Top");
            ui.selectable_value(&mut self.selected_edge, SelectedEdge::Bottom, "Bottom");
        });

        ui.separator();

        // Get the selected edge blend
        let edge_blend = match self.selected_edge {
            SelectedEdge::Left => &mut config.left,
            SelectedEdge::Right => &mut config.right,
            SelectedEdge::Top => &mut config.top,
            SelectedEdge::Bottom => &mut config.bottom,
        };

        // Enable/disable toggle
        let is_enabled = edge_blend.is_some();
        let mut enabled = is_enabled;
        if ui.checkbox(&mut enabled, "Enable blend").changed() {
            if enabled && edge_blend.is_none() {
                *edge_blend = Some(EdgeBlend::default());
            } else if !enabled {
                *edge_blend = None;
            }
        }

        if let Some(blend) = edge_blend {
            ui.separator();
            
            // Preset buttons
            ui.horizontal(|ui| {
                ui.label("Presets:");
                if ui.button("Linear").clicked() {
                    blend.power = BlendPreset::Linear.power();
                }
                if ui.button("Smooth").clicked() {
                    blend.power = BlendPreset::Smooth.power();
                }
                if ui.button("Aggressive").clicked() {
                    blend.power = BlendPreset::Aggressive.power();
                }
            });

            ui.separator();

            // Parameters (right-click to reset)
            ui.label("💡 Right-click any parameter to reset");
            egui::Grid::new("blend_params")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.label("Width (px):");
                    resettable_drag_value_u32(ui, &mut blend.width, 1.0, 0..=1000, defaults::BLEND_WIDTH);
                    ui.end_row();

                    ui.label("Power:");
                    resettable_slider(ui, &mut blend.power, 0.5..=5.0, defaults::BLEND_POWER);
                    ui.end_row();

                    ui.label("Gamma:");
                    resettable_slider(ui, &mut blend.gamma, 0.5..=3.0, defaults::BLEND_GAMMA);
                    ui.end_row();

                    ui.label("Black Level:");
                    resettable_slider(ui, &mut blend.black_level, 0.0..=0.2, defaults::BLEND_BLACK_LEVEL);
                    ui.end_row();
                });

            ui.separator();

            // Curve preview
            ui.label("Blend Curve Preview:");
            self.draw_curve_preview(ui, blend);
            
            ui.separator();
            
            // Gradient preview
            ui.label("Blend Gradient Preview:");
            self.draw_gradient_preview(ui, blend);
        }
    }

    fn draw_curve_preview(&self, ui: &mut egui::Ui, blend: &EdgeBlend) {
        let size = Vec2::new(ui.available_width(), 100.0);
        let (response, painter) = ui.allocate_painter(size, egui::Sense::hover());
        let rect = response.rect;

        // Background
        painter.rect_filled(rect, 4.0, Color32::from_gray(20));

        // Draw grid
        for i in 1..4 {
            let x = rect.min.x + rect.width() * i as f32 / 4.0;
            let y = rect.max.y - rect.height() * i as f32 / 4.0;
            
            painter.line_segment(
                [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
                Stroke::new(1.0, Color32::from_gray(40)),
            );
            painter.line_segment(
                [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
                Stroke::new(1.0, Color32::from_gray(40)),
            );
        }

        // Draw curve
        let points: Vec<Pos2> = (0..=self.curve_resolution)
            .map(|i| {
                let t = i as f32 / self.curve_resolution as f32;
                let y = blend.blend_factor(t);
                Pos2::new(
                    rect.min.x + t * rect.width(),
                    rect.max.y - y * rect.height(),
                )
            })
            .collect();

        for window in points.windows(2) {
            painter.line_segment(
                [window[0], window[1]],
                Stroke::new(2.0, Color32::from_rgb(100, 180, 255)),
            );
        }

        // Draw linear reference
        painter.line_segment(
            [rect.left_bottom(), rect.right_top()],
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 50)),
        );

        // Labels
        painter.text(
            rect.left_top() + Vec2::new(4.0, 4.0),
            egui::Align2::LEFT_TOP,
            "1.0",
            egui::FontId::proportional(10.0),
            Color32::from_gray(150),
        );
        painter.text(
            rect.left_bottom() + Vec2::new(4.0, -4.0),
            egui::Align2::LEFT_BOTTOM,
            "0.0",
            egui::FontId::proportional(10.0),
            Color32::from_gray(150),
        );
    }

    fn draw_gradient_preview(&self, ui: &mut egui::Ui, blend: &EdgeBlend) {
        let size = Vec2::new(ui.available_width(), 30.0);
        let (response, painter) = ui.allocate_painter(size, egui::Sense::hover());
        let rect = response.rect;

        // Draw gradient
        let steps = rect.width() as usize;
        for i in 0..steps {
            let t = i as f32 / steps as f32;
            let factor = blend.blend_factor(t);
            let gray = (factor * 255.0) as u8;
            
            let x = rect.min.x + i as f32;
            painter.line_segment(
                [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
                Stroke::new(1.0, Color32::from_gray(gray)),
            );
        }

        // Border
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, Color32::from_gray(80)));
    }
}

