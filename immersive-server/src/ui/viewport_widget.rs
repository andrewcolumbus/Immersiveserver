//! Unified viewport interaction widget for pan/zoom behavior.
//!
//! Provides consistent viewport handling across all preview areas:
//! - Preview Monitor Panel (egui)
//! - Advanced Output Window (egui)
//! - Main Environment Preview (winit)

use crate::compositor::Viewport;

/// Default scroll sensitivity divisor (matches egui panels)
pub const DEFAULT_SCROLL_SENSITIVITY: f32 = 50.0;
/// Default scroll threshold to trigger zoom
pub const DEFAULT_SCROLL_THRESHOLD: f32 = 0.5;

/// Configuration for viewport interaction behavior
#[derive(Clone)]
pub struct ViewportConfig {
    /// Scroll sensitivity divisor (higher = less sensitive). Default: 50.0
    pub scroll_sensitivity: f32,
    /// Minimum scroll delta to trigger zoom. Default: 0.5
    pub scroll_threshold: f32,
    /// Enable double-click to reset. Default: true
    pub double_click_reset: bool,
}

impl Default for ViewportConfig {
    fn default() -> Self {
        Self {
            scroll_sensitivity: DEFAULT_SCROLL_SENSITIVITY,
            scroll_threshold: DEFAULT_SCROLL_THRESHOLD,
            double_click_reset: true,
        }
    }
}

/// Response from viewport widget interaction
#[derive(Default)]
pub struct ViewportResponse {
    /// True if any interaction occurred (repaint needed)
    pub changed: bool,
    /// True if viewport was reset to default
    pub was_reset: bool,
}

// =============================================================================
// egui Input Handling
// =============================================================================

/// Handle viewport interactions for an egui response.
///
/// Call this after `ui.allocate_rect()` with `Sense::click_and_drag()`.
/// Handles: right-click drag (pan), scroll wheel (zoom), double-right-click (reset)
pub fn handle_viewport_input(
    ui: &egui::Ui,
    response: &egui::Response,
    rect: egui::Rect,
    viewport: &mut Viewport,
    content_size: (f32, f32),
    config: &ViewportConfig,
) -> ViewportResponse {
    let preview_size = (rect.width(), rect.height());
    let mut result = ViewportResponse::default();

    // Handle double-right-click to reset viewport
    if config.double_click_reset && response.double_clicked_by(egui::PointerButton::Secondary) {
        viewport.reset();
        result.changed = true;
        result.was_reset = true;
        return result;
    }

    // Handle right-click drag start
    if response.drag_started_by(egui::PointerButton::Secondary) {
        if let Some(pos) = response.interact_pointer_pos() {
            let local_pos = (pos.x - rect.left(), pos.y - rect.top());
            viewport.on_right_mouse_down(local_pos);
            result.changed = true;
        }
    }

    // Handle right-click drag
    if response.dragged_by(egui::PointerButton::Secondary) {
        if let Some(pos) = response.interact_pointer_pos() {
            let local_pos = (pos.x - rect.left(), pos.y - rect.top());
            viewport.on_mouse_move(local_pos, preview_size, content_size);
            result.changed = true;
        }
    }

    // Handle right-click drag end
    if response.drag_stopped_by(egui::PointerButton::Secondary) {
        viewport.on_right_mouse_up();
        result.changed = true;
    }

    // Handle scroll wheel zoom (when hovered)
    if response.hovered() {
        let scroll = ui.input(|i| i.raw_scroll_delta.y);
        if scroll.abs() > config.scroll_threshold {
            if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                let local_pos = (pos.x - rect.left(), pos.y - rect.top());
                // Normalize scroll to reasonable zoom increments
                let zoom_delta = scroll / config.scroll_sensitivity;
                viewport.on_scroll(zoom_delta, local_pos, preview_size, content_size);
                result.changed = true;
            }
        }
    }

    result
}

/// Compute UV rect from viewport for egui image rendering.
///
/// Use this when rendering textures with `ui.painter().image()` to apply
/// viewport pan/zoom transformations.
pub fn compute_uv_rect(
    viewport: &Viewport,
    preview_size: (f32, f32),
    content_size: (f32, f32),
) -> egui::Rect {
    let (scale_x, scale_y, offset_x, offset_y) = viewport.get_shader_params(preview_size, content_size);

    // Convert shader params to UV rect
    // The shader does: adjusted_uv = (uv - 0.5) / scale + 0.5 + offset
    // So we need to invert this for the UV rect
    let half_width = 0.5 / scale_x;
    let half_height = 0.5 / scale_y;
    let center_u = 0.5 - offset_x / scale_x;
    let center_v = 0.5 - offset_y / scale_y;

    egui::Rect::from_min_max(
        egui::pos2(center_u - half_width, center_v - half_height),
        egui::pos2(center_u + half_width, center_v + half_height),
    )
}

/// Result of UV rect computation for zoom-out scenarios
pub struct UvRenderInfo {
    /// The UV rect (clamped to 0-1)
    pub uv_rect: egui::Rect,
    /// The destination rect within the preview area where texture should be drawn
    pub dest_rect: egui::Rect,
}

/// Compute UV rect and destination rect for rendering.
///
/// When zoomed out (below 100%), the UV coordinates may extend beyond 0-1,
/// which causes texture clamping artifacts. This function clamps the UV rect
/// to valid 0-1 bounds and computes a corresponding smaller destination rect.
pub fn compute_uv_and_dest_rect(
    viewport: &Viewport,
    preview_rect: egui::Rect,
    content_size: (f32, f32),
) -> UvRenderInfo {
    let preview_size = (preview_rect.width(), preview_rect.height());
    let (scale_x, scale_y, offset_x, offset_y) = viewport.get_shader_params(preview_size, content_size);

    // Calculate UV rect (may be outside 0-1 when zoomed out)
    let half_width = 0.5 / scale_x;
    let half_height = 0.5 / scale_y;
    let center_u = 0.5 - offset_x / scale_x;
    let center_v = 0.5 - offset_y / scale_y;

    let uv_min_x = center_u - half_width;
    let uv_max_x = center_u + half_width;
    let uv_min_y = center_v - half_height;
    let uv_max_y = center_v + half_height;

    // Calculate how much of UV is valid (0-1) and adjust dest rect accordingly
    let valid_uv_min_x = uv_min_x.max(0.0);
    let valid_uv_max_x = uv_max_x.min(1.0);
    let valid_uv_min_y = uv_min_y.max(0.0);
    let valid_uv_max_y = uv_max_y.min(1.0);

    // Map back to destination coordinates
    let uv_width = uv_max_x - uv_min_x;
    let uv_height = uv_max_y - uv_min_y;

    // Avoid division by zero
    if uv_width.abs() < 0.0001 || uv_height.abs() < 0.0001 {
        return UvRenderInfo {
            uv_rect: egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            dest_rect: preview_rect,
        };
    }

    let dest_left = preview_rect.left() + ((valid_uv_min_x - uv_min_x) / uv_width) * preview_rect.width();
    let dest_right = preview_rect.left() + ((valid_uv_max_x - uv_min_x) / uv_width) * preview_rect.width();
    let dest_top = preview_rect.top() + ((valid_uv_min_y - uv_min_y) / uv_height) * preview_rect.height();
    let dest_bottom = preview_rect.top() + ((valid_uv_max_y - uv_min_y) / uv_height) * preview_rect.height();

    UvRenderInfo {
        uv_rect: egui::Rect::from_min_max(
            egui::pos2(valid_uv_min_x, valid_uv_min_y),
            egui::pos2(valid_uv_max_x, valid_uv_max_y),
        ),
        dest_rect: egui::Rect::from_min_max(
            egui::pos2(dest_left, dest_top),
            egui::pos2(dest_right, dest_bottom),
        ),
    }
}

/// Draw zoom level indicator in the bottom-right corner of the viewport.
///
/// Shows current zoom percentage (e.g., "100%", "200%", "50%")
/// Only draws when zoom != 100% to reduce visual clutter.
pub fn draw_zoom_indicator(
    ui: &egui::Ui,
    rect: egui::Rect,
    viewport: &Viewport,
) {
    let zoom_percent = (viewport.zoom() * 100.0).round() as i32;

    // Don't show indicator at default zoom (100%)
    if zoom_percent == 100 {
        return;
    }

    let text = format!("{}%", zoom_percent);
    let font = egui::FontId::proportional(12.0);
    let text_color = egui::Color32::WHITE;
    let bg_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180);

    // Calculate position (bottom-right with padding)
    let padding = 8.0;
    let text_galley = ui.painter().layout_no_wrap(text.clone(), font.clone(), text_color);
    let text_size = text_galley.size();

    let bg_rect = egui::Rect::from_min_size(
        egui::pos2(
            rect.right() - text_size.x - padding * 3.0,
            rect.bottom() - text_size.y - padding * 3.0,
        ),
        egui::vec2(text_size.x + padding * 2.0, text_size.y + padding * 2.0),
    );

    // Draw background and text
    ui.painter().rect_filled(bg_rect, 4.0, bg_color);
    ui.painter().text(
        bg_rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        font,
        text_color,
    );
}

// =============================================================================
// winit Input Handling (for main window)
// =============================================================================

/// Handle winit scroll event for viewport zoom
pub fn handle_winit_scroll(
    viewport: &mut Viewport,
    scroll_delta: f32,
    cursor_pos: (f32, f32),
    window_size: (f32, f32),
    content_size: (f32, f32),
    config: &ViewportConfig,
) -> ViewportResponse {
    let normalized_delta = scroll_delta / config.scroll_sensitivity;
    if scroll_delta.abs() < config.scroll_threshold {
        return ViewportResponse::default();
    }
    viewport.on_scroll(normalized_delta, cursor_pos, window_size, content_size);
    ViewportResponse { changed: true, was_reset: false }
}

/// Handle winit right mouse button down for viewport pan start
pub fn handle_winit_right_mouse_down(
    viewport: &mut Viewport,
    pos: (f32, f32),
    config: &ViewportConfig,
) -> ViewportResponse {
    let was_reset = viewport.on_right_mouse_down(pos);
    ViewportResponse {
        changed: true,
        was_reset: config.double_click_reset && was_reset,
    }
}

/// Handle winit mouse move for viewport pan
pub fn handle_winit_mouse_move(
    viewport: &mut Viewport,
    pos: (f32, f32),
    window_size: (f32, f32),
    content_size: (f32, f32),
) -> ViewportResponse {
    viewport.on_mouse_move(pos, window_size, content_size);
    ViewportResponse { changed: true, was_reset: false }
}

/// Handle winit right mouse button up
pub fn handle_winit_right_mouse_up(viewport: &mut Viewport) -> ViewportResponse {
    viewport.on_right_mouse_up();
    ViewportResponse { changed: true, was_reset: false }
}
