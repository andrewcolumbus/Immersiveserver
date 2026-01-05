//! Advanced Output Window
//!
//! A modal window for configuring multi-screen outputs with slice-based input selection.
//! Accessible via View → Advanced Output.

use crate::output::{DisplayInfo, EdgeBlendConfig, MaskShape, OutputDevice, OutputManager, Point2D as MaskPoint2D, Screen, ScreenId, Slice, SliceId, SliceInput, SliceMask, WarpMesh};
use crate::output::slice::Point2D;

/// Actions returned from the Advanced Output window
#[derive(Debug, Clone)]
pub enum AdvancedOutputAction {
    /// Add a new screen
    AddScreen,
    /// Remove a screen
    RemoveScreen { screen_id: ScreenId },
    /// Add a slice to a screen
    AddSlice { screen_id: ScreenId },
    /// Remove a slice from a screen
    RemoveSlice {
        screen_id: ScreenId,
        slice_id: SliceId,
    },
    /// Update slice properties
    UpdateSlice {
        screen_id: ScreenId,
        slice_id: SliceId,
        slice: Slice,
    },
    /// Update screen properties
    UpdateScreen { screen_id: ScreenId, screen: Screen },
}

/// Advanced Output window for configuring multi-screen outputs
pub struct AdvancedOutputWindow {
    /// Whether the window is open
    pub open: bool,
    /// Currently selected screen
    selected_screen: Option<ScreenId>,
    /// Currently selected slice within selected screen
    selected_slice: Option<SliceId>,
    /// Temporary screen name for editing
    temp_screen_name: String,
    /// Temporary slice name for editing
    temp_slice_name: String,
    /// Temporary resolution strings
    temp_width: String,
    temp_height: String,
    /// egui texture ID for the live preview
    pub preview_texture_id: Option<egui::TextureId>,
    /// Currently dragged warp point (col, row)
    dragging_warp_point: Option<(usize, usize)>,
    /// Currently dragged mask vertex index
    dragging_mask_vertex: Option<usize>,
    /// Temporary device name for streaming outputs
    temp_device_name: String,
    /// Temporary OMT port
    temp_omt_port: String,
}

impl Default for AdvancedOutputWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl AdvancedOutputWindow {
    /// Create a new Advanced Output window (closed by default)
    pub fn new() -> Self {
        Self {
            open: false,
            selected_screen: None,
            selected_slice: None,
            temp_screen_name: String::new(),
            temp_slice_name: String::new(),
            temp_width: String::new(),
            temp_height: String::new(),
            preview_texture_id: None,
            dragging_warp_point: None,
            dragging_mask_vertex: None,
            temp_device_name: String::new(),
            temp_omt_port: "5960".to_string(),
        }
    }

    /// Get the currently selected screen ID (for texture registration in app.rs)
    pub fn selected_screen_id(&self) -> Option<ScreenId> {
        self.selected_screen
    }

    /// Toggle the window open/closed
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    /// Render the Advanced Output window
    ///
    /// Returns a list of actions to be processed by the app.
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        output_manager: Option<&OutputManager>,
        layer_count: usize,
        available_displays: &[DisplayInfo],
    ) -> Vec<AdvancedOutputAction> {
        let mut actions = Vec::new();

        if !self.open {
            return actions;
        }

        let mut open = self.open;
        egui::Window::new("Advanced Output")
            .id(egui::Id::new("advanced_output_window"))
            .open(&mut open)
            .default_size([700.0, 650.0])
            .min_width(500.0)
            .min_height(400.0)
            .resizable(true)
            .collapsible(true)
            .show(ctx, |ui| {
                self.render_contents(ui, output_manager, layer_count, available_displays, &mut actions);
            });
        self.open = open;

        actions
    }

    /// Render the window contents
    fn render_contents(
        &mut self,
        ui: &mut egui::Ui,
        output_manager: Option<&OutputManager>,
        layer_count: usize,
        available_displays: &[DisplayInfo],
        actions: &mut Vec<AdvancedOutputAction>,
    ) {
        // Get screens from output manager
        let screens: Vec<&Screen> = output_manager
            .map(|m| m.screens().collect())
            .unwrap_or_default();

        // Validate selected screen still exists
        if let Some(screen_id) = self.selected_screen {
            if !screens.iter().any(|s| s.id == screen_id) {
                self.selected_screen = None;
                self.selected_slice = None;
            }
        }

        // Three-column layout: Screens | Preview | Properties
        // Use allocate_ui_with_layout to fill available vertical space
        let available = ui.available_size();
        ui.allocate_ui_with_layout(
            available,
            egui::Layout::left_to_right(egui::Align::TOP),
            |ui| {
            // LEFT COLUMN: Screens and Slices list
            ui.vertical(|ui| {
                ui.set_min_width(150.0);
                ui.set_max_width(180.0);

                // Screens section
                ui.heading("Screens");
                ui.add_space(4.0);

                egui::ScrollArea::vertical()
                    .id_salt("screens_list")
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for screen in &screens {
                            let is_selected = self.selected_screen == Some(screen.id);
                            let response = ui.selectable_label(
                                is_selected,
                                format!(
                                    "{} ({}x{})",
                                    screen.name, screen.width, screen.height
                                ),
                            );
                            if response.clicked() {
                                self.selected_screen = Some(screen.id);
                                self.selected_slice = None;
                                self.temp_screen_name = screen.name.clone();
                                self.temp_width = screen.width.to_string();
                                self.temp_height = screen.height.to_string();
                            }
                        }
                    });

                ui.horizontal(|ui| {
                    if ui.small_button("+").clicked() {
                        actions.push(AdvancedOutputAction::AddScreen);
                    }
                    if ui
                        .add_enabled(
                            self.selected_screen.is_some(),
                            egui::Button::new("-").small(),
                        )
                        .clicked()
                    {
                        if let Some(screen_id) = self.selected_screen {
                            actions.push(AdvancedOutputAction::RemoveScreen { screen_id });
                            self.selected_screen = None;
                            self.selected_slice = None;
                        }
                    }
                });

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);

                // Slices section (for selected screen)
                ui.heading("Slices");
                ui.add_space(4.0);

                if let Some(screen_id) = self.selected_screen {
                    if let Some(screen) = screens.iter().find(|s| s.id == screen_id) {
                        egui::ScrollArea::vertical()
                            .id_salt("slices_list")
                            .max_height(200.0)
                            .show(ui, |ui| {
                                for slice in &screen.slices {
                                    let is_selected = self.selected_slice == Some(slice.id);
                                    let input_label = match &slice.input {
                                        SliceInput::Composition => "Comp".to_string(),
                                        SliceInput::Layer { layer_id } => {
                                            format!("L{}", layer_id)
                                        }
                                    };
                                    let response = ui.selectable_label(
                                        is_selected,
                                        format!("{} [{}]", slice.name, input_label),
                                    );
                                    if response.clicked() {
                                        self.selected_slice = Some(slice.id);
                                        self.temp_slice_name = slice.name.clone();
                                    }
                                }
                            });

                        ui.horizontal(|ui| {
                            if ui.small_button("+").clicked() {
                                actions.push(AdvancedOutputAction::AddSlice { screen_id });
                            }
                            if ui
                                .add_enabled(
                                    self.selected_slice.is_some(),
                                    egui::Button::new("-").small(),
                                )
                                .clicked()
                            {
                                if let Some(slice_id) = self.selected_slice {
                                    actions.push(AdvancedOutputAction::RemoveSlice {
                                        screen_id,
                                        slice_id,
                                    });
                                    self.selected_slice = None;
                                }
                            }
                        });
                    }
                } else {
                    ui.label(
                        egui::RichText::new("Select a screen")
                            .weak()
                            .italics(),
                    );
                }
            });

            ui.separator();

            // MIDDLE COLUMN: Preview
            ui.vertical(|ui| {
                ui.set_min_width(200.0);
                ui.heading("Preview");
                ui.add_space(4.0);

                // Preview area - use live texture if available
                // Calculate preview size based on available space (4:3 aspect ratio)
                let available_height = (ui.available_height() - 100.0).max(100.0).min(400.0);
                let preview_size = egui::vec2(available_height * 4.0 / 3.0, available_height);
                let (rect, response) =
                    ui.allocate_exact_size(preview_size, egui::Sense::click_and_drag());

                // Draw preview background
                ui.painter().rect_filled(
                    rect,
                    4.0,
                    egui::Color32::from_rgb(30, 30, 30),
                );

                if let Some(screen_id) = self.selected_screen {
                    if let Some(screen) = screens.iter().find(|s| s.id == screen_id) {
                        // Draw live texture if available and screen is enabled
                        if screen.enabled {
                            if let Some(tex_id) = self.preview_texture_id {
                                // Draw live preview texture
                                ui.painter().image(
                                    tex_id,
                                    rect,
                                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                    egui::Color32::WHITE,
                                );
                            } else {
                                // Texture not yet registered
                                ui.painter().text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    format!("{}x{}\n(loading...)", screen.width, screen.height),
                                    egui::FontId::proportional(12.0),
                                    egui::Color32::GRAY,
                                );
                            }
                        } else {
                            // Screen disabled
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                format!("{}x{}\n(disabled)", screen.width, screen.height),
                                egui::FontId::proportional(12.0),
                                egui::Color32::DARK_GRAY,
                            );
                        }

                        // Draw slice rectangles overlay (semi-transparent outlines)
                        for slice in &screen.slices {
                            if slice.enabled {
                                let slice_rect = egui::Rect::from_min_size(
                                    rect.min
                                        + egui::vec2(
                                            slice.output.rect.x * preview_size.x,
                                            slice.output.rect.y * preview_size.y,
                                        ),
                                    egui::vec2(
                                        slice.output.rect.width * preview_size.x,
                                        slice.output.rect.height * preview_size.y,
                                    ),
                                );

                                let is_selected = self.selected_slice == Some(slice.id);
                                let stroke_color = if is_selected {
                                    egui::Color32::from_rgb(100, 149, 237) // Cornflower blue
                                } else {
                                    egui::Color32::from_rgba_unmultiplied(120, 120, 120, 100)
                                };

                                ui.painter().rect_stroke(
                                    slice_rect,
                                    0.0,
                                    egui::Stroke::new(
                                        if is_selected { 2.0 } else { 1.0 },
                                        stroke_color,
                                    ),
                                    egui::StrokeKind::Inside,
                                );

                                // Draw edge blend regions for selected slice
                                if is_selected && slice.output.edge_blend.is_any_enabled() {
                                    let blend_color = egui::Color32::from_rgba_unmultiplied(255, 200, 0, 40);
                                    let edge = &slice.output.edge_blend;

                                    // Left edge blend region
                                    if edge.left.enabled {
                                        let blend_rect = egui::Rect::from_min_max(
                                            slice_rect.left_top(),
                                            egui::pos2(
                                                slice_rect.left() + slice_rect.width() * edge.left.width,
                                                slice_rect.bottom(),
                                            ),
                                        );
                                        ui.painter().rect_filled(blend_rect, 0.0, blend_color);
                                    }

                                    // Right edge blend region
                                    if edge.right.enabled {
                                        let blend_rect = egui::Rect::from_min_max(
                                            egui::pos2(
                                                slice_rect.right() - slice_rect.width() * edge.right.width,
                                                slice_rect.top(),
                                            ),
                                            slice_rect.right_bottom(),
                                        );
                                        ui.painter().rect_filled(blend_rect, 0.0, blend_color);
                                    }

                                    // Top edge blend region
                                    if edge.top.enabled {
                                        let blend_rect = egui::Rect::from_min_max(
                                            slice_rect.left_top(),
                                            egui::pos2(
                                                slice_rect.right(),
                                                slice_rect.top() + slice_rect.height() * edge.top.width,
                                            ),
                                        );
                                        ui.painter().rect_filled(blend_rect, 0.0, blend_color);
                                    }

                                    // Bottom edge blend region
                                    if edge.bottom.enabled {
                                        let blend_rect = egui::Rect::from_min_max(
                                            egui::pos2(
                                                slice_rect.left(),
                                                slice_rect.bottom() - slice_rect.height() * edge.bottom.width,
                                            ),
                                            slice_rect.right_bottom(),
                                        );
                                        ui.painter().rect_filled(blend_rect, 0.0, blend_color);
                                    }
                                }

                                // Draw mesh warp grid overlay for selected slice
                                if is_selected {
                                    if let Some(mesh) = &slice.output.mesh {
                                        let grid_color = egui::Color32::from_rgba_unmultiplied(255, 200, 100, 150);
                                        let point_color = egui::Color32::from_rgb(255, 200, 100);

                                        // Draw grid lines (horizontal)
                                        for row in 0..mesh.rows {
                                            for col in 0..(mesh.columns - 1) {
                                                if let (Some(p1), Some(p2)) = (mesh.get_point(col, row), mesh.get_point(col + 1, row)) {
                                                    let pos1 = slice_rect.min + egui::vec2(
                                                        p1.position[0] * slice_rect.width(),
                                                        p1.position[1] * slice_rect.height(),
                                                    );
                                                    let pos2 = slice_rect.min + egui::vec2(
                                                        p2.position[0] * slice_rect.width(),
                                                        p2.position[1] * slice_rect.height(),
                                                    );
                                                    ui.painter().line_segment(
                                                        [pos1, pos2],
                                                        egui::Stroke::new(1.0, grid_color),
                                                    );
                                                }
                                            }
                                        }

                                        // Draw grid lines (vertical)
                                        for col in 0..mesh.columns {
                                            for row in 0..(mesh.rows - 1) {
                                                if let (Some(p1), Some(p2)) = (mesh.get_point(col, row), mesh.get_point(col, row + 1)) {
                                                    let pos1 = slice_rect.min + egui::vec2(
                                                        p1.position[0] * slice_rect.width(),
                                                        p1.position[1] * slice_rect.height(),
                                                    );
                                                    let pos2 = slice_rect.min + egui::vec2(
                                                        p2.position[0] * slice_rect.width(),
                                                        p2.position[1] * slice_rect.height(),
                                                    );
                                                    ui.painter().line_segment(
                                                        [pos1, pos2],
                                                        egui::Stroke::new(1.0, grid_color),
                                                    );
                                                }
                                            }
                                        }

                                        // Draw control points
                                        for point in &mesh.points {
                                            let pos = slice_rect.min + egui::vec2(
                                                point.position[0] * slice_rect.width(),
                                                point.position[1] * slice_rect.height(),
                                            );
                                            ui.painter().circle_filled(pos, 3.0, point_color);
                                        }
                                    }
                                }

                                // Draw mask outline for selected slice
                                if let Some(mask) = &slice.mask {
                                    if mask.enabled {
                                        let mask_color = egui::Color32::from_rgba_unmultiplied(255, 100, 255, 200);
                                        let point_color = egui::Color32::from_rgb(255, 100, 255);

                                        match &mask.shape {
                                            MaskShape::Rectangle { x, y, width, height } => {
                                                let mask_rect = egui::Rect::from_min_size(
                                                    slice_rect.min + egui::vec2(
                                                        x * slice_rect.width(),
                                                        y * slice_rect.height(),
                                                    ),
                                                    egui::vec2(
                                                        width * slice_rect.width(),
                                                        height * slice_rect.height(),
                                                    ),
                                                );
                                                ui.painter().rect_stroke(
                                                    mask_rect,
                                                    0.0,
                                                    egui::Stroke::new(2.0, mask_color),
                                                    egui::StrokeKind::Inside,
                                                );
                                            }
                                            MaskShape::Ellipse { center, radius_x, radius_y } => {
                                                let center_pos = slice_rect.min + egui::vec2(
                                                    center.x * slice_rect.width(),
                                                    center.y * slice_rect.height(),
                                                );
                                                // Draw approximate ellipse using line segments
                                                let segments = 32;
                                                let rx = radius_x * slice_rect.width();
                                                let ry = radius_y * slice_rect.height();
                                                for i in 0..segments {
                                                    let a1 = (i as f32 / segments as f32) * std::f32::consts::TAU;
                                                    let a2 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
                                                    let p1 = center_pos + egui::vec2(a1.cos() * rx, a1.sin() * ry);
                                                    let p2 = center_pos + egui::vec2(a2.cos() * rx, a2.sin() * ry);
                                                    ui.painter().line_segment([p1, p2], egui::Stroke::new(2.0, mask_color));
                                                }
                                                // Draw center point
                                                ui.painter().circle_filled(center_pos, 4.0, point_color);
                                            }
                                            MaskShape::Polygon { points } => {
                                                // Draw polygon edges
                                                for i in 0..points.len() {
                                                    let p1 = &points[i];
                                                    let p2 = &points[(i + 1) % points.len()];
                                                    let pos1 = slice_rect.min + egui::vec2(
                                                        p1.x * slice_rect.width(),
                                                        p1.y * slice_rect.height(),
                                                    );
                                                    let pos2 = slice_rect.min + egui::vec2(
                                                        p2.x * slice_rect.width(),
                                                        p2.y * slice_rect.height(),
                                                    );
                                                    ui.painter().line_segment([pos1, pos2], egui::Stroke::new(2.0, mask_color));
                                                }
                                                // Draw vertices as draggable points
                                                for (i, p) in points.iter().enumerate() {
                                                    let pos = slice_rect.min + egui::vec2(
                                                        p.x * slice_rect.width(),
                                                        p.y * slice_rect.height(),
                                                    );
                                                    let is_dragging = self.dragging_mask_vertex == Some(i);
                                                    let radius = if is_dragging { 6.0 } else { 4.0 };
                                                    ui.painter().circle_filled(pos, radius, point_color);
                                                }
                                            }
                                            MaskShape::Bezier { segments } => {
                                                // Draw bezier segments
                                                let bezier_color = mask_color;
                                                for segment in segments {
                                                    // Draw bezier curve using line approximation
                                                    let steps = 16;
                                                    for i in 0..steps {
                                                        let t1 = i as f32 / steps as f32;
                                                        let t2 = (i + 1) as f32 / steps as f32;
                                                        let p1 = segment.evaluate(t1);
                                                        let p2 = segment.evaluate(t2);
                                                        let pos1 = slice_rect.min + egui::vec2(
                                                            p1.x * slice_rect.width(),
                                                            p1.y * slice_rect.height(),
                                                        );
                                                        let pos2 = slice_rect.min + egui::vec2(
                                                            p2.x * slice_rect.width(),
                                                            p2.y * slice_rect.height(),
                                                        );
                                                        ui.painter().line_segment([pos1, pos2], egui::Stroke::new(2.0, bezier_color));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "No screen selected",
                        egui::FontId::proportional(12.0),
                        egui::Color32::DARK_GRAY,
                    );
                }

                // Handle warp point dragging
                if let Some(screen_id) = self.selected_screen {
                    if let Some(slice_id) = self.selected_slice {
                        if let Some(screen) = screens.iter().find(|s| s.id == screen_id) {
                            if let Some(slice) = screen.slices.iter().find(|s| s.id == slice_id) {
                                if let Some(mesh) = &slice.output.mesh {
                                    // Calculate slice rect in preview coordinates
                                    let slice_rect = egui::Rect::from_min_size(
                                        rect.min + egui::vec2(
                                            slice.output.rect.x * preview_size.x,
                                            slice.output.rect.y * preview_size.y,
                                        ),
                                        egui::vec2(
                                            slice.output.rect.width * preview_size.x,
                                            slice.output.rect.height * preview_size.y,
                                        ),
                                    );

                                    // On click start, find nearest warp point
                                    if response.drag_started() {
                                        if let Some(pointer_pos) = response.interact_pointer_pos() {
                                            let mut best_dist = 15.0_f32; // Click radius threshold
                                            let mut best_point: Option<(usize, usize)> = None;

                                            for col in 0..mesh.columns {
                                                for row in 0..mesh.rows {
                                                    if let Some(point) = mesh.get_point(col, row) {
                                                        let pos = slice_rect.min + egui::vec2(
                                                            point.position[0] * slice_rect.width(),
                                                            point.position[1] * slice_rect.height(),
                                                        );
                                                        let dist = pointer_pos.distance(pos);
                                                        if dist < best_dist {
                                                            best_dist = dist;
                                                            best_point = Some((col, row));
                                                        }
                                                    }
                                                }
                                            }

                                            self.dragging_warp_point = best_point;
                                        }
                                    }

                                    // During drag, update point position
                                    if response.dragged() {
                                        if let Some((col, row)) = self.dragging_warp_point {
                                            if let Some(pointer_pos) = response.interact_pointer_pos() {
                                                // Convert pointer to normalized coordinates
                                                let local = pointer_pos - slice_rect.min;
                                                let norm_x = (local.x / slice_rect.width()).clamp(0.0, 1.0);
                                                let norm_y = (local.y / slice_rect.height()).clamp(0.0, 1.0);

                                                // Update the mesh point and trigger action
                                                let mut updated_slice = slice.clone();
                                                if let Some(mesh) = &mut updated_slice.output.mesh {
                                                    mesh.set_point_position(col, row, norm_x, norm_y);
                                                    actions.push(AdvancedOutputAction::UpdateSlice {
                                                        screen_id,
                                                        slice_id,
                                                        slice: updated_slice,
                                                    });
                                                }
                                            }
                                        }
                                    }

                                    // On drag end, clear dragging state
                                    if response.drag_stopped() {
                                        self.dragging_warp_point = None;
                                    }
                                }

                                // Handle mask vertex dragging for polygon masks
                                if let Some(mask) = &slice.mask {
                                    if mask.enabled {
                                        if let MaskShape::Polygon { points } = &mask.shape {
                                            // Calculate slice rect in preview coordinates
                                            let slice_rect = egui::Rect::from_min_size(
                                                rect.min + egui::vec2(
                                                    slice.output.rect.x * preview_size.x,
                                                    slice.output.rect.y * preview_size.y,
                                                ),
                                                egui::vec2(
                                                    slice.output.rect.width * preview_size.x,
                                                    slice.output.rect.height * preview_size.y,
                                                ),
                                            );

                                            // On click start, find nearest mask vertex
                                            if response.drag_started() && self.dragging_warp_point.is_none() {
                                                if let Some(pointer_pos) = response.interact_pointer_pos() {
                                                    let mut best_dist = 15.0_f32; // Click radius threshold
                                                    let mut best_vertex: Option<usize> = None;

                                                    for (i, point) in points.iter().enumerate() {
                                                        let pos = slice_rect.min + egui::vec2(
                                                            point.x * slice_rect.width(),
                                                            point.y * slice_rect.height(),
                                                        );
                                                        let dist = pointer_pos.distance(pos);
                                                        if dist < best_dist {
                                                            best_dist = dist;
                                                            best_vertex = Some(i);
                                                        }
                                                    }

                                                    self.dragging_mask_vertex = best_vertex;
                                                }
                                            }

                                            // During drag, update vertex position
                                            if response.dragged() {
                                                if let Some(vertex_idx) = self.dragging_mask_vertex {
                                                    if let Some(pointer_pos) = response.interact_pointer_pos() {
                                                        // Convert pointer to normalized coordinates
                                                        let local = pointer_pos - slice_rect.min;
                                                        let norm_x = (local.x / slice_rect.width()).clamp(0.0, 1.0);
                                                        let norm_y = (local.y / slice_rect.height()).clamp(0.0, 1.0);

                                                        // Update the mask vertex and trigger action
                                                        let mut updated_slice = slice.clone();
                                                        if let Some(mask) = &mut updated_slice.mask {
                                                            if let MaskShape::Polygon { points } = &mut mask.shape {
                                                                if vertex_idx < points.len() {
                                                                    points[vertex_idx] = MaskPoint2D { x: norm_x, y: norm_y };
                                                                    actions.push(AdvancedOutputAction::UpdateSlice {
                                                                        screen_id,
                                                                        slice_id,
                                                                        slice: updated_slice,
                                                                    });
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            // On drag end, clear dragging state
                                            if response.drag_stopped() {
                                                self.dragging_mask_vertex = None;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                ui.add_space(4.0);
                // Show resolution info
                if let Some(screen_id) = self.selected_screen {
                    if let Some(screen) = screens.iter().find(|s| s.id == screen_id) {
                        ui.label(
                            egui::RichText::new(format!("{}x{}", screen.width, screen.height))
                                .small()
                                .weak(),
                        );
                    }
                }
            });

            ui.separator();

            // RIGHT COLUMN: Properties
            ui.vertical(|ui| {
                ui.set_min_width(200.0);
                ui.heading("Properties");
                ui.add_space(4.0);

                egui::ScrollArea::vertical()
                    .id_salt("properties_scroll")
                    .show(ui, |ui| {
                        if let Some(screen_id) = self.selected_screen {
                            if let Some(screen) = screens.iter().find(|s| s.id == screen_id) {
                                // Show slice properties if a slice is selected
                                if let Some(slice_id) = self.selected_slice {
                                    if let Some(slice) =
                                        screen.slices.iter().find(|s| s.id == slice_id)
                                    {
                                        self.render_slice_properties(
                                            ui,
                                            screen_id,
                                            slice,
                                            layer_count,
                                            actions,
                                        );
                                    }
                                } else {
                                    // Show screen properties
                                    self.render_screen_properties(ui, screen, available_displays, actions);
                                }
                            }
                        } else {
                            ui.label(
                                egui::RichText::new("Select a screen or slice to edit")
                                    .weak()
                                    .italics(),
                            );
                        }
                    });
            });
        });
    }

    /// Render screen properties panel
    fn render_screen_properties(
        &mut self,
        ui: &mut egui::Ui,
        screen: &Screen,
        available_displays: &[DisplayInfo],
        actions: &mut Vec<AdvancedOutputAction>,
    ) {
        let mut changed = false;
        let mut screen_copy = screen.clone();

        ui.label(egui::RichText::new("Screen Properties").strong());
        ui.add_space(8.0);

        // Name
        ui.horizontal(|ui| {
            ui.label("Name:");
            if ui
                .text_edit_singleline(&mut self.temp_screen_name)
                .changed()
            {
                screen_copy.name = self.temp_screen_name.clone();
                changed = true;
            }
        });

        ui.add_space(4.0);

        // Resolution
        ui.label("Resolution:");
        ui.horizontal(|ui| {
            ui.label("W:");
            let width_response = ui.add(
                egui::TextEdit::singleline(&mut self.temp_width)
                    .desired_width(50.0),
            );
            ui.label("H:");
            let height_response = ui.add(
                egui::TextEdit::singleline(&mut self.temp_height)
                    .desired_width(50.0),
            );

            if width_response.lost_focus() || height_response.lost_focus() {
                if let (Ok(w), Ok(h)) =
                    (self.temp_width.parse::<u32>(), self.temp_height.parse::<u32>())
                {
                    if w > 0 && h > 0 && (w != screen.width || h != screen.height) {
                        screen_copy.width = w;
                        screen_copy.height = h;
                        changed = true;
                    }
                }
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Output Device section
        ui.label(egui::RichText::new("Output Device").strong());
        ui.add_space(4.0);

        // Device type selector
        let current_device_type = screen.device.type_name();
        egui::ComboBox::from_id_salt("device_type")
            .selected_text(current_device_type)
            .show_ui(ui, |ui| {
                // Virtual
                if ui.selectable_label(matches!(screen_copy.device, OutputDevice::Virtual), "Virtual").clicked() {
                    screen_copy.device = OutputDevice::Virtual;
                    changed = true;
                }

                // Display (only show if displays are available)
                if !available_displays.is_empty() {
                    if ui.selectable_label(matches!(screen_copy.device, OutputDevice::Display { .. }), "Display").clicked() {
                        // Default to first display
                        screen_copy.device = OutputDevice::Display {
                            display_id: available_displays[0].id,
                        };
                        changed = true;
                    }
                }

                // NDI
                if ui.selectable_label(matches!(screen_copy.device, OutputDevice::Ndi { .. }), "NDI").clicked() {
                    let name = if self.temp_device_name.is_empty() {
                        format!("{} NDI", screen.name)
                    } else {
                        self.temp_device_name.clone()
                    };
                    screen_copy.device = OutputDevice::Ndi { name };
                    self.temp_device_name = if let OutputDevice::Ndi { name } = &screen_copy.device {
                        name.clone()
                    } else {
                        String::new()
                    };
                    changed = true;
                }

                // OMT
                if ui.selectable_label(matches!(screen_copy.device, OutputDevice::Omt { .. }), "OMT").clicked() {
                    let name = if self.temp_device_name.is_empty() {
                        format!("{} OMT", screen.name)
                    } else {
                        self.temp_device_name.clone()
                    };
                    let port = self.temp_omt_port.parse().unwrap_or(5960);
                    screen_copy.device = OutputDevice::Omt { name, port };
                    self.temp_device_name = if let OutputDevice::Omt { name, .. } = &screen_copy.device {
                        name.clone()
                    } else {
                        String::new()
                    };
                    changed = true;
                }

                // Syphon (macOS only)
                #[cfg(target_os = "macos")]
                if ui.selectable_label(matches!(screen_copy.device, OutputDevice::Syphon { .. }), "Syphon").clicked() {
                    let name = if self.temp_device_name.is_empty() {
                        format!("{} Syphon", screen.name)
                    } else {
                        self.temp_device_name.clone()
                    };
                    screen_copy.device = OutputDevice::Syphon { name };
                    self.temp_device_name = if let OutputDevice::Syphon { name } = &screen_copy.device {
                        name.clone()
                    } else {
                        String::new()
                    };
                    changed = true;
                }

                // Spout (Windows only)
                #[cfg(target_os = "windows")]
                if ui.selectable_label(matches!(screen_copy.device, OutputDevice::Spout { .. }), "Spout").clicked() {
                    let name = if self.temp_device_name.is_empty() {
                        format!("{} Spout", screen.name)
                    } else {
                        self.temp_device_name.clone()
                    };
                    screen_copy.device = OutputDevice::Spout { name };
                    self.temp_device_name = if let OutputDevice::Spout { name } = &screen_copy.device {
                        name.clone()
                    } else {
                        String::new()
                    };
                    changed = true;
                }
            });

        ui.add_space(4.0);

        // Device-specific configuration
        // Extract info before rendering to avoid borrow issues in closures
        let (is_virtual, current_display_id, current_name, current_port) = match &screen_copy.device {
            OutputDevice::Virtual => (true, None, None, None),
            OutputDevice::Display { display_id } => (false, Some(*display_id), None, None),
            OutputDevice::Ndi { name } => (false, None, Some(name.clone()), None),
            OutputDevice::Omt { name, port } => (false, None, Some(name.clone()), Some(*port)),
            #[cfg(target_os = "macos")]
            OutputDevice::Syphon { name } => (false, None, Some(name.clone()), None),
            #[cfg(target_os = "windows")]
            OutputDevice::Spout { name } => (false, None, Some(name.clone()), None),
        };

        if is_virtual {
            ui.label(egui::RichText::new("Preview only (no output)").weak().italics());
        } else if let Some(display_id) = current_display_id {
            // Display selector dropdown
            let current_display = available_displays.iter().find(|d| d.id == display_id);
            let is_disconnected = current_display.is_none();
            let display_label = current_display
                .map(|d| d.label())
                .unwrap_or_else(|| format!("Display {} (disconnected)", display_id));

            ui.horizontal(|ui| {
                ui.label("Monitor:");
                egui::ComboBox::from_id_salt("display_selector")
                    .selected_text(&display_label)
                    .show_ui(ui, |ui| {
                        for display in available_displays {
                            if ui.selectable_label(display.id == display_id, display.label()).clicked() {
                                screen_copy.device = OutputDevice::Display { display_id: display.id };
                                changed = true;
                            }
                        }
                    });
            });

            // Show warning if display is disconnected
            if is_disconnected {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("⚠").color(egui::Color32::YELLOW));
                    ui.label(
                        egui::RichText::new("Display disconnected - output paused")
                            .color(egui::Color32::YELLOW)
                            .italics()
                    );
                });
                ui.horizontal(|ui| {
                    if ui.button("Switch to Virtual").clicked() {
                        screen_copy.device = OutputDevice::Virtual;
                        changed = true;
                    }
                });
            }
        } else if matches!(screen_copy.device, OutputDevice::Ndi { .. }) {
            // NDI name editor
            if let Some(ref name) = current_name {
                if self.temp_device_name.is_empty() || !self.temp_device_name.eq(name) {
                    self.temp_device_name = name.clone();
                }
            }
            ui.horizontal(|ui| {
                ui.label("Name:");
                if ui.text_edit_singleline(&mut self.temp_device_name).changed() {
                    screen_copy.device = OutputDevice::Ndi {
                        name: self.temp_device_name.clone(),
                    };
                    changed = true;
                }
            });
        } else if matches!(screen_copy.device, OutputDevice::Omt { .. }) {
            // OMT name and port editor
            if let Some(ref name) = current_name {
                if self.temp_device_name.is_empty() || !self.temp_device_name.eq(name) {
                    self.temp_device_name = name.clone();
                }
            }
            if let Some(port) = current_port {
                if self.temp_omt_port.is_empty() {
                    self.temp_omt_port = port.to_string();
                }
            }
            ui.horizontal(|ui| {
                ui.label("Name:");
                if ui.text_edit_singleline(&mut self.temp_device_name).changed() {
                    screen_copy.device = OutputDevice::Omt {
                        name: self.temp_device_name.clone(),
                        port: self.temp_omt_port.parse().unwrap_or(5960),
                    };
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Port:");
                if ui.add(egui::TextEdit::singleline(&mut self.temp_omt_port).desired_width(60.0)).changed() {
                    if let Ok(p) = self.temp_omt_port.parse::<u16>() {
                        screen_copy.device = OutputDevice::Omt {
                            name: self.temp_device_name.clone(),
                            port: p,
                        };
                        changed = true;
                    }
                }
            });
        }
        #[cfg(target_os = "macos")]
        if matches!(screen_copy.device, OutputDevice::Syphon { .. }) {
            // Syphon name editor
            if let Some(ref name) = current_name {
                if self.temp_device_name.is_empty() || !self.temp_device_name.eq(name) {
                    self.temp_device_name = name.clone();
                }
            }
            ui.horizontal(|ui| {
                ui.label("Name:");
                if ui.text_edit_singleline(&mut self.temp_device_name).changed() {
                    screen_copy.device = OutputDevice::Syphon {
                        name: self.temp_device_name.clone(),
                    };
                    changed = true;
                }
            });
        }
        #[cfg(target_os = "windows")]
        if matches!(screen_copy.device, OutputDevice::Spout { .. }) {
            // Spout name editor
            if let Some(ref name) = current_name {
                if self.temp_device_name.is_empty() || !self.temp_device_name.eq(name) {
                    self.temp_device_name = name.clone();
                }
            }
            ui.horizontal(|ui| {
                ui.label("Name:");
                if ui.text_edit_singleline(&mut self.temp_device_name).changed() {
                    screen_copy.device = OutputDevice::Spout {
                        name: self.temp_device_name.clone(),
                    };
                    changed = true;
                }
            });
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Timing section (for projector sync)
        ui.label(egui::RichText::new("Timing").strong());
        ui.add_space(4.0);

        // Frame delay slider
        ui.horizontal(|ui| {
            ui.label("Delay:");

            // Convert to i32 for slider, then back to u32
            let mut delay_val = screen_copy.delay_ms as i32;
            let slider = egui::Slider::new(&mut delay_val, 0..=500)
                .suffix(" ms")
                .clamping(egui::SliderClamping::Always);

            if ui.add(slider).changed() {
                screen_copy.delay_ms = delay_val.max(0) as u32;
                changed = true;
            }

            // Show frame count at 60fps as reference
            let frames_at_60 = (screen_copy.delay_ms as f32 * 60.0 / 1000.0).round() as u32;
            if frames_at_60 > 0 {
                ui.label(format!("({} frames @ 60fps)", frames_at_60))
                    .on_hover_text("Number of frames of delay at 60 FPS");
            }
        });

        if screen_copy.delay_ms > 0 {
            ui.label(egui::RichText::new("Delay active - output will lag behind preview").weak().italics());
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Color Correction section
        ui.horizontal(|ui| {
            ui.label("Color Correction");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Reset").on_hover_text("Reset color to defaults").clicked() {
                    screen_copy.color = crate::output::OutputColorCorrection::default();
                    changed = true;
                }
            });
        });
        ui.add_space(4.0);

        // Brightness slider
        ui.horizontal(|ui| {
            ui.label("Brightness:");
            if ui.add(egui::Slider::new(&mut screen_copy.color.brightness, -1.0..=1.0).max_decimals(2)).changed() {
                changed = true;
            }
        });

        // Contrast slider
        ui.horizontal(|ui| {
            ui.label("Contrast:");
            if ui.add(egui::Slider::new(&mut screen_copy.color.contrast, 0.0..=2.0).max_decimals(2)).changed() {
                changed = true;
            }
        });

        // Gamma slider
        ui.horizontal(|ui| {
            ui.label("Gamma:");
            if ui.add(egui::Slider::new(&mut screen_copy.color.gamma, 0.1..=4.0).logarithmic(true).max_decimals(2)).changed() {
                changed = true;
            }
        });

        // Saturation slider
        ui.horizontal(|ui| {
            ui.label("Saturation:");
            if ui.add(egui::Slider::new(&mut screen_copy.color.saturation, 0.0..=2.0).max_decimals(2)).changed() {
                changed = true;
            }
        });

        // RGB Channels (collapsing section)
        ui.collapsing("RGB Channels", |ui| {
            ui.horizontal(|ui| {
                ui.label("Red:");
                if ui.add(egui::Slider::new(&mut screen_copy.color.red, 0.0..=2.0).max_decimals(2)).changed() {
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Green:");
                if ui.add(egui::Slider::new(&mut screen_copy.color.green, 0.0..=2.0).max_decimals(2)).changed() {
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Blue:");
                if ui.add(egui::Slider::new(&mut screen_copy.color.blue, 0.0..=2.0).max_decimals(2)).changed() {
                    changed = true;
                }
            });
        });

        // Show indicator if any color correction is applied
        if !screen_copy.color.is_identity() {
            ui.add_space(2.0);
            ui.colored_label(egui::Color32::GREEN, "(color modified)");
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Enabled toggle
        let mut enabled = screen_copy.enabled;
        if ui.checkbox(&mut enabled, "Enabled").changed() {
            screen_copy.enabled = enabled;
            changed = true;
        }

        if changed {
            actions.push(AdvancedOutputAction::UpdateScreen {
                screen_id: screen.id,
                screen: screen_copy,
            });
        }
    }

    /// Render slice properties panel
    fn render_slice_properties(
        &mut self,
        ui: &mut egui::Ui,
        screen_id: ScreenId,
        slice: &Slice,
        layer_count: usize,
        actions: &mut Vec<AdvancedOutputAction>,
    ) {
        let mut changed = false;
        let mut slice_copy = slice.clone();

        ui.label(egui::RichText::new("Slice Properties").strong());
        ui.add_space(8.0);

        // Name
        ui.horizontal(|ui| {
            ui.label("Name:");
            if ui
                .text_edit_singleline(&mut self.temp_slice_name)
                .changed()
            {
                slice_copy.name = self.temp_slice_name.clone();
                changed = true;
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Input source
        ui.label("Input Source");
        ui.add_space(4.0);

        let current_input = match &slice.input {
            SliceInput::Composition => 0,
            SliceInput::Layer { layer_id } => *layer_id as usize + 1,
        };

        egui::ComboBox::from_id_salt("slice_input")
            .selected_text(match &slice.input {
                SliceInput::Composition => "Composition".to_string(),
                SliceInput::Layer { layer_id } => format!("Layer {}", layer_id),
            })
            .show_ui(ui, |ui| {
                if ui
                    .selectable_value(&mut slice_copy.input, SliceInput::Composition, "Composition")
                    .clicked()
                {
                    changed = true;
                }
                for i in 0..layer_count {
                    let layer_input = SliceInput::Layer { layer_id: i as u32 };
                    if ui
                        .selectable_label(current_input == i + 1, format!("Layer {}", i))
                        .clicked()
                    {
                        slice_copy.input = layer_input;
                        changed = true;
                    }
                }
            });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Input rect (crop from source)
        ui.horizontal(|ui| {
            ui.label("Input Rect (crop)");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Preset buttons
                if ui.small_button("Full").on_hover_text("Reset to full input (0,0,1,1)").clicked() {
                    slice_copy.input_rect.x = 0.0;
                    slice_copy.input_rect.y = 0.0;
                    slice_copy.input_rect.width = 1.0;
                    slice_copy.input_rect.height = 1.0;
                    changed = true;
                }
                if ui.small_button("Match Output").on_hover_text("Copy output rect to input rect").clicked() {
                    slice_copy.input_rect.x = slice_copy.output.rect.x;
                    slice_copy.input_rect.y = slice_copy.output.rect.y;
                    slice_copy.input_rect.width = slice_copy.output.rect.width;
                    slice_copy.input_rect.height = slice_copy.output.rect.height;
                    changed = true;
                }
            });
        });
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("X:");
            let mut x = slice_copy.input_rect.x;
            if ui
                .add(egui::DragValue::new(&mut x).range(0.0..=1.0).speed(0.01))
                .changed()
            {
                slice_copy.input_rect.x = x;
                changed = true;
            }
            ui.label("Y:");
            let mut y = slice_copy.input_rect.y;
            if ui
                .add(egui::DragValue::new(&mut y).range(0.0..=1.0).speed(0.01))
                .changed()
            {
                slice_copy.input_rect.y = y;
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("W:");
            let mut w = slice_copy.input_rect.width;
            if ui
                .add(egui::DragValue::new(&mut w).range(0.0..=1.0).speed(0.01))
                .changed()
            {
                slice_copy.input_rect.width = w;
                changed = true;
            }
            ui.label("H:");
            let mut h = slice_copy.input_rect.height;
            if ui
                .add(egui::DragValue::new(&mut h).range(0.0..=1.0).speed(0.01))
                .changed()
            {
                slice_copy.input_rect.height = h;
                changed = true;
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Output rect (position on screen)
        ui.horizontal(|ui| {
            ui.label("Output Rect (position)");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Preset buttons
                if ui.small_button("Full").on_hover_text("Fill entire screen (0,0,1,1)").clicked() {
                    slice_copy.output.rect.x = 0.0;
                    slice_copy.output.rect.y = 0.0;
                    slice_copy.output.rect.width = 1.0;
                    slice_copy.output.rect.height = 1.0;
                    changed = true;
                }
                if ui.small_button("Match Input").on_hover_text("Copy input rect to output rect").clicked() {
                    slice_copy.output.rect.x = slice_copy.input_rect.x;
                    slice_copy.output.rect.y = slice_copy.input_rect.y;
                    slice_copy.output.rect.width = slice_copy.input_rect.width;
                    slice_copy.output.rect.height = slice_copy.input_rect.height;
                    changed = true;
                }
            });
        });
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("X:");
            let mut x = slice_copy.output.rect.x;
            if ui
                .add(egui::DragValue::new(&mut x).range(0.0..=1.0).speed(0.01))
                .changed()
            {
                slice_copy.output.rect.x = x;
                changed = true;
            }
            ui.label("Y:");
            let mut y = slice_copy.output.rect.y;
            if ui
                .add(egui::DragValue::new(&mut y).range(0.0..=1.0).speed(0.01))
                .changed()
            {
                slice_copy.output.rect.y = y;
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("W:");
            let mut w = slice_copy.output.rect.width;
            if ui
                .add(egui::DragValue::new(&mut w).range(0.0..=1.0).speed(0.01))
                .changed()
            {
                slice_copy.output.rect.width = w;
                changed = true;
            }
            ui.label("H:");
            let mut h = slice_copy.output.rect.height;
            if ui
                .add(egui::DragValue::new(&mut h).range(0.0..=1.0).speed(0.01))
                .changed()
            {
                slice_copy.output.rect.height = h;
                changed = true;
            }
        });

        ui.add_space(8.0);

        // Rotation
        ui.horizontal(|ui| {
            ui.label("Rotation:");
            let mut rotation_deg = slice_copy.output.rotation.to_degrees();
            if ui
                .add(
                    egui::DragValue::new(&mut rotation_deg)
                        .range(-180.0..=180.0)
                        .speed(1.0)
                        .suffix("°"),
                )
                .changed()
            {
                slice_copy.output.rotation = rotation_deg.to_radians();
                changed = true;
            }
        });

        ui.add_space(4.0);

        // Flip toggles
        ui.horizontal(|ui| {
            let mut flip_h = slice_copy.output.flip_h;
            if ui.checkbox(&mut flip_h, "Flip H").changed() {
                slice_copy.output.flip_h = flip_h;
                changed = true;
            }
            let mut flip_v = slice_copy.output.flip_v;
            if ui.checkbox(&mut flip_v, "Flip V").changed() {
                slice_copy.output.flip_v = flip_v;
                changed = true;
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Perspective warp
        ui.horizontal(|ui| {
            ui.label("Perspective Warp");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Reset button
                if ui.small_button("Reset").on_hover_text("Reset corners to identity").clicked() {
                    slice_copy.output.perspective = None;
                    changed = true;
                }
            });
        });
        ui.add_space(4.0);

        // Get current perspective or default identity
        let has_perspective = slice_copy.output.perspective.is_some();
        let corners = slice_copy.output.perspective.unwrap_or([
            Point2D { x: 0.0, y: 0.0 },  // TL
            Point2D { x: 1.0, y: 0.0 },  // TR
            Point2D { x: 1.0, y: 1.0 },  // BR
            Point2D { x: 0.0, y: 1.0 },  // BL
        ]);

        // Enable checkbox
        let mut perspective_enabled = has_perspective;
        if ui.checkbox(&mut perspective_enabled, "Enable").changed() {
            if perspective_enabled {
                // Enable with identity corners
                slice_copy.output.perspective = Some([
                    Point2D { x: 0.0, y: 0.0 },
                    Point2D { x: 1.0, y: 0.0 },
                    Point2D { x: 1.0, y: 1.0 },
                    Point2D { x: 0.0, y: 1.0 },
                ]);
            } else {
                slice_copy.output.perspective = None;
            }
            changed = true;
        }

        // Corner controls (only show if enabled)
        if has_perspective {
            let mut new_corners = corners;

            ui.add_space(4.0);
            // Top-left
            ui.horizontal(|ui| {
                ui.label("TL:");
                let mut x = new_corners[0].x;
                let mut y = new_corners[0].y;
                if ui.add(egui::DragValue::new(&mut x).range(-0.5..=1.5).speed(0.01).prefix("x:")).changed() {
                    new_corners[0].x = x;
                    changed = true;
                }
                if ui.add(egui::DragValue::new(&mut y).range(-0.5..=1.5).speed(0.01).prefix("y:")).changed() {
                    new_corners[0].y = y;
                    changed = true;
                }
            });

            // Top-right
            ui.horizontal(|ui| {
                ui.label("TR:");
                let mut x = new_corners[1].x;
                let mut y = new_corners[1].y;
                if ui.add(egui::DragValue::new(&mut x).range(-0.5..=1.5).speed(0.01).prefix("x:")).changed() {
                    new_corners[1].x = x;
                    changed = true;
                }
                if ui.add(egui::DragValue::new(&mut y).range(-0.5..=1.5).speed(0.01).prefix("y:")).changed() {
                    new_corners[1].y = y;
                    changed = true;
                }
            });

            // Bottom-right
            ui.horizontal(|ui| {
                ui.label("BR:");
                let mut x = new_corners[2].x;
                let mut y = new_corners[2].y;
                if ui.add(egui::DragValue::new(&mut x).range(-0.5..=1.5).speed(0.01).prefix("x:")).changed() {
                    new_corners[2].x = x;
                    changed = true;
                }
                if ui.add(egui::DragValue::new(&mut y).range(-0.5..=1.5).speed(0.01).prefix("y:")).changed() {
                    new_corners[2].y = y;
                    changed = true;
                }
            });

            // Bottom-left
            ui.horizontal(|ui| {
                ui.label("BL:");
                let mut x = new_corners[3].x;
                let mut y = new_corners[3].y;
                if ui.add(egui::DragValue::new(&mut x).range(-0.5..=1.5).speed(0.01).prefix("x:")).changed() {
                    new_corners[3].x = x;
                    changed = true;
                }
                if ui.add(egui::DragValue::new(&mut y).range(-0.5..=1.5).speed(0.01).prefix("y:")).changed() {
                    new_corners[3].y = y;
                    changed = true;
                }
            });

            if changed && perspective_enabled {
                slice_copy.output.perspective = Some(new_corners);
            }
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Mesh Warp
        ui.horizontal(|ui| {
            ui.label("Mesh Warp");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Reset button
                if ui.small_button("Reset").on_hover_text("Reset mesh to identity").clicked() {
                    if let Some(mesh) = &mut slice_copy.output.mesh {
                        mesh.reset();
                        changed = true;
                    }
                }
            });
        });
        ui.add_space(4.0);

        // Get current mesh state
        let has_mesh = slice_copy.output.mesh.is_some();

        // Enable checkbox
        let mut mesh_enabled = has_mesh;
        if ui.checkbox(&mut mesh_enabled, "Enable").changed() {
            if mesh_enabled {
                // Enable with default 4×4 grid
                slice_copy.output.mesh = Some(WarpMesh::new(4, 4));
            } else {
                slice_copy.output.mesh = None;
            }
            changed = true;
        }

        // Mesh controls (only show if enabled)
        if has_mesh {
            ui.add_space(4.0);

            // Grid size selector
            let current_size = if let Some(mesh) = &slice_copy.output.mesh {
                format!("{}×{}", mesh.columns, mesh.rows)
            } else {
                "4×4".to_string()
            };

            ui.horizontal(|ui| {
                ui.label("Grid:");
                egui::ComboBox::from_id_salt("mesh_grid_size")
                    .selected_text(&current_size)
                    .show_ui(ui, |ui| {
                        for (cols, rows) in [(4, 4), (8, 8), (16, 16)] {
                            let label = format!("{}×{}", cols, rows);
                            if ui.selectable_label(current_size == label, &label).clicked() {
                                if let Some(mesh) = &mut slice_copy.output.mesh {
                                    mesh.resize(cols, rows);
                                    changed = true;
                                }
                            }
                        }
                    });
            });

            // Show grid info
            if let Some(mesh) = &slice_copy.output.mesh {
                ui.add_space(4.0);
                ui.label(format!("{} control points", mesh.points.len()));

                // Show if mesh has any deformation
                if mesh.is_identity() {
                    ui.colored_label(egui::Color32::GRAY, "(no deformation)");
                } else {
                    ui.colored_label(egui::Color32::GREEN, "(modified)");
                }
            }
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Edge Blend
        ui.horizontal(|ui| {
            ui.label("Edge Blend");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Disable All button
                if ui.small_button("Disable All").on_hover_text("Disable all edge blending").clicked() {
                    slice_copy.output.edge_blend.disable_all();
                    changed = true;
                }
            });
        });
        ui.add_space(4.0);

        // Check if any blending is enabled
        let has_blend = slice_copy.output.edge_blend.is_any_enabled();

        // Helper function to show edge controls
        fn edge_row(
            ui: &mut egui::Ui,
            label: &str,
            enabled: &mut bool,
            width: &mut f32,
            gamma: &mut f32,
            changed: &mut bool,
        ) {
            ui.horizontal(|ui| {
                if ui.checkbox(enabled, label).changed() {
                    *changed = true;
                }
                if *enabled {
                    ui.add(egui::DragValue::new(width)
                        .range(0.0..=0.5)
                        .speed(0.005)
                        .suffix(" W")
                        .max_decimals(2));
                    ui.add(egui::DragValue::new(gamma)
                        .range(0.1..=4.0)
                        .speed(0.05)
                        .suffix(" γ")
                        .max_decimals(1));
                }
            });
        }

        // Edge blend controls
        {
            let edge = &mut slice_copy.output.edge_blend;

            edge_row(ui, "Left", &mut edge.left.enabled, &mut edge.left.width, &mut edge.left.gamma, &mut changed);
            edge_row(ui, "Right", &mut edge.right.enabled, &mut edge.right.width, &mut edge.right.gamma, &mut changed);
            edge_row(ui, "Top", &mut edge.top.enabled, &mut edge.top.width, &mut edge.top.gamma, &mut changed);
            edge_row(ui, "Bottom", &mut edge.bottom.enabled, &mut edge.bottom.width, &mut edge.bottom.gamma, &mut changed);
        }

        // Preset buttons
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.small_button("Horizontal 15%").on_hover_text("Enable left/right edges at 15%").clicked() {
                slice_copy.output.edge_blend = EdgeBlendConfig::horizontal(0.15, 2.2);
                changed = true;
            }
            if ui.small_button("All 15%").on_hover_text("Enable all edges at 15%").clicked() {
                slice_copy.output.edge_blend = EdgeBlendConfig::all(0.15, 2.2);
                changed = true;
            }
        });

        // Black level compensation (show only if any blending enabled)
        if has_blend {
            ui.add_space(4.0);
            ui.collapsing("Black Level Compensation", |ui| {
                let edge = &mut slice_copy.output.edge_blend;

                ui.horizontal(|ui| {
                    ui.label("Left:");
                    if ui.add(egui::Slider::new(&mut edge.left.black_level, 0.0..=0.5).max_decimals(2)).changed() {
                        changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Right:");
                    if ui.add(egui::Slider::new(&mut edge.right.black_level, 0.0..=0.5).max_decimals(2)).changed() {
                        changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Top:");
                    if ui.add(egui::Slider::new(&mut edge.top.black_level, 0.0..=0.5).max_decimals(2)).changed() {
                        changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Bottom:");
                    if ui.add(egui::Slider::new(&mut edge.bottom.black_level, 0.0..=0.5).max_decimals(2)).changed() {
                        changed = true;
                    }
                });
            });
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Mask section
        ui.horizontal(|ui| {
            ui.label("Mask");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if slice_copy.mask.is_some() {
                    if ui.small_button("Remove").on_hover_text("Remove mask").clicked() {
                        slice_copy.mask = None;
                        changed = true;
                    }
                }
            });
        });
        ui.add_space(4.0);

        if slice_copy.mask.is_none() {
            // Show preset buttons to add a mask
            ui.horizontal(|ui| {
                if ui.small_button("Rectangle").on_hover_text("Add rectangle mask").clicked() {
                    slice_copy.mask = Some(SliceMask {
                        shape: MaskShape::Rectangle {
                            x: 0.1,
                            y: 0.1,
                            width: 0.8,
                            height: 0.8,
                        },
                        feather: 0.0,
                        inverted: false,
                        enabled: true,
                    });
                    changed = true;
                }
                if ui.small_button("Ellipse").on_hover_text("Add ellipse mask").clicked() {
                    slice_copy.mask = Some(SliceMask {
                        shape: MaskShape::Ellipse {
                            center: MaskPoint2D { x: 0.5, y: 0.5 },
                            radius_x: 0.4,
                            radius_y: 0.4,
                        },
                        feather: 0.0,
                        inverted: false,
                        enabled: true,
                    });
                    changed = true;
                }
                if ui.small_button("Polygon").on_hover_text("Add polygon mask").clicked() {
                    slice_copy.mask = Some(SliceMask {
                        shape: MaskShape::Polygon {
                            points: vec![
                                MaskPoint2D { x: 0.2, y: 0.2 },
                                MaskPoint2D { x: 0.8, y: 0.2 },
                                MaskPoint2D { x: 0.8, y: 0.8 },
                                MaskPoint2D { x: 0.2, y: 0.8 },
                            ],
                        },
                        feather: 0.0,
                        inverted: false,
                        enabled: true,
                    });
                    changed = true;
                }
            });
        } else if let Some(mask) = &mut slice_copy.mask {
            // Mask controls
            ui.horizontal(|ui| {
                if ui.checkbox(&mut mask.enabled, "Enabled").changed() {
                    changed = true;
                }
                if ui.checkbox(&mut mask.inverted, "Invert").changed() {
                    changed = true;
                }
            });

            // Feather slider
            ui.horizontal(|ui| {
                ui.label("Feather:");
                if ui.add(egui::Slider::new(&mut mask.feather, 0.0..=0.1).max_decimals(3)).changed() {
                    changed = true;
                }
            });

            ui.add_space(4.0);

            // Shape-specific controls
            match &mut mask.shape {
                MaskShape::Rectangle { x, y, width, height } => {
                    ui.label(egui::RichText::new("Rectangle").weak());
                    ui.horizontal(|ui| {
                        ui.label("X:");
                        if ui.add(egui::DragValue::new(x).range(0.0..=1.0).speed(0.01)).changed() {
                            changed = true;
                        }
                        ui.label("Y:");
                        if ui.add(egui::DragValue::new(y).range(0.0..=1.0).speed(0.01)).changed() {
                            changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("W:");
                        if ui.add(egui::DragValue::new(width).range(0.0..=1.0).speed(0.01)).changed() {
                            changed = true;
                        }
                        ui.label("H:");
                        if ui.add(egui::DragValue::new(height).range(0.0..=1.0).speed(0.01)).changed() {
                            changed = true;
                        }
                    });
                }
                MaskShape::Ellipse { center, radius_x, radius_y } => {
                    ui.label(egui::RichText::new("Ellipse").weak());
                    ui.horizontal(|ui| {
                        ui.label("Center X:");
                        if ui.add(egui::DragValue::new(&mut center.x).range(0.0..=1.0).speed(0.01)).changed() {
                            changed = true;
                        }
                        ui.label("Y:");
                        if ui.add(egui::DragValue::new(&mut center.y).range(0.0..=1.0).speed(0.01)).changed() {
                            changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Radius X:");
                        if ui.add(egui::DragValue::new(radius_x).range(0.0..=1.0).speed(0.01)).changed() {
                            changed = true;
                        }
                        ui.label("Y:");
                        if ui.add(egui::DragValue::new(radius_y).range(0.0..=1.0).speed(0.01)).changed() {
                            changed = true;
                        }
                    });
                }
                MaskShape::Polygon { points } => {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("Polygon ({} vertices)", points.len())).weak());
                        if ui.small_button("Add").on_hover_text("Add vertex at center").clicked() {
                            // Add a new point at the center of the polygon
                            if !points.is_empty() {
                                let cx: f32 = points.iter().map(|p| p.x).sum::<f32>() / points.len() as f32;
                                let cy: f32 = points.iter().map(|p| p.y).sum::<f32>() / points.len() as f32;
                                points.push(MaskPoint2D { x: cx, y: cy });
                                changed = true;
                            }
                        }
                    });
                    ui.label(egui::RichText::new("Drag points in preview to edit").italics().weak());
                }
                MaskShape::Bezier { segments } => {
                    ui.label(egui::RichText::new(format!("Bezier ({} segments)", segments.len())).weak());
                    ui.label(egui::RichText::new("Bezier editing coming soon").italics().weak());
                }
            }
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Color Correction section
        ui.horizontal(|ui| {
            ui.label("Color");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Reset").on_hover_text("Reset color to defaults").clicked() {
                    slice_copy.color = crate::output::SliceColorCorrection::default();
                    changed = true;
                }
            });
        });
        ui.add_space(4.0);

        // Opacity slider (most commonly used)
        ui.horizontal(|ui| {
            ui.label("Opacity:");
            if ui.add(egui::Slider::new(&mut slice_copy.color.opacity, 0.0..=1.0).max_decimals(2)).changed() {
                changed = true;
            }
        });

        // Brightness slider
        ui.horizontal(|ui| {
            ui.label("Brightness:");
            if ui.add(egui::Slider::new(&mut slice_copy.color.brightness, -1.0..=1.0).max_decimals(2)).changed() {
                changed = true;
            }
        });

        // Contrast slider
        ui.horizontal(|ui| {
            ui.label("Contrast:");
            if ui.add(egui::Slider::new(&mut slice_copy.color.contrast, 0.0..=2.0).max_decimals(2)).changed() {
                changed = true;
            }
        });

        // Gamma slider
        ui.horizontal(|ui| {
            ui.label("Gamma:");
            if ui.add(egui::Slider::new(&mut slice_copy.color.gamma, 0.1..=4.0).logarithmic(true).max_decimals(2)).changed() {
                changed = true;
            }
        });

        // RGB Channels (collapsing section for less common adjustments)
        ui.collapsing("RGB Channels", |ui| {
            ui.horizontal(|ui| {
                ui.label("Red:");
                if ui.add(egui::Slider::new(&mut slice_copy.color.red, 0.0..=2.0).max_decimals(2)).changed() {
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Green:");
                if ui.add(egui::Slider::new(&mut slice_copy.color.green, 0.0..=2.0).max_decimals(2)).changed() {
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Blue:");
                if ui.add(egui::Slider::new(&mut slice_copy.color.blue, 0.0..=2.0).max_decimals(2)).changed() {
                    changed = true;
                }
            });
        });

        // Show indicator if any color correction is applied
        if !slice_copy.color.is_identity() {
            ui.add_space(2.0);
            ui.colored_label(egui::Color32::GREEN, "(color modified)");
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Enabled toggle
        let mut enabled = slice_copy.enabled;
        if ui.checkbox(&mut enabled, "Enabled").changed() {
            slice_copy.enabled = enabled;
            changed = true;
        }

        if changed {
            actions.push(AdvancedOutputAction::UpdateSlice {
                screen_id,
                slice_id: slice.id,
                slice: slice_copy,
            });
        }
    }
}
