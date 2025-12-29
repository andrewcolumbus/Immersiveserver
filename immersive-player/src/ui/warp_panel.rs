//! Warp panel for geometric correction configuration
//!
//! Provides UI for configuring perspective and bezier warping.

use crate::output::{BezierWarp, PerspectiveWarp, WarpMode};
use eframe::egui::{self, Color32, Pos2, Stroke, Vec2};
use glam::Vec2 as GlamVec2;

/// Warp panel UI state
#[derive(Default)]
pub struct WarpPanel {
    /// Currently selected corner/point
    selected_point: Option<usize>,
    /// Bezier grid size for new warps
    bezier_grid_size: (u32, u32),
}

impl WarpPanel {
    pub fn new() -> Self {
        Self {
            selected_point: None,
            bezier_grid_size: (4, 4),
        }
    }

    /// Show the warp panel
    pub fn show(&mut self, ui: &mut egui::Ui, warp_mode: &mut WarpMode) {
        ui.heading("Warping");

        // Warp mode selector
        ui.horizontal(|ui| {
            let is_none = matches!(warp_mode, WarpMode::None);
            let is_perspective = matches!(warp_mode, WarpMode::Perspective(_));
            let is_bezier = matches!(warp_mode, WarpMode::Bezier(_));

            if ui.selectable_label(is_none, "None").clicked() && !is_none {
                *warp_mode = WarpMode::None;
            }
            if ui.selectable_label(is_perspective, "Perspective").clicked() && !is_perspective {
                *warp_mode = WarpMode::Perspective(PerspectiveWarp::identity());
            }
            if ui.selectable_label(is_bezier, "Bezier").clicked() && !is_bezier {
                *warp_mode = WarpMode::Bezier(BezierWarp::new(self.bezier_grid_size));
            }
        });

        ui.separator();

        match warp_mode {
            WarpMode::None => {
                ui.label("No warping applied.");
                ui.label("Select Perspective or Bezier to enable warping.");
            }
            WarpMode::Perspective(warp) => {
                self.show_perspective_controls(ui, warp);
            }
            WarpMode::Bezier(warp) => {
                self.show_bezier_controls(ui, warp);
            }
        }
    }

    fn show_perspective_controls(&mut self, ui: &mut egui::Ui, warp: &mut PerspectiveWarp) {
        ui.label("Perspective Warp (4-corner)");
        ui.label("Drag corners in the output canvas to adjust.");
        
        ui.separator();

        // Corner coordinates
        let corner_names = ["Top-Left", "Top-Right", "Bottom-Right", "Bottom-Left"];
        
        egui::Grid::new("perspective_corners")
            .num_columns(3)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                for (i, name) in corner_names.iter().enumerate() {
                    ui.label(*name);
                    
                    let mut x = warp.corners[i].x;
                    let mut y = warp.corners[i].y;
                    
                    ui.add(egui::DragValue::new(&mut x).speed(0.01).prefix("X: "));
                    ui.add(egui::DragValue::new(&mut y).speed(0.01).prefix("Y: "));
                    
                    warp.corners[i] = GlamVec2::new(x, y);
                    ui.end_row();
                }
            });

        ui.separator();

        // Preview
        ui.label("Preview:");
        self.draw_perspective_preview(ui, warp);

        ui.separator();

        if ui.button("Reset to Default").clicked() {
            warp.reset();
        }
    }

    fn show_bezier_controls(&mut self, ui: &mut egui::Ui, warp: &mut BezierWarp) {
        ui.label("Bezier Grid Warp");
        
        ui.horizontal(|ui| {
            ui.label("Grid Size:");
            ui.label(format!("{}×{}", warp.grid_size.0, warp.grid_size.1));
        });

        ui.horizontal(|ui| {
            ui.label("Subdivision:");
            let mut subdiv = warp.subdivision;
            if ui.add(egui::DragValue::new(&mut subdiv).speed(1).clamp_range(1..=32)).changed() {
                warp.subdivision = subdiv;
            }
        });

        ui.separator();

        // New grid size selector
        ui.label("Create New Grid:");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut self.bezier_grid_size.0).speed(1).clamp_range(2..=10).prefix("W: "));
            ui.add(egui::DragValue::new(&mut self.bezier_grid_size.1).speed(1).clamp_range(2..=10).prefix("H: "));
            if ui.button("Apply").clicked() {
                *warp = BezierWarp::new(self.bezier_grid_size);
            }
        });

        ui.separator();

        // Preview
        ui.label("Preview:");
        self.draw_bezier_preview(ui, warp);

        ui.separator();

        if ui.button("Reset to Default").clicked() {
            warp.reset();
        }
    }

    fn draw_perspective_preview(&mut self, ui: &mut egui::Ui, warp: &mut PerspectiveWarp) {
        let size = Vec2::new(ui.available_width().min(200.0), 150.0);
        let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
        let rect = response.rect;

        // Background
        painter.rect_filled(rect, 4.0, Color32::from_gray(30));

        // Draw original rectangle
        let margin = 20.0;
        let inner_rect = rect.shrink(margin);
        painter.rect_stroke(inner_rect, 0.0, Stroke::new(1.0, Color32::from_gray(80)));

        // Draw warped quad
        let corners: Vec<Pos2> = warp.corners.iter().map(|c| {
            Pos2::new(
                inner_rect.min.x + c.x * inner_rect.width(),
                inner_rect.min.y + c.y * inner_rect.height(),
            )
        }).collect();

        // Draw edges
        for i in 0..4 {
            let next = (i + 1) % 4;
            painter.line_segment(
                [corners[i], corners[next]],
                Stroke::new(2.0, Color32::from_rgb(100, 180, 255)),
            );
        }

        // Draw corner handles
        for (i, pos) in corners.iter().enumerate() {
            let is_selected = self.selected_point == Some(i);
            let color = if is_selected {
                Color32::from_rgb(255, 200, 100)
            } else {
                Color32::from_rgb(100, 180, 255)
            };
            
            painter.circle_filled(*pos, 6.0, color);
            painter.circle_stroke(*pos, 6.0, Stroke::new(1.0, Color32::WHITE));
        }

        // Handle interaction
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                // Find closest corner
                let mut closest = 0;
                let mut closest_dist = f32::MAX;
                for (i, corner_pos) in corners.iter().enumerate() {
                    let dist = pos.distance(*corner_pos);
                    if dist < closest_dist {
                        closest_dist = dist;
                        closest = i;
                    }
                }
                if closest_dist < 20.0 {
                    self.selected_point = Some(closest);
                }
            }
        }

        if response.dragged() {
            if let Some(selected) = self.selected_point {
                let delta = response.drag_delta();
                let new_x = warp.corners[selected].x + delta.x / inner_rect.width();
                let new_y = warp.corners[selected].y + delta.y / inner_rect.height();
                warp.corners[selected] = GlamVec2::new(new_x.clamp(0.0, 1.0), new_y.clamp(0.0, 1.0));
            }
        }
    }

    fn draw_bezier_preview(&mut self, ui: &mut egui::Ui, warp: &mut BezierWarp) {
        let size = Vec2::new(ui.available_width().min(200.0), 150.0);
        let (response, painter) = ui.allocate_painter(size, egui::Sense::click_and_drag());
        let rect = response.rect;

        // Background
        painter.rect_filled(rect, 4.0, Color32::from_gray(30));

        let margin = 20.0;
        let inner_rect = rect.shrink(margin);

        // Draw grid lines
        for y in 0..warp.grid_size.1 {
            for x in 0..warp.grid_size.0 {
                let point = warp.get_point(x as usize, y as usize);
                let pos = Pos2::new(
                    inner_rect.min.x + point.x * inner_rect.width(),
                    inner_rect.min.y + point.y * inner_rect.height(),
                );

                // Horizontal line
                if x + 1 < warp.grid_size.0 {
                    let next = warp.get_point(x as usize + 1, y as usize);
                    let next_pos = Pos2::new(
                        inner_rect.min.x + next.x * inner_rect.width(),
                        inner_rect.min.y + next.y * inner_rect.height(),
                    );
                    painter.line_segment([pos, next_pos], Stroke::new(1.0, Color32::from_rgb(80, 140, 200)));
                }

                // Vertical line
                if y + 1 < warp.grid_size.1 {
                    let next = warp.get_point(x as usize, y as usize + 1);
                    let next_pos = Pos2::new(
                        inner_rect.min.x + next.x * inner_rect.width(),
                        inner_rect.min.y + next.y * inner_rect.height(),
                    );
                    painter.line_segment([pos, next_pos], Stroke::new(1.0, Color32::from_rgb(80, 140, 200)));
                }
            }
        }

        // Draw control points
        for y in 0..warp.grid_size.1 {
            for x in 0..warp.grid_size.0 {
                let idx = y as usize * warp.grid_size.0 as usize + x as usize;
                let point = warp.get_point(x as usize, y as usize);
                let pos = Pos2::new(
                    inner_rect.min.x + point.x * inner_rect.width(),
                    inner_rect.min.y + point.y * inner_rect.height(),
                );

                let is_selected = self.selected_point == Some(idx);
                let is_corner = (x == 0 || x == warp.grid_size.0 - 1) && (y == 0 || y == warp.grid_size.1 - 1);
                
                let color = if is_selected {
                    Color32::from_rgb(255, 200, 100)
                } else if is_corner {
                    Color32::from_rgb(100, 180, 255)
                } else {
                    Color32::from_rgb(150, 150, 150)
                };

                painter.circle_filled(pos, if is_corner { 5.0 } else { 3.0 }, color);
            }
        }

        // Handle interaction
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let mut closest_idx = 0;
                let mut closest_dist = f32::MAX;
                
                for y in 0..warp.grid_size.1 {
                    for x in 0..warp.grid_size.0 {
                        let idx = y as usize * warp.grid_size.0 as usize + x as usize;
                        let point = warp.get_point(x as usize, y as usize);
                        let point_pos = Pos2::new(
                            inner_rect.min.x + point.x * inner_rect.width(),
                            inner_rect.min.y + point.y * inner_rect.height(),
                        );
                        let dist = pos.distance(point_pos);
                        if dist < closest_dist {
                            closest_dist = dist;
                            closest_idx = idx;
                        }
                    }
                }
                
                if closest_dist < 15.0 {
                    self.selected_point = Some(closest_idx);
                }
            }
        }

        if response.dragged() {
            if let Some(selected) = self.selected_point {
                let x = selected % warp.grid_size.0 as usize;
                let y = selected / warp.grid_size.0 as usize;
                let delta = response.drag_delta();
                warp.move_point(
                    x,
                    y,
                    GlamVec2::new(
                        delta.x / inner_rect.width(),
                        delta.y / inner_rect.height(),
                    ),
                );
            }
        }
    }
}

