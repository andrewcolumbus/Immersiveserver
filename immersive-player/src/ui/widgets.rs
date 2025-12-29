//! Custom UI widgets with enhanced functionality
//!
//! Provides resettable sliders, drag values, and other enhanced controls.
//! Includes Resolume-style spinboxes and property controls.

#![allow(dead_code)]

use eframe::egui::{self, Color32, Response, RichText, Rounding, Sense, Stroke, Ui, Vec2};
use std::ops::RangeInclusive;

/// A slider that can be reset to its default value via right-click context menu.
pub fn resettable_slider(
    ui: &mut Ui,
    value: &mut f32,
    range: RangeInclusive<f32>,
    default: f32,
) -> Response {
    let response = ui.add(egui::Slider::new(value, range));
    response.context_menu(|ui| {
        if ui.button("⟲ Reset to Default").clicked() {
            *value = default;
            ui.close_menu();
        }
        ui.separator();
        ui.label(format!("Default: {:.2}", default));
    });
    response
}

/// A slider with a label that can be reset to its default value via right-click.
pub fn resettable_slider_with_label(
    ui: &mut Ui,
    label: &str,
    value: &mut f32,
    range: RangeInclusive<f32>,
    default: f32,
) -> Response {
    ui.horizontal(|ui| {
        ui.label(label);
        resettable_slider(ui, value, range, default)
    })
    .inner
}

/// A drag value that can be reset to its default value via right-click context menu.
pub fn resettable_drag_value(
    ui: &mut Ui,
    value: &mut f32,
    speed: f32,
    range: RangeInclusive<f32>,
    default: f32,
) -> Response {
    let response = ui.add(
        egui::DragValue::new(value)
            .speed(speed)
            .clamp_range(range),
    );
    response.context_menu(|ui| {
        if ui.button("⟲ Reset to Default").clicked() {
            *value = default;
            ui.close_menu();
        }
        ui.separator();
        ui.label(format!("Default: {:.2}", default));
    });
    response
}

/// A drag value for u32 that can be reset to its default value via right-click.
pub fn resettable_drag_value_u32(
    ui: &mut Ui,
    value: &mut u32,
    speed: f32,
    range: RangeInclusive<u32>,
    default: u32,
) -> Response {
    let response = ui.add(
        egui::DragValue::new(value)
            .speed(speed)
            .clamp_range(range),
    );
    response.context_menu(|ui| {
        if ui.button("⟲ Reset to Default").clicked() {
            *value = default;
            ui.close_menu();
        }
        ui.separator();
        ui.label(format!("Default: {}", default));
    });
    response
}

/// A checkbox that can be reset via right-click.
pub fn resettable_checkbox(
    ui: &mut Ui,
    value: &mut bool,
    label: &str,
    default: bool,
) -> Response {
    let response = ui.checkbox(value, label);
    response.context_menu(|ui| {
        if ui.button("⟲ Reset to Default").clicked() {
            *value = default;
            ui.close_menu();
        }
        ui.separator();
        ui.label(format!("Default: {}", if default { "On" } else { "Off" }));
    });
    response
}

/// Default values for screen properties
pub mod defaults {
    pub const OPACITY: f32 = 1.0;
    pub const BRIGHTNESS: f32 = 0.0;
    pub const CONTRAST: f32 = 1.0;
    pub const RGB_CHANNEL: f32 = 1.0;
    pub const DELAY_FRAMES: u32 = 0;
    
    pub const BLEND_WIDTH: u32 = 100;
    pub const BLEND_POWER: f32 = 2.2;
    pub const BLEND_GAMMA: f32 = 1.0;
    pub const BLEND_BLACK_LEVEL: f32 = 0.0;
    
    pub const PLAYBACK_SPEED: f32 = 1.0;
}

/// Resolume-style spinbox with +/- buttons
pub fn spinbox_u32(
    ui: &mut Ui,
    label: &str,
    value: &mut u32,
    min: u32,
    max: u32,
) -> Response {
    ui.horizontal(|ui| {
        ui.label(label);
        
        // Minus button
        if ui.add(
            egui::Button::new(RichText::new("−").size(12.0))
                .min_size(Vec2::new(20.0, 20.0))
                .fill(Color32::from_gray(50))
        ).clicked() {
            *value = value.saturating_sub(1).max(min);
        }
        
        // Value display (editable)
        let response = ui.add(
            egui::DragValue::new(value)
                .clamp_range(min..=max)
                .speed(1.0)
        );
        
        // Plus button
        if ui.add(
            egui::Button::new(RichText::new("+").size(12.0))
                .min_size(Vec2::new(20.0, 20.0))
                .fill(Color32::from_gray(50))
        ).clicked() {
            *value = value.saturating_add(1).min(max);
        }
        
        response
    }).inner
}

/// Resolume-style spinbox for f32 with +/- buttons
pub fn spinbox_f32(
    ui: &mut Ui,
    label: &str,
    value: &mut f32,
    min: f32,
    max: f32,
    step: f32,
) -> Response {
    ui.horizontal(|ui| {
        ui.label(label);
        
        // Minus button
        if ui.add(
            egui::Button::new(RichText::new("−").size(12.0))
                .min_size(Vec2::new(20.0, 20.0))
                .fill(Color32::from_gray(50))
        ).clicked() {
            *value = (*value - step).max(min);
        }
        
        // Value display (editable)
        let response = ui.add(
            egui::DragValue::new(value)
                .clamp_range(min..=max)
                .speed(step * 0.1)
                .fixed_decimals(2)
        );
        
        // Plus button
        if ui.add(
            egui::Button::new(RichText::new("+").size(12.0))
                .min_size(Vec2::new(20.0, 20.0))
                .fill(Color32::from_gray(50))
        ).clicked() {
            *value = (*value + step).min(max);
        }
        
        response
    }).inner
}

/// Resolume-style slider with colored indicator bar
pub fn slider_with_bar(
    ui: &mut Ui,
    value: &mut f32,
    range: RangeInclusive<f32>,
    bar_color: Color32,
) -> Response {
    let available_width = ui.available_width().min(120.0);
    let height = 16.0;
    
    let (rect, response) = ui.allocate_exact_size(Vec2::new(available_width, height), Sense::click_and_drag());
    
    if response.dragged() || response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let t = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
            let min = *range.start();
            let max = *range.end();
            *value = min + t * (max - min);
        }
    }
    
    // Background
    ui.painter().rect_filled(rect, Rounding::same(2.0), Color32::from_gray(40));
    
    // Filled portion
    let min = *range.start();
    let max = *range.end();
    let t = (*value - min) / (max - min);
    let fill_rect = egui::Rect::from_min_size(
        rect.min,
        Vec2::new(rect.width() * t, rect.height()),
    );
    ui.painter().rect_filled(fill_rect, Rounding::same(2.0), bar_color);
    
    // Border
    ui.painter().rect_stroke(rect, Rounding::same(2.0), Stroke::new(1.0, Color32::from_gray(60)));
    
    response
}

/// Resolume-style property row with label, value, spinbox buttons, and colored bar
pub fn property_row(
    ui: &mut Ui,
    label: &str,
    value: &mut f32,
    range: RangeInclusive<f32>,
    step: f32,
    bar_color: Color32,
) {
    ui.horizontal(|ui| {
        ui.set_min_width(240.0);
        
        // Label
        ui.label(RichText::new(label).size(11.0).color(Color32::from_gray(180)));
        
        // Value with spinbox buttons
        let min = *range.start();
        let max = *range.end();
        
        // Minus button
        if ui.add(
            egui::Button::new(RichText::new("−").size(10.0))
                .min_size(Vec2::new(18.0, 18.0))
                .fill(Color32::from_gray(45))
        ).clicked() {
            *value = (*value - step).max(min);
        }
        
        // Value display
        ui.add(
            egui::DragValue::new(value)
                .clamp_range(range.clone())
                .speed(step * 0.1)
                .fixed_decimals(if step < 1.0 { 2 } else { 0 })
        );
        
        // Plus button
        if ui.add(
            egui::Button::new(RichText::new("+").size(10.0))
                .min_size(Vec2::new(18.0, 18.0))
                .fill(Color32::from_gray(45))
        ).clicked() {
            *value = (*value + step).min(max);
        }
        
        // Colored bar indicator
        slider_with_bar(ui, value, range, bar_color);
    });
}

/// Flip button group (4 buttons for different flip modes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlipState {
    #[default]
    None,
    Horizontal,
    Vertical,
    Both,
}

/// Draw a flip button group like Resolume
pub fn flip_buttons(ui: &mut Ui, current: &mut FlipState) {
    ui.horizontal(|ui| {
        let button_size = Vec2::new(24.0, 24.0);
        let active_color = Color32::from_rgb(74, 157, 91);
        let inactive_color = Color32::from_gray(50);
        
        // None / Reset button
        let none_active = *current == FlipState::None;
        if ui.add(
            egui::Button::new(RichText::new("⊘").size(12.0))
                .min_size(button_size)
                .fill(if none_active { active_color } else { inactive_color })
        ).on_hover_text("No flip").clicked() {
            *current = FlipState::None;
        }
        
        // Horizontal flip
        let h_active = *current == FlipState::Horizontal || *current == FlipState::Both;
        if ui.add(
            egui::Button::new(RichText::new("↔").size(12.0))
                .min_size(button_size)
                .fill(if h_active { active_color } else { inactive_color })
        ).on_hover_text("Flip horizontal").clicked() {
            *current = match *current {
                FlipState::None => FlipState::Horizontal,
                FlipState::Horizontal => FlipState::None,
                FlipState::Vertical => FlipState::Both,
                FlipState::Both => FlipState::Vertical,
            };
        }
        
        // Vertical flip
        let v_active = *current == FlipState::Vertical || *current == FlipState::Both;
        if ui.add(
            egui::Button::new(RichText::new("↕").size(12.0))
                .min_size(button_size)
                .fill(if v_active { active_color } else { inactive_color })
        ).on_hover_text("Flip vertical").clicked() {
            *current = match *current {
                FlipState::None => FlipState::Vertical,
                FlipState::Vertical => FlipState::None,
                FlipState::Horizontal => FlipState::Both,
                FlipState::Both => FlipState::Horizontal,
            };
        }
        
        // Rotate 180 (both)
        let both_active = *current == FlipState::Both;
        if ui.add(
            egui::Button::new(RichText::new("⟳").size(12.0))
                .min_size(button_size)
                .fill(if both_active { active_color } else { inactive_color })
        ).on_hover_text("Rotate 180°").clicked() {
            *current = if both_active { FlipState::None } else { FlipState::Both };
        }
    });
}

/// Device type dropdown selector
pub fn device_dropdown(
    ui: &mut Ui,
    current: &mut crate::output::DeviceType,
    displays: &[crate::output::DisplayInfo],
) {
    use crate::output::DeviceType;
    
    egui::ComboBox::from_label("")
        .selected_text(current.display_name())
        .show_ui(ui, |ui| {
            // Virtual Output
            if ui.selectable_label(*current == DeviceType::Virtual, "Virtual Output").clicked() {
                *current = DeviceType::Virtual;
            }
            
            // Fullscreen options (one per display)
            ui.separator();
            ui.label(RichText::new("Fullscreen").size(10.0).color(Color32::from_gray(120)));
            
            for display in displays {
                let label = format!("  {} ({}×{})", display.name, display.resolution.0, display.resolution.1);
                if ui.selectable_label(false, &label).clicked() {
                    *current = DeviceType::Fullscreen;
                }
            }
            
            ui.separator();
            
            // Aqueduct
            if ui.selectable_label(*current == DeviceType::Aqueduct, "Aqueduct").clicked() {
                *current = DeviceType::Aqueduct;
            }
            
            // NDI (Coming Soon)
            ui.separator();
            ui.add_enabled_ui(false, |ui| {
                ui.horizontal(|ui| {
                    ui.label("NDI");
                    ui.label(RichText::new("COMING SOON").size(9.0).color(Color32::from_rgb(200, 120, 0)));
                });
            });
        });
}

/// Input guide image loader widget
pub fn input_guide_loader(ui: &mut Ui, guide_path: &mut Option<std::path::PathBuf>) {
    ui.horizontal(|ui| {
        if ui.add(
            egui::Button::new("Load...")
                .min_size(Vec2::new(50.0, 20.0))
        ).clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "bmp"])
                .pick_file()
            {
                *guide_path = Some(path);
            }
        }
        
        if guide_path.is_some() {
            if ui.small_button("✕").clicked() {
                *guide_path = None;
            }
            ui.label(RichText::new("Image loaded").size(10.0).color(Color32::from_gray(150)));
        } else {
            ui.label(RichText::new("Drop image file here.").size(10.0).color(Color32::from_gray(100)));
        }
    });
}

