//! Reusable UI widgets with consistent behavior
//!
//! Provides slider and DragValue variants that instantly reset to default on right-click.

use egui::{DragValue, Response, Slider, Ui, PointerButton};
use std::ops::RangeInclusive;

/// Slider that resets to default on right-click (instant, no menu)
///
/// Returns the response. Check `response.changed()` to detect value changes
/// (includes both drag changes and right-click resets).
pub fn slider_with_reset(
    ui: &mut Ui,
    value: &mut f32,
    range: RangeInclusive<f32>,
    default: f32,
) -> Response {
    let mut response = ui.add(Slider::new(value, range));
    if response.clicked_by(PointerButton::Secondary) {
        *value = default;
        response.mark_changed();
    }
    response
}

/// Slider with text suffix that resets to default on right-click
pub fn slider_with_reset_suffix(
    ui: &mut Ui,
    value: &mut f32,
    range: RangeInclusive<f32>,
    default: f32,
    suffix: &str,
) -> Response {
    let mut response = ui.add(Slider::new(value, range).suffix(suffix));
    if response.clicked_by(PointerButton::Secondary) {
        *value = default;
        response.mark_changed();
    }
    response
}

/// DragValue (f32) that resets to default on right-click (instant, no menu)
pub fn drag_value_with_reset(
    ui: &mut Ui,
    value: &mut f32,
    default: f32,
) -> Response {
    let mut response = ui.add(DragValue::new(value));
    if response.clicked_by(PointerButton::Secondary) {
        *value = default;
        response.mark_changed();
    }
    response
}

/// DragValue (f32) with speed that resets to default on right-click
pub fn drag_value_with_reset_speed(
    ui: &mut Ui,
    value: &mut f32,
    default: f32,
    speed: f64,
) -> Response {
    let mut response = ui.add(DragValue::new(value).speed(speed));
    if response.clicked_by(PointerButton::Secondary) {
        *value = default;
        response.mark_changed();
    }
    response
}

/// DragValue (f32) with suffix that resets to default on right-click
pub fn drag_value_with_reset_suffix(
    ui: &mut Ui,
    value: &mut f32,
    default: f32,
    suffix: &str,
) -> Response {
    let mut response = ui.add(DragValue::new(value).suffix(suffix));
    if response.clicked_by(PointerButton::Secondary) {
        *value = default;
        response.mark_changed();
    }
    response
}

/// DragValue (i32) that resets to default on right-click
pub fn drag_value_i32_with_reset(
    ui: &mut Ui,
    value: &mut i32,
    default: i32,
) -> Response {
    let mut response = ui.add(DragValue::new(value));
    if response.clicked_by(PointerButton::Secondary) {
        *value = default;
        response.mark_changed();
    }
    response
}

/// DragValue (u32) that resets to default on right-click
pub fn drag_value_u32_with_reset(
    ui: &mut Ui,
    value: &mut u32,
    default: u32,
) -> Response {
    let mut response = ui.add(DragValue::new(value));
    if response.clicked_by(PointerButton::Secondary) {
        *value = default;
        response.mark_changed();
    }
    response
}

/// DragValue (f32) with range and suffix that resets to default on right-click
pub fn drag_value_with_reset_range_suffix(
    ui: &mut Ui,
    value: &mut f32,
    default: f32,
    range: RangeInclusive<f32>,
    suffix: &str,
) -> Response {
    let mut response = ui.add(DragValue::new(value).range(range).suffix(suffix));
    if response.clicked_by(PointerButton::Secondary) {
        *value = default;
        response.mark_changed();
    }
    response
}

/// Add right-click reset behavior to any Response
/// Call this after adding any widget, passing the value and default
pub fn add_reset_on_right_click<T: Copy>(
    response: &mut Response,
    value: &mut T,
    default: T,
) {
    if response.clicked_by(PointerButton::Secondary) {
        *value = default;
        response.mark_changed();
    }
}

/// Add right-click reset behavior (f32 version for convenience)
pub fn add_reset_f32(response: &mut Response, value: &mut f32, default: f32) {
    add_reset_on_right_click(response, value, default);
}

/// Add right-click reset behavior (i32 version for convenience)
pub fn add_reset_i32(response: &mut Response, value: &mut i32, default: i32) {
    add_reset_on_right_click(response, value, default);
}

/// Add right-click reset behavior (u32 version for convenience)
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
