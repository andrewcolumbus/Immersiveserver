//! Screen tree panel for hierarchical output configuration
//!
//! Resolume-style tree view showing Screens > Slices > Masks

#![allow(dead_code)]

use crate::output::{OutputDevice, OutputManager, Screen, Slice};
use eframe::egui::{self, Color32, RichText, Rounding, Sense, Stroke, Vec2};

/// Selection state for the tree
#[derive(Debug, Clone, PartialEq)]
pub enum TreeSelection {
    None,
    Screen(usize),
    Slice(usize, usize),
    Mask(usize, usize),
}

impl Default for TreeSelection {
    fn default() -> Self {
        Self::None
    }
}

/// Screen tree panel state
#[derive(Default)]
pub struct ScreenTreePanel {
    /// Currently expanded screens
    expanded_screens: std::collections::HashSet<usize>,
    /// Currently expanded slices (screen_idx, slice_idx)
    expanded_slices: std::collections::HashSet<(usize, usize)>,
    /// Drag state for reordering
    dragging: Option<TreeSelection>,
}

impl ScreenTreePanel {
    pub fn new() -> Self {
        Self {
            expanded_screens: std::collections::HashSet::new(),
            expanded_slices: std::collections::HashSet::new(),
            dragging: None,
        }
    }

    /// Show the screen tree panel
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        output_manager: &mut OutputManager,
        selection: &mut TreeSelection,
    ) {
        // Header with add button
        ui.horizontal(|ui| {
            // Add button with dropdown
            ui.menu_button(RichText::new("+ ").size(16.0).strong(), |ui| {
                if ui.button("Add Screen").clicked() {
                    let position = output_manager.auto_position_screen();
                    let screen = Screen::new_at_position(
                        format!("Screen {}", output_manager.screens.len() + 1),
                        0,
                        (800, 600),
                        position,
                    );
                    output_manager.add_screen(screen);
                    ui.close_menu();
                }
                
                ui.separator();
                
                ui.label("Device Types:");
                
                if ui.button("  Virtual Output").clicked() {
                    let position = output_manager.auto_position_screen();
                    let mut screen = Screen::new_at_position(
                        format!("Screen {}", output_manager.screens.len() + 1),
                        0,
                        (800, 600),
                        position,
                    );
                    screen.device = OutputDevice::new_virtual(800, 600);
                    output_manager.add_screen(screen);
                    ui.close_menu();
                }
                
                if ui.button("  Fullscreen").clicked() {
                    let position = output_manager.auto_position_screen();
                    // Use the first available display (usually the primary)
                    let (display_id, display_name, resolution) = output_manager.displays
                        .first()
                        .map(|d| (d.id, d.name.clone(), d.resolution))
                        .unwrap_or((0, "Display 1".to_string(), (1920, 1080)));
                    
                    let mut screen = Screen::new_at_position(
                        format!("Screen {}", output_manager.screens.len() + 1),
                        display_id,
                        resolution,
                        position,
                    );
                    screen.device = OutputDevice::new_fullscreen(display_id, display_name);
                    output_manager.add_screen(screen);
                    ui.close_menu();
                }
                
                if ui.button("  Aqueduct").clicked() {
                    let position = output_manager.auto_position_screen();
                    let mut screen = Screen::new_at_position(
                        format!("Screen {}", output_manager.screens.len() + 1),
                        0,
                        (1920, 1080),
                        position,
                    );
                    screen.device = OutputDevice::new_aqueduct(
                        format!("Aqueduct {}", output_manager.screens.len() + 1),
                        9000 + output_manager.screens.len() as u16,
                    );
                    output_manager.add_screen(screen);
                    ui.close_menu();
                }
                
                ui.add_enabled_ui(false, |ui| {
                    ui.horizontal(|ui| {
                        let _ = ui.button("  NDI");
                        ui.label(RichText::new("COMING SOON").size(9.0).color(Color32::from_rgb(255, 180, 0)));
                    });
                });
            });
        });

        ui.add_space(4.0);

        // Scrollable tree area
        egui::ScrollArea::vertical()
            .id_source("screen_tree_scroll")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                let mut screens_to_remove = Vec::new();
                let screen_count = output_manager.screens.len();

                for screen_idx in 0..screen_count {
                    let screen = &mut output_manager.screens[screen_idx];
                    let is_selected = matches!(selection, TreeSelection::Screen(idx) if *idx == screen_idx);
                    
                    // Screen row
                    let response = self.show_screen_row(ui, screen, screen_idx, is_selected, selection);
                    
                    if response.delete_clicked {
                        screens_to_remove.push(screen_idx);
                    }
                    
                    // Show slices if expanded
                    if self.expanded_screens.contains(&screen_idx) {
                        self.show_slices(ui, screen, screen_idx, selection);
                    }
                }

                // Remove screens (in reverse order to preserve indices)
                for idx in screens_to_remove.into_iter().rev() {
                    output_manager.screens.remove(idx);
                    if matches!(selection, TreeSelection::Screen(i) if *i == idx) {
                        *selection = TreeSelection::None;
                    }
                }
            });
    }

    /// Show a single screen row
    fn show_screen_row(
        &mut self,
        ui: &mut egui::Ui,
        screen: &mut Screen,
        screen_idx: usize,
        is_selected: bool,
        selection: &mut TreeSelection,
    ) -> ScreenRowResponse {
        let mut response = ScreenRowResponse::default();
        let is_expanded = self.expanded_screens.contains(&screen_idx);

        // Row background
        let row_rect = ui.available_rect_before_wrap();
        let row_height = 44.0;
        let full_row_rect = egui::Rect::from_min_size(
            row_rect.min,
            Vec2::new(ui.available_width(), row_height),
        );

        // Selection/hover background
        let bg_color = if is_selected {
            Color32::from_rgb(74, 157, 91) // Green selection like Resolume
        } else {
            Color32::TRANSPARENT
        };
        
        let row_response = ui.allocate_rect(full_row_rect, Sense::click());
        
        if row_response.hovered() && !is_selected {
            ui.painter().rect_filled(full_row_rect, Rounding::same(4.0), Color32::from_gray(50));
        } else if is_selected {
            ui.painter().rect_filled(full_row_rect, Rounding::same(4.0), bg_color);
        }

        if row_response.clicked() {
            *selection = TreeSelection::Screen(screen_idx);
        }

        // Draw content on top
        let content_rect = full_row_rect.shrink2(Vec2::new(8.0, 4.0));
        let mut cursor_x = content_rect.min.x;

        // Enable checkbox
        let checkbox_size = 16.0;
        let checkbox_rect = egui::Rect::from_min_size(
            egui::pos2(cursor_x, content_rect.center().y - checkbox_size / 2.0),
            Vec2::splat(checkbox_size),
        );
        
        let checkbox_response = ui.allocate_rect(checkbox_rect, Sense::click());
        let checkbox_color = if screen.enabled {
            Color32::from_rgb(74, 157, 91)
        } else {
            Color32::from_gray(60)
        };
        
        ui.painter().rect_filled(checkbox_rect, Rounding::same(3.0), checkbox_color);
        if screen.enabled {
            // Draw checkmark
            let center = checkbox_rect.center();
            ui.painter().line_segment(
                [egui::pos2(center.x - 4.0, center.y), egui::pos2(center.x - 1.0, center.y + 3.0)],
                Stroke::new(2.0, Color32::WHITE),
            );
            ui.painter().line_segment(
                [egui::pos2(center.x - 1.0, center.y + 3.0), egui::pos2(center.x + 4.0, center.y - 3.0)],
                Stroke::new(2.0, Color32::WHITE),
            );
        }
        
        if checkbox_response.clicked() {
            screen.enabled = !screen.enabled;
        }
        
        cursor_x += checkbox_size + 8.0;

        // Expand/collapse arrow
        let arrow_size = 12.0;
        let arrow_rect = egui::Rect::from_min_size(
            egui::pos2(cursor_x, content_rect.center().y - arrow_size / 2.0),
            Vec2::splat(arrow_size),
        );
        
        let arrow_response = ui.allocate_rect(arrow_rect, Sense::click());
        let arrow = if is_expanded { "▼" } else { "▶" };
        ui.painter().text(
            arrow_rect.center(),
            egui::Align2::CENTER_CENTER,
            arrow,
            egui::FontId::proportional(10.0),
            Color32::from_gray(180),
        );
        
        if arrow_response.clicked() {
            if is_expanded {
                self.expanded_screens.remove(&screen_idx);
            } else {
                self.expanded_screens.insert(screen_idx);
            }
        }
        
        cursor_x += arrow_size + 4.0;

        // Screen name and device info
        let text_x = cursor_x;
        let name_color = if is_selected { Color32::WHITE } else { Color32::from_gray(230) };
        
        ui.painter().text(
            egui::pos2(text_x, content_rect.min.y + 6.0),
            egui::Align2::LEFT_TOP,
            &screen.name,
            egui::FontId::proportional(13.0),
            name_color,
        );

        // Device type subtitle
        let device_text = screen.device.to_string();
        let device_color = if is_selected { Color32::from_gray(220) } else { Color32::from_gray(140) };
        
        ui.painter().text(
            egui::pos2(text_x + 12.0, content_rect.min.y + 22.0),
            egui::Align2::LEFT_TOP,
            &device_text,
            egui::FontId::proportional(10.0),
            device_color,
        );

        // Delete button (X)
        let delete_size = 16.0;
        let delete_rect = egui::Rect::from_min_size(
            egui::pos2(content_rect.max.x - delete_size - 4.0, content_rect.center().y - delete_size / 2.0),
            Vec2::splat(delete_size),
        );
        
        let delete_response = ui.allocate_rect(delete_rect, Sense::click());
        
        if delete_response.hovered() {
            ui.painter().rect_filled(delete_rect, Rounding::same(3.0), Color32::from_rgb(180, 60, 60));
        }
        
        ui.painter().text(
            delete_rect.center(),
            egui::Align2::CENTER_CENTER,
            "×",
            egui::FontId::proportional(14.0),
            if delete_response.hovered() { Color32::WHITE } else { Color32::from_gray(120) },
        );
        
        if delete_response.clicked() {
            response.delete_clicked = true;
        }

        // Add spacing after the row
        ui.add_space(row_height);

        response
    }

    /// Show slices for a screen
    fn show_slices(
        &mut self,
        ui: &mut egui::Ui,
        screen: &mut Screen,
        screen_idx: usize,
        selection: &mut TreeSelection,
    ) {
        let slice_count = screen.slices.len();
        
        // Indented container for slices
        ui.horizontal(|ui| {
            ui.add_space(24.0); // Indent
            ui.vertical(|ui| {
                // Add slice button if no slices
                if slice_count == 0 {
                    if ui.small_button("+ Add Slice").clicked() {
                        let slice = Slice::full_screen(screen.resolution.0, screen.resolution.1);
                        screen.add_slice(slice);
                    }
                }

                for slice_idx in 0..slice_count {
                    let slice = &mut screen.slices[slice_idx];
                    let is_slice_selected = matches!(selection, TreeSelection::Slice(s, sl) if *s == screen_idx && *sl == slice_idx);
                    
                    self.show_slice_row(ui, slice, screen_idx, slice_idx, is_slice_selected, selection);
                    
                    // Show mask if expanded
                    let key = (screen_idx, slice_idx);
                    if self.expanded_slices.contains(&key) && slice.mask.is_some() {
                        self.show_mask_row(ui, screen_idx, slice_idx, selection);
                    }
                }
                
                // Add slice button at the bottom
                if slice_count > 0 {
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        if ui.small_button("+ Slice").clicked() {
                            let slice = Slice::full_screen(screen.resolution.0, screen.resolution.1);
                            screen.add_slice(slice);
                        }
                    });
                }
            });
        });
    }

    /// Show a single slice row
    fn show_slice_row(
        &mut self,
        ui: &mut egui::Ui,
        slice: &mut Slice,
        screen_idx: usize,
        slice_idx: usize,
        is_selected: bool,
        selection: &mut TreeSelection,
    ) {
        let key = (screen_idx, slice_idx);
        let is_expanded = self.expanded_slices.contains(&key);
        let has_mask = slice.mask.is_some();

        let row_height = 28.0;
        let full_row_rect = egui::Rect::from_min_size(
            ui.cursor().min,
            Vec2::new(ui.available_width(), row_height),
        );

        let bg_color = if is_selected {
            Color32::from_rgb(74, 157, 91)
        } else {
            Color32::TRANSPARENT
        };
        
        let row_response = ui.allocate_rect(full_row_rect, Sense::click());
        
        if row_response.hovered() && !is_selected {
            ui.painter().rect_filled(full_row_rect, Rounding::same(3.0), Color32::from_gray(45));
        } else if is_selected {
            ui.painter().rect_filled(full_row_rect, Rounding::same(3.0), bg_color);
        }

        if row_response.clicked() {
            *selection = TreeSelection::Slice(screen_idx, slice_idx);
        }

        let content_rect = full_row_rect.shrink2(Vec2::new(8.0, 4.0));
        let mut cursor_x = content_rect.min.x;

        // Enable checkbox
        let checkbox_size = 14.0;
        let checkbox_rect = egui::Rect::from_min_size(
            egui::pos2(cursor_x, content_rect.center().y - checkbox_size / 2.0),
            Vec2::splat(checkbox_size),
        );
        
        let checkbox_response = ui.allocate_rect(checkbox_rect, Sense::click());
        let checkbox_color = if slice.enabled {
            Color32::from_rgb(74, 157, 91)
        } else {
            Color32::from_gray(50)
        };
        
        ui.painter().rect_filled(checkbox_rect, Rounding::same(2.0), checkbox_color);
        if slice.enabled {
            let center = checkbox_rect.center();
            ui.painter().line_segment(
                [egui::pos2(center.x - 3.0, center.y), egui::pos2(center.x - 0.5, center.y + 2.5)],
                Stroke::new(1.5, Color32::WHITE),
            );
            ui.painter().line_segment(
                [egui::pos2(center.x - 0.5, center.y + 2.5), egui::pos2(center.x + 3.0, center.y - 2.5)],
                Stroke::new(1.5, Color32::WHITE),
            );
        }
        
        if checkbox_response.clicked() {
            slice.enabled = !slice.enabled;
        }
        
        cursor_x += checkbox_size + 6.0;

        // Expand arrow (only if has mask)
        if has_mask {
            let arrow_size = 10.0;
            let arrow_rect = egui::Rect::from_min_size(
                egui::pos2(cursor_x, content_rect.center().y - arrow_size / 2.0),
                Vec2::splat(arrow_size),
            );
            
            let arrow_response = ui.allocate_rect(arrow_rect, Sense::click());
            let arrow = if is_expanded { "▼" } else { "▶" };
            ui.painter().text(
                arrow_rect.center(),
                egui::Align2::CENTER_CENTER,
                arrow,
                egui::FontId::proportional(9.0),
                Color32::from_gray(150),
            );
            
            if arrow_response.clicked() {
                if is_expanded {
                    self.expanded_slices.remove(&key);
                } else {
                    self.expanded_slices.insert(key);
                }
            }
            
            cursor_x += arrow_size + 4.0;
        } else {
            cursor_x += 14.0; // Space for alignment
        }

        // Slice name
        let name_color = if is_selected { Color32::WHITE } else { Color32::from_gray(200) };
        ui.painter().text(
            egui::pos2(cursor_x, content_rect.center().y),
            egui::Align2::LEFT_CENTER,
            &slice.name,
            egui::FontId::proportional(12.0),
            name_color,
        );

        ui.add_space(row_height);
    }

    /// Show mask row for a slice
    fn show_mask_row(
        &mut self,
        ui: &mut egui::Ui,
        screen_idx: usize,
        slice_idx: usize,
        selection: &mut TreeSelection,
    ) {
        let is_selected = matches!(selection, TreeSelection::Mask(s, sl) if *s == screen_idx && *sl == slice_idx);
        
        let row_height = 24.0;
        
        ui.horizontal(|ui| {
            ui.add_space(24.0); // Extra indent for mask
            
            let full_row_rect = egui::Rect::from_min_size(
                ui.cursor().min,
                Vec2::new(ui.available_width(), row_height),
            );

            let bg_color = if is_selected {
                Color32::from_rgb(74, 157, 91)
            } else {
                Color32::TRANSPARENT
            };
            
            let row_response = ui.allocate_rect(full_row_rect, Sense::click());
            
            if row_response.hovered() && !is_selected {
                ui.painter().rect_filled(full_row_rect, Rounding::same(2.0), Color32::from_gray(40));
            } else if is_selected {
                ui.painter().rect_filled(full_row_rect, Rounding::same(2.0), bg_color);
            }

            if row_response.clicked() {
                *selection = TreeSelection::Mask(screen_idx, slice_idx);
            }

            let content_rect = full_row_rect.shrink2(Vec2::new(8.0, 4.0));

            // Mask checkbox and name
            let checkbox_size = 12.0;
            let checkbox_rect = egui::Rect::from_min_size(
                egui::pos2(content_rect.min.x, content_rect.center().y - checkbox_size / 2.0),
                Vec2::splat(checkbox_size),
            );
            
            ui.painter().rect_filled(checkbox_rect, Rounding::same(2.0), Color32::from_rgb(74, 157, 91));
            let center = checkbox_rect.center();
            ui.painter().line_segment(
                [egui::pos2(center.x - 2.5, center.y), egui::pos2(center.x - 0.5, center.y + 2.0)],
                Stroke::new(1.5, Color32::WHITE),
            );
            ui.painter().line_segment(
                [egui::pos2(center.x - 0.5, center.y + 2.0), egui::pos2(center.x + 2.5, center.y - 2.0)],
                Stroke::new(1.5, Color32::WHITE),
            );

            let name_color = if is_selected { Color32::WHITE } else { Color32::from_gray(180) };
            ui.painter().text(
                egui::pos2(content_rect.min.x + checkbox_size + 6.0, content_rect.center().y),
                egui::Align2::LEFT_CENTER,
                "mask",
                egui::FontId::proportional(11.0),
                name_color,
            );

            ui.add_space(row_height);
        });
    }
}

/// Response from showing a screen row
#[derive(Default)]
struct ScreenRowResponse {
    delete_clicked: bool,
}

/// Helper widget showing "COMING SOON" badge
pub fn coming_soon_badge(ui: &mut egui::Ui) {
    let badge_rect = ui.available_rect_before_wrap();
    let badge_size = Vec2::new(70.0, 14.0);
    let badge = egui::Rect::from_min_size(badge_rect.min, badge_size);
    
    ui.painter().rect_filled(
        badge,
        Rounding::same(2.0),
        Color32::from_rgb(200, 120, 0),
    );
    
    ui.painter().text(
        badge.center(),
        egui::Align2::CENTER_CENTER,
        "COMING SOON",
        egui::FontId::proportional(8.0),
        Color32::WHITE,
    );
    
    ui.allocate_rect(badge, Sense::hover());
}

