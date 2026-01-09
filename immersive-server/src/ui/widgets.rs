//! Reusable UI widgets with consistent behavior
//!
//! Provides slider and DragValue variants that instantly reset to default on right-click.
//! Use the `Resettable` builder for flexible widget configuration, or the convenience
//! functions for common cases.

use egui::{DragValue, Response, Slider, Ui, PointerButton};
use std::ops::RangeInclusive;

// ============================================================================
// Resettable Widget Builder
// ============================================================================

/// Builder for widgets that reset to default on right-click.
///
/// # Example
/// ```ignore
/// // Slider with suffix
/// Resettable::slider(&mut value, 0.0..=1.0, 1.0)
///     .suffix("%")
///     .show(ui);
///
/// // DragValue with speed and range
/// Resettable::drag(&mut value, 1.0)
///     .speed(0.1)
///     .range(0.0..=100.0)
///     .show(ui);
/// ```
pub struct Resettable<'a, T: Copy + egui::emath::Numeric> {
    value: &'a mut T,
    default: T,
    range: Option<RangeInclusive<T>>,
    speed: Option<f64>,
    suffix: Option<&'a str>,
    is_slider: bool,
}

impl<'a, T: Copy + egui::emath::Numeric> Resettable<'a, T> {
    /// Create a slider builder with right-click reset.
    pub fn slider(value: &'a mut T, range: RangeInclusive<T>, default: T) -> Self {
        Self {
            value,
            default,
            range: Some(range),
            speed: None,
            suffix: None,
            is_slider: true,
        }
    }

    /// Create a DragValue builder with right-click reset.
    pub fn drag(value: &'a mut T, default: T) -> Self {
        Self {
            value,
            default,
            range: None,
            speed: None,
            suffix: None,
            is_slider: false,
        }
    }

    /// Set the range (for DragValue; ignored for Slider which requires range in constructor).
    pub fn range(mut self, range: RangeInclusive<T>) -> Self {
        self.range = Some(range);
        self
    }

    /// Set the drag speed.
    pub fn speed(mut self, speed: f64) -> Self {
        self.speed = Some(speed);
        self
    }

    /// Set the suffix text (e.g., "%", "px").
    pub fn suffix(mut self, suffix: &'a str) -> Self {
        self.suffix = Some(suffix);
        self
    }

    /// Show the widget and return the response.
    pub fn show(self, ui: &mut Ui) -> Response {
        let mut response = if self.is_slider {
            let range = self.range.expect("Slider requires a range");
            let mut slider = Slider::new(self.value, range);
            if let Some(suffix) = self.suffix {
                slider = slider.suffix(suffix);
            }
            ui.add(slider)
        } else {
            let mut drag = DragValue::new(self.value);
            if let Some(range) = self.range {
                drag = drag.range(range);
            }
            if let Some(speed) = self.speed {
                drag = drag.speed(speed);
            }
            if let Some(suffix) = self.suffix {
                drag = drag.suffix(suffix);
            }
            ui.add(drag)
        };

        if response.clicked_by(PointerButton::Secondary) {
            *self.value = self.default;
            response.mark_changed();
        }
        response
    }
}

// ============================================================================
// Convenience Functions (using the builder internally)
// ============================================================================

/// Slider that resets to default on right-click (instant, no menu).
pub fn slider_with_reset(
    ui: &mut Ui,
    value: &mut f32,
    range: RangeInclusive<f32>,
    default: f32,
) -> Response {
    Resettable::slider(value, range, default).show(ui)
}

/// Slider with text suffix that resets to default on right-click.
pub fn slider_with_reset_suffix(
    ui: &mut Ui,
    value: &mut f32,
    range: RangeInclusive<f32>,
    default: f32,
    suffix: &str,
) -> Response {
    Resettable::slider(value, range, default).suffix(suffix).show(ui)
}

/// DragValue (f32) that resets to default on right-click.
pub fn drag_value_with_reset(ui: &mut Ui, value: &mut f32, default: f32) -> Response {
    Resettable::drag(value, default).show(ui)
}

/// DragValue (f32) with speed that resets to default on right-click.
pub fn drag_value_with_reset_speed(
    ui: &mut Ui,
    value: &mut f32,
    default: f32,
    speed: f64,
) -> Response {
    Resettable::drag(value, default).speed(speed).show(ui)
}

/// DragValue (f32) with suffix that resets to default on right-click.
pub fn drag_value_with_reset_suffix(
    ui: &mut Ui,
    value: &mut f32,
    default: f32,
    suffix: &str,
) -> Response {
    Resettable::drag(value, default).suffix(suffix).show(ui)
}

/// DragValue (i32) that resets to default on right-click.
pub fn drag_value_i32_with_reset(ui: &mut Ui, value: &mut i32, default: i32) -> Response {
    Resettable::drag(value, default).show(ui)
}

/// DragValue (u32) that resets to default on right-click.
pub fn drag_value_u32_with_reset(ui: &mut Ui, value: &mut u32, default: u32) -> Response {
    Resettable::drag(value, default).show(ui)
}

/// DragValue (f32) with range and suffix that resets to default on right-click.
pub fn drag_value_with_reset_range_suffix(
    ui: &mut Ui,
    value: &mut f32,
    default: f32,
    range: RangeInclusive<f32>,
    suffix: &str,
) -> Response {
    Resettable::drag(value, default).range(range).suffix(suffix).show(ui)
}

// ============================================================================
// Generic Reset Helper
// ============================================================================

/// Add right-click reset behavior to any Response.
/// Call this after adding any widget, passing the value and default.
pub fn add_reset_on_right_click<T: Copy>(response: &mut Response, value: &mut T, default: T) {
    if response.clicked_by(PointerButton::Secondary) {
        *value = default;
        response.mark_changed();
    }
}

/// Add right-click reset behavior (f32 version for convenience).
pub fn add_reset_f32(response: &mut Response, value: &mut f32, default: f32) {
    add_reset_on_right_click(response, value, default);
}

/// Add right-click reset behavior (i32 version for convenience).
pub fn add_reset_i32(response: &mut Response, value: &mut i32, default: i32) {
    add_reset_on_right_click(response, value, default);
}

/// Add right-click reset behavior (u32 version for convenience).
pub fn add_reset_u32(response: &mut Response, value: &mut u32, default: u32) {
    add_reset_on_right_click(response, value, default);
}

// ============================================================================
// Texture Registration Helpers
// ============================================================================

use egui_wgpu::Renderer as EguiRenderer;

/// Register a wgpu TextureView with egui, freeing any existing texture first.
///
/// This handles the common pattern of:
/// 1. Free the old texture if it exists
/// 2. Register the new texture with egui
/// 3. Store the new texture ID
///
/// Texture format (RGBA/BGRA) is determined by the TextureView, not this function.
pub fn register_egui_texture(
    egui_renderer: &mut EguiRenderer,
    device: &wgpu::Device,
    texture_view: &wgpu::TextureView,
    current_id: &mut Option<egui::TextureId>,
) -> egui::TextureId {
    if let Some(old_id) = current_id.take() {
        egui_renderer.free_texture(&old_id);
    }
    let texture_id = egui_renderer.register_native_texture(
        device,
        texture_view,
        wgpu::FilterMode::Linear,
    );
    *current_id = Some(texture_id);
    texture_id
}

/// Register a wgpu TextureView with egui using a raw pointer (for borrowed views).
///
/// # Safety
/// The `texture_view_ptr` must point to a valid TextureView for the duration of this call.
pub unsafe fn register_egui_texture_ptr(
    egui_renderer: &mut EguiRenderer,
    device: &wgpu::Device,
    texture_view_ptr: *const wgpu::TextureView,
    current_id: &mut Option<egui::TextureId>,
) -> egui::TextureId {
    if let Some(old_id) = current_id.take() {
        egui_renderer.free_texture(&old_id);
    }
    let texture_id = egui_renderer.register_native_texture(
        device,
        &*texture_view_ptr,
        wgpu::FilterMode::Linear,
    );
    *current_id = Some(texture_id);
    texture_id
}

/// Free an egui texture if it exists, setting the ID to None.
pub fn free_egui_texture(
    egui_renderer: &mut EguiRenderer,
    current_id: &mut Option<egui::TextureId>,
) {
    if let Some(id) = current_id.take() {
        egui_renderer.free_texture(&id);
    }
}

// ============================================================================
// Texture Rendering Helpers
// ============================================================================

/// Full UV rect (0,0) to (1,1) for rendering entire texture.
pub const FULL_UV: egui::Rect = egui::Rect {
    min: egui::pos2(0.0, 0.0),
    max: egui::pos2(1.0, 1.0),
};

/// Draw a texture filling a rect with full UVs.
pub fn draw_texture(ui: &Ui, texture_id: egui::TextureId, rect: egui::Rect) {
    ui.painter().image(texture_id, rect, FULL_UV, egui::Color32::WHITE);
}

/// Draw a texture with custom UV coordinates.
pub fn draw_texture_uv(
    ui: &Ui,
    texture_id: egui::TextureId,
    rect: egui::Rect,
    uv_rect: egui::Rect,
) {
    ui.painter().image(texture_id, rect, uv_rect, egui::Color32::WHITE);
}

/// Draw a texture with aspect-ratio preservation (letterbox/pillarbox).
/// Returns the actual rect where the texture was drawn.
pub fn draw_texture_aspect_fit(
    ui: &Ui,
    texture_id: egui::TextureId,
    available: egui::Rect,
    texture_aspect: f32,
) -> egui::Rect {
    let available_aspect = available.width() / available.height();

    let image_rect = if texture_aspect > available_aspect {
        // Texture is wider - fit width, center vertically
        let height = available.width() / texture_aspect;
        let y_offset = (available.height() - height) / 2.0;
        egui::Rect::from_min_size(
            egui::pos2(available.left(), available.top() + y_offset),
            egui::vec2(available.width(), height),
        )
    } else {
        // Texture is taller - fit height, center horizontally
        let width = available.height() * texture_aspect;
        let x_offset = (available.width() - width) / 2.0;
        egui::Rect::from_min_size(
            egui::pos2(available.left() + x_offset, available.top()),
            egui::vec2(width, available.height()),
        )
    };

    draw_texture(ui, texture_id, image_rect);
    image_rect
}

/// Draw a placeholder when texture is not available.
pub fn draw_texture_placeholder(ui: &Ui, rect: egui::Rect, message: &str) {
    ui.painter().rect_filled(rect, 4.0, egui::Color32::from_gray(30));
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        message,
        egui::FontId::default(),
        egui::Color32::GRAY,
    );
}

/// Draw a texture or placeholder if None.
pub fn draw_texture_or_placeholder(
    ui: &Ui,
    texture_id: Option<egui::TextureId>,
    rect: egui::Rect,
    placeholder_message: &str,
) {
    if let Some(tex_id) = texture_id {
        draw_texture(ui, tex_id, rect);
    } else {
        draw_texture_placeholder(ui, rect, placeholder_message);
    }
}

// ============================================================================
// SliderLimit Widget - Dual-Handle Range Selection
// ============================================================================

/// A dual-handle range slider for selecting min/max limits.
///
/// Visual design: Two triangular handles (▲) on a track, with the region
/// between them highlighted. Users can drag either handle independently.
///
/// # Example
/// ```ignore
/// let response = SliderLimit::new(&mut min, &mut max, 0.0..=1.0).show(ui);
/// if response.changed() {
///     // min or max was modified
/// }
/// ```
pub struct SliderLimit<'a> {
    min: &'a mut f32,
    max: &'a mut f32,
    range: RangeInclusive<f32>,
    width: Option<f32>,
}

impl<'a> SliderLimit<'a> {
    /// Create a new SliderLimit widget
    pub fn new(min: &'a mut f32, max: &'a mut f32, range: RangeInclusive<f32>) -> Self {
        Self {
            min,
            max,
            range,
            width: None,
        }
    }

    /// Set the width of the slider
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Show the widget and return the response
    pub fn show(self, ui: &mut Ui) -> Response {
        let width = self.width.unwrap_or(ui.available_width().min(200.0));
        let height = 16.0;
        let handle_size = 6.0;
        let track_padding = handle_size;

        let (rect, mut response) = ui.allocate_exact_size(
            egui::vec2(width, height),
            egui::Sense::click_and_drag()
        );

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            let track_left = rect.left() + track_padding;
            let track_right = rect.right() - track_padding;
            let track_width = track_right - track_left;
            let track_y = rect.center().y;

            let range_min = *self.range.start();
            let range_max = *self.range.end();
            let range_span = range_max - range_min;

            // Normalize values to 0-1
            let min_norm = (*self.min - range_min) / range_span;
            let max_norm = (*self.max - range_min) / range_span;

            let min_x = track_left + min_norm * track_width;
            let max_x = track_left + max_norm * track_width;

            // Draw track background
            painter.line_segment(
                [egui::pos2(track_left, track_y), egui::pos2(track_right, track_y)],
                egui::Stroke::new(2.0, egui::Color32::from_gray(50))
            );

            // Draw highlighted region between handles
            painter.line_segment(
                [egui::pos2(min_x, track_y), egui::pos2(max_x, track_y)],
                egui::Stroke::new(3.0, egui::Color32::from_rgb(80, 120, 180))
            );

            // Determine which handle is being hovered/dragged
            let pointer_pos = response.interact_pointer_pos();
            let dist_to_min = pointer_pos.map(|p| (p.x - min_x).abs()).unwrap_or(f32::MAX);
            let dist_to_max = pointer_pos.map(|p| (p.x - max_x).abs()).unwrap_or(f32::MAX);
            let hovering_min = dist_to_min < dist_to_max && dist_to_min < handle_size * 2.5;
            let hovering_max = dist_to_max <= dist_to_min && dist_to_max < handle_size * 2.5;

            // Draw triangular handles (pointing up)
            let draw_handle = |x: f32, hovered: bool| {
                let color = if hovered {
                    egui::Color32::WHITE
                } else {
                    egui::Color32::from_gray(180)
                };

                // Triangle pointing up
                let top = egui::pos2(x, track_y - handle_size);
                let left = egui::pos2(x - handle_size * 0.7, track_y + 1.0);
                let right = egui::pos2(x + handle_size * 0.7, track_y + 1.0);

                painter.add(egui::Shape::convex_polygon(
                    vec![top, right, left],
                    color,
                    egui::Stroke::new(1.0, egui::Color32::from_gray(60))
                ));
            };

            draw_handle(min_x, hovering_min);
            draw_handle(max_x, hovering_max);

            // Handle dragging
            if response.dragged() {
                if let Some(pos) = pointer_pos {
                    let new_norm = ((pos.x - track_left) / track_width).clamp(0.0, 1.0);
                    let new_value = range_min + new_norm * range_span;

                    // Move the closer handle
                    if dist_to_min < dist_to_max {
                        // Dragging min handle - can't exceed max
                        *self.min = new_value.min(*self.max - 0.01);
                    } else {
                        // Dragging max handle - can't go below min
                        *self.max = new_value.max(*self.min + 0.01);
                    }
                    response.mark_changed();
                }
            }

            // Right-click to reset to full range
            if response.clicked_by(PointerButton::Secondary) {
                *self.min = range_min;
                *self.max = range_max;
                response.mark_changed();
            }
        }

        response
    }
}

/// Convenience function for slider_limit widget
pub fn slider_limit(
    ui: &mut Ui,
    min: &mut f32,
    max: &mut f32,
    range: RangeInclusive<f32>,
) -> Response {
    SliderLimit::new(min, max, range).show(ui)
}
