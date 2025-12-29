//! Screen Manager Panel
//!
//! Simplified panel for managing output screens.

#![allow(dead_code)]

use crate::output::{OutputManager, Screen, Slice};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

/// Screen manager panel for output configuration
pub struct ScreenManagerPanel {
    /// Selected screen index
    pub selected_screen: Option<usize>,
    /// Whether to show the add screen dialog
    show_add_dialog: bool,
    /// New screen name
    new_screen_name: String,
    /// New screen width
    new_screen_width: String,
    /// New screen height
    new_screen_height: String,
}

impl Default for ScreenManagerPanel {
    fn default() -> Self {
        Self {
            selected_screen: None,
            show_add_dialog: false,
            new_screen_name: "New Screen".to_string(),
            new_screen_width: "1920".to_string(),
            new_screen_height: "1080".to_string(),
        }
    }
}

impl ScreenManagerPanel {
    /// Create a new screen manager panel
    pub fn new() -> Self {
        Self::default()
    }

    /// Show the screen manager panel
    pub fn show(&mut self, ui: &mut egui::Ui, output_manager: &mut OutputManager) {
        ui.heading("📺 Screens");

        // Add screen button
        ui.horizontal(|ui| {
            if ui.button("+ Add Screen").clicked() {
                self.show_add_dialog = true;
            }
            if ui.button("🔄 Detect Displays").clicked() {
                output_manager.enumerate_displays();
            }
        });

        // Add screen dialog
        if self.show_add_dialog {
            ui.separator();
            ui.group(|ui| {
                ui.label("New Screen");
                
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut self.new_screen_name);
                });
                
                ui.horizontal(|ui| {
                    ui.label("Width:");
                    ui.add(egui::TextEdit::singleline(&mut self.new_screen_width).desired_width(60.0));
                    ui.label("Height:");
                    ui.add(egui::TextEdit::singleline(&mut self.new_screen_height).desired_width(60.0));
                });
                
                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() {
                        if let (Ok(w), Ok(h)) = (
                            self.new_screen_width.parse::<u32>(),
                            self.new_screen_height.parse::<u32>(),
                        ) {
                            let mut screen = Screen::new(
                                self.new_screen_name.clone(),
                                0,
                                (w, h),
                            );
                            screen.add_slice(Slice::full_screen(w, h));
                            output_manager.add_screen(screen);
                            self.show_add_dialog = false;
                            self.new_screen_name = format!("Screen {}", output_manager.screens.len() + 1);
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_add_dialog = false;
                    }
                });
            });
        }

        ui.separator();

        // Screen list
        if output_manager.screens.is_empty() {
            ui.label("No screens configured");
            ui.label("Click '+ Add Screen' to create one");
        } else {
            egui::ScrollArea::vertical()
                .id_source("screen_manager_scroll")
                .max_height(200.0)
                .show(ui, |ui| {
                    let mut to_remove = None;
                    
                    for (idx, screen) in output_manager.screens.iter().enumerate() {
                        let is_selected = self.selected_screen == Some(idx);
                        
                        ui.horizontal(|ui| {
                            // Selection indicator
                            let response = ui.selectable_label(
                                is_selected,
                                format!("📺 {} ({}x{})", screen.name, screen.resolution.0, screen.resolution.1)
                            );
                            
                            if response.clicked() {
                                self.selected_screen = Some(idx);
                            }
                            
                            // Remove button
                            if ui.small_button("✕").clicked() {
                                to_remove = Some(idx);
                            }
                        });
                    }
                    
                    // Remove screen after iteration
                    if let Some(idx) = to_remove {
                        if idx < output_manager.screens.len() {
                            let id = output_manager.screens[idx].id;
                            output_manager.remove_screen(id);
                            if self.selected_screen == Some(idx) {
                                self.selected_screen = None;
                            }
                        }
                    }
                });
        }

        // Selected screen properties
        if let Some(idx) = self.selected_screen {
            if let Some(screen) = output_manager.screens.get_mut(idx) {
                ui.separator();
                ui.label(format!("Selected: {}", screen.name));
                
                ui.horizontal(|ui| {
                    ui.label("Enabled:");
                    ui.checkbox(&mut screen.enabled, "");
                });
                
                ui.horizontal(|ui| {
                    ui.label("Opacity:");
                    ui.add(egui::Slider::new(&mut screen.opacity, 0.0..=1.0));
                });
                
                // Blend info
                if screen.has_blending() {
                    ui.label("✓ Edge blending configured");
                }
                
                // Quick actions
                ui.horizontal(|ui| {
                    if ui.button("Edit in Advanced Output").clicked() {
                        // This would open the advanced output window
                        // For now just log it
                        log::info!("Opening advanced output for screen {}", screen.name);
                    }
                });
            }
        }
    }

    /// Draw a mini preview of screen layout
    pub fn draw_screen_layout(&self, ui: &mut egui::Ui, output_manager: &OutputManager, max_size: Vec2) {
        if output_manager.screens.is_empty() {
            return;
        }

        // Calculate bounds
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for screen in &output_manager.screens {
            min_x = min_x.min(screen.position.0);
            min_y = min_y.min(screen.position.1);
            max_x = max_x.max(screen.position.0 + screen.resolution.0 as f32);
            max_y = max_y.max(screen.position.1 + screen.resolution.1 as f32);
        }

        let total_width = max_x - min_x;
        let total_height = max_y - min_y;
        
        if total_width <= 0.0 || total_height <= 0.0 {
            return;
        }

        let scale = (max_size.x / total_width).min(max_size.y / total_height);
        let preview_size = Vec2::new(total_width * scale, total_height * scale);

        let (response, painter) = ui.allocate_painter(preview_size, egui::Sense::hover());
        let rect = response.rect;

        // Background
        painter.rect_filled(rect, 4.0, Color32::from_gray(30));

        // Draw each screen
        for (idx, screen) in output_manager.screens.iter().enumerate() {
            let x = rect.min.x + (screen.position.0 - min_x) * scale;
            let y = rect.min.y + (screen.position.1 - min_y) * scale;
            let w = screen.resolution.0 as f32 * scale;
            let h = screen.resolution.1 as f32 * scale;

            let screen_rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
            
            let color = if self.selected_screen == Some(idx) {
                Color32::from_rgb(80, 120, 80)
            } else {
                Color32::from_rgb(60, 60, 80)
            };

            painter.rect_filled(screen_rect, 2.0, color);
            painter.rect_stroke(screen_rect, 2.0, Stroke::new(1.0, Color32::WHITE));

            // Screen number
            painter.text(
                screen_rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{}", idx + 1),
                egui::FontId::proportional(12.0),
                Color32::WHITE,
            );
        }
    }
}

