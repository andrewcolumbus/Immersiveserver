//! Output editor for visual screen/slice configuration
//!
//! Provides a visual canvas for configuring output screens and slices.
//! Features a tabbed interface with Input Selection and Output Transformation modes.

#![allow(dead_code)]

use crate::output::{OutputManager, Screen};
use crate::video::VideoPlayer;
use eframe::egui::{self, Color32, Pos2, Rect as EguiRect, RichText, Stroke, Vec2};

/// Preview quality mode for screen rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreviewQuality {
    /// Render actual video frames (GPU intensive)
    ActualFrames,
    /// Show animated activity indicator (lightweight)
    #[default]
    ActivityIndicator,
}

/// Editor tab selection (Resolume-style)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorTab {
    /// Input Selection - shows composition with slice input regions
    #[default]
    InputSelection,
    /// Output Transformation - shows display output with warped slices
    OutputTransformation,
}

/// Tool mode for the canvas
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolMode {
    /// Pan/navigate tool
    #[default]
    Pan,
    /// Edit corner points
    EditPoints,
    /// Transform (move/scale/rotate)
    Transform,
}

/// Resize handle positions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeHandle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

/// Output editor UI state
pub struct OutputEditor {
    /// Current editor tab
    pub current_tab: EditorTab,
    /// Current tool mode
    pub tool_mode: ToolMode,
    /// Zoom level
    zoom: f32,
    /// Pan offset
    pan: Vec2,
    /// Show grid overlay
    pub show_grid: bool,
    /// Currently dragging screen index
    dragging_screen: Option<usize>,
    /// Drag start position (canvas coords)
    drag_start_pos: Option<Pos2>,
    /// Original screen position when drag started
    drag_start_screen_pos: Option<(f32, f32)>,
    /// Drag state for corners
    drag_corner: Option<(usize, usize, usize)>, // (screen_idx, slice_idx, corner_idx)
    /// Currently resizing screen
    resizing_screen: Option<usize>,
    /// Which resize handle is being dragged
    resize_handle: Option<ResizeHandle>,
    /// Original screen resolution when resize started
    resize_start_resolution: Option<(u32, u32)>,
    /// Original screen position when resize started (for handles that move origin)
    resize_start_position: Option<(f32, f32)>,
    /// Preview quality mode
    pub preview_quality: PreviewQuality,
    /// Animation time for activity indicator
    animation_time: f32,
    /// Composition size (total canvas)
    pub composition_size: (u32, u32),
}

/// Preset zoom levels
const ZOOM_PRESETS: &[(f32, &str)] = &[
    (0.03, "3%"),
    (0.10, "10%"),
    (0.21, "21%"),
    (0.50, "50%"),
    (1.0, "100%"),
    (2.0, "200%"),
];

impl Default for OutputEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputEditor {
    pub fn new() -> Self {
        Self {
            current_tab: EditorTab::default(),
            tool_mode: ToolMode::default(),
            zoom: 0.21, // Start at 21% like Resolume
            pan: Vec2::ZERO,
            show_grid: true,
            dragging_screen: None,
            drag_start_pos: None,
            drag_start_screen_pos: None,
            drag_corner: None,
            resizing_screen: None,
            resize_handle: None,
            resize_start_resolution: None,
            resize_start_position: None,
            preview_quality: PreviewQuality::default(),
            animation_time: 0.0,
            composition_size: (1920, 1080), // Default 1080p composition
        }
    }

    /// Get current zoom as percentage string
    fn zoom_label(&self) -> String {
        format!("{}%", (self.zoom * 100.0).round() as i32)
    }

    /// Set zoom to nearest preset or specific value
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(0.01, 5.0);
    }

    /// Show the output editor canvas with tabs and toolbar
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        output_manager: &mut OutputManager,
        player: &VideoPlayer,
        selected_screen: &mut Option<usize>,
        selected_slice: &mut Option<usize>,
        show_test_pattern: bool,
    ) {
        // Update animation time
        self.animation_time += ui.input(|i| i.predicted_dt);

        // Top bar with tabs and toolbar
        self.draw_top_bar(ui, output_manager);

        ui.add_space(4.0);

        // Canvas area
        let available_size = ui.available_size();
        let (response, painter) = ui.allocate_painter(available_size, egui::Sense::click_and_drag());
        let canvas_rect = response.rect;

        // Calculate canvas transform
        let transform = CanvasTransform {
            offset: canvas_rect.center().to_vec2() + self.pan,
            scale: self.zoom,
        };

        // Handle pan with middle mouse button or when pan tool is active
        let pan_active = self.tool_mode == ToolMode::Pan;
        if response.dragged_by(egui::PointerButton::Middle) 
            || (pan_active && response.dragged_by(egui::PointerButton::Primary)) 
        {
            self.pan += response.drag_delta();
        }

        // Handle zoom with scroll
        if let Some(hover_pos) = response.hover_pos() {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                let old_zoom = self.zoom;
                self.zoom = (self.zoom * (1.0 + scroll * 0.002)).clamp(0.01, 5.0);
                
                // Zoom toward cursor
                let cursor_rel = hover_pos - canvas_rect.center();
                self.pan += cursor_rel * (1.0 - self.zoom / old_zoom);
            }
        }

        // Handle screen dragging with left mouse button (when not in pan mode)
        if !pan_active {
            self.handle_screen_dragging(&response, &transform, output_manager, selected_screen, selected_slice);
        }

        // Background
        painter.rect_filled(canvas_rect, 0.0, Color32::from_rgb(38, 43, 47));

        // Draw grid
        if self.show_grid {
            self.draw_grid(&painter, canvas_rect);
        }

        // Draw composition boundary
        self.draw_composition_boundary(&painter, &transform, canvas_rect);

        // Draw screens based on current tab
        match self.current_tab {
            EditorTab::InputSelection => {
                self.draw_input_selection_view(&painter, &transform, output_manager, player, selected_screen, selected_slice, show_test_pattern);
            }
            EditorTab::OutputTransformation => {
                self.draw_output_transformation_view(&painter, &transform, output_manager, player, selected_screen, selected_slice, show_test_pattern);
            }
        }

        // Draw composition info label
        self.draw_composition_label(&painter, canvas_rect);

        // Request repaint for animations
        if player.is_playing() || self.preview_quality == PreviewQuality::ActivityIndicator {
            ui.ctx().request_repaint();
        }
    }

    /// Draw the top bar with tabs and toolbar (Resolume-style)
    fn draw_top_bar(&mut self, ui: &mut egui::Ui, _output_manager: &mut OutputManager) {
        ui.horizontal(|ui| {
            ui.set_height(32.0);
            
            // Tab buttons
            let tab_style = |selected: bool| {
                if selected {
                    egui::Button::new(RichText::new("").size(13.0))
                        .fill(Color32::from_rgb(74, 157, 91))
                } else {
                    egui::Button::new(RichText::new("").size(13.0))
                        .fill(Color32::from_gray(50))
                }
            };
            
            // Input Selection tab
            let input_btn = ui.add(
                tab_style(self.current_tab == EditorTab::InputSelection)
                    .min_size(Vec2::new(120.0, 28.0))
            );
            let input_rect = input_btn.rect;
            ui.painter().text(
                input_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Input Selection",
                egui::FontId::proportional(12.0),
                if self.current_tab == EditorTab::InputSelection { Color32::WHITE } else { Color32::from_gray(180) },
            );
            if input_btn.clicked() {
                self.current_tab = EditorTab::InputSelection;
            }
            
            // Output Transformation tab
            let output_btn = ui.add(
                tab_style(self.current_tab == EditorTab::OutputTransformation)
                    .min_size(Vec2::new(140.0, 28.0))
            );
            let output_rect = output_btn.rect;
            ui.painter().text(
                output_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Output Transformation",
                egui::FontId::proportional(12.0),
                if self.current_tab == EditorTab::OutputTransformation { Color32::WHITE } else { Color32::from_gray(180) },
            );
            if output_btn.clicked() {
                self.current_tab = EditorTab::OutputTransformation;
            }
            
            ui.add_space(20.0);
            
            // Composition label
            ui.label(
                RichText::new(format!("Composition {}×{}", self.composition_size.0, self.composition_size.1))
                    .size(11.0)
                    .color(Color32::from_gray(150))
            );
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Tool buttons on the right (for Output Transformation tab)
                if self.current_tab == EditorTab::OutputTransformation {
                    // Transform button
                    let transform_selected = self.tool_mode == ToolMode::Transform;
                    if ui.add(
                        egui::Button::new(RichText::new("⊡ Transform").size(11.0))
                            .fill(if transform_selected { Color32::from_rgb(74, 157, 91) } else { Color32::from_gray(50) })
                    ).clicked() {
                        self.tool_mode = ToolMode::Transform;
                    }
                    
                    // Edit Points button
                    let edit_selected = self.tool_mode == ToolMode::EditPoints;
                    if ui.add(
                        egui::Button::new(RichText::new("+ Edit Points").size(11.0))
                            .fill(if edit_selected { Color32::from_rgb(74, 157, 91) } else { Color32::from_gray(50) })
                    ).clicked() {
                        self.tool_mode = ToolMode::EditPoints;
                    }
                    
                    ui.add_space(10.0);
                }
                
                // Grid toggle
                let grid_icon = if self.show_grid { "⊞" } else { "⊟" };
                if ui.add(
                    egui::Button::new(RichText::new(grid_icon).size(14.0))
                        .fill(if self.show_grid { Color32::from_rgb(74, 157, 91) } else { Color32::from_gray(50) })
                        .min_size(Vec2::new(28.0, 28.0))
                ).clicked() {
                    self.show_grid = !self.show_grid;
                }
                
                // Tag/label tool button
                if ui.add(
                    egui::Button::new(RichText::new("🏷").size(14.0))
                        .fill(Color32::from_rgb(74, 157, 91))
                        .min_size(Vec2::new(28.0, 28.0))
                ).on_hover_text("Show labels").clicked() {
                    // Toggle labels
                }
                
                // Pan tool button
                let pan_selected = self.tool_mode == ToolMode::Pan;
                if ui.add(
                    egui::Button::new(RichText::new("✋").size(14.0))
                        .fill(if pan_selected { Color32::from_rgb(74, 157, 91) } else { Color32::from_gray(50) })
                        .min_size(Vec2::new(28.0, 28.0))
                ).on_hover_text("Pan tool").clicked() {
                    self.tool_mode = ToolMode::Pan;
                }
                
                // Redo button
                ui.add_enabled_ui(false, |ui| {
                    ui.add(
                        egui::Button::new(RichText::new("↷").size(14.0))
                            .fill(Color32::from_gray(40))
                            .min_size(Vec2::new(28.0, 28.0))
                    );
                });
                
                // Undo button
                ui.add_enabled_ui(false, |ui| {
                    ui.add(
                        egui::Button::new(RichText::new("↶").size(14.0))
                            .fill(Color32::from_gray(40))
                            .min_size(Vec2::new(28.0, 28.0))
                    );
                });
                
                // Zoom dropdown
                ui.menu_button(
                    RichText::new(format!("{} ▼", self.zoom_label())).size(11.0),
                    |ui| {
                        for &(zoom, label) in ZOOM_PRESETS {
                            if ui.selectable_label((self.zoom - zoom).abs() < 0.01, label).clicked() {
                                self.set_zoom(zoom);
                                ui.close_menu();
                            }
                        }
                        ui.separator();
                        if ui.button("Fit").clicked() {
                            // Calculate fit zoom
                            self.zoom = 0.1;
                            self.pan = Vec2::ZERO;
                            ui.close_menu();
                        }
                    }
                );
            });
        });
    }

    /// Draw the composition boundary rectangle
    fn draw_composition_boundary(&self, painter: &egui::Painter, transform: &CanvasTransform, _canvas_rect: EguiRect) {
        let comp_rect = EguiRect::from_min_size(
            Pos2::new(
                transform.offset.x,
                transform.offset.y,
            ),
            Vec2::new(
                self.composition_size.0 as f32 * transform.scale,
                self.composition_size.1 as f32 * transform.scale,
            ),
        );
        
        // Draw composition background (slightly lighter than canvas)
        painter.rect_filled(comp_rect, 0.0, Color32::from_rgb(45, 50, 55));
        
        // Draw composition border
        painter.rect_stroke(comp_rect, 0.0, Stroke::new(1.0, Color32::from_gray(80)));
    }

    /// Draw composition info label
    fn draw_composition_label(&self, painter: &egui::Painter, canvas_rect: EguiRect) {
        let label = format!("{}×{}", self.composition_size.0, self.composition_size.1);
        painter.text(
            Pos2::new(canvas_rect.max.x - 10.0, canvas_rect.max.y - 10.0),
            egui::Align2::RIGHT_BOTTOM,
            &label,
            egui::FontId::proportional(10.0),
            Color32::from_gray(100),
        );
    }

    /// Draw input selection view (shows composition with slice input regions)
    fn draw_input_selection_view(
        &self,
        painter: &egui::Painter,
        transform: &CanvasTransform,
        output_manager: &OutputManager,
        player: &VideoPlayer,
        selected_screen: &Option<usize>,
        selected_slice: &Option<usize>,
        show_test_pattern: bool,
    ) {
        // In input selection mode, we show the composition and where each slice samples from
        for (screen_idx, screen) in output_manager.screens.iter().enumerate() {
            let is_selected = *selected_screen == Some(screen_idx);
            let is_dragging = self.dragging_screen == Some(screen_idx);
            self.draw_screen(painter, transform, screen, is_selected, is_dragging, player, show_test_pattern);

            // Draw slices within screen
            for (slice_idx, slice) in screen.slices.iter().enumerate() {
                let is_slice_selected = is_selected && *selected_slice == Some(slice_idx);
                self.draw_slice(painter, transform, screen, slice, is_slice_selected);
            }
        }
    }

    /// Draw output transformation view (shows display output with warped slices)
    fn draw_output_transformation_view(
        &self,
        painter: &egui::Painter,
        transform: &CanvasTransform,
        output_manager: &OutputManager,
        player: &VideoPlayer,
        selected_screen: &Option<usize>,
        selected_slice: &Option<usize>,
        show_test_pattern: bool,
    ) {
        // In output transformation mode, we show the output displays with transformed slices
        for (screen_idx, screen) in output_manager.screens.iter().enumerate() {
            let is_selected = *selected_screen == Some(screen_idx);
            let is_dragging = self.dragging_screen == Some(screen_idx);
            self.draw_screen(painter, transform, screen, is_selected, is_dragging, player, show_test_pattern);

            // Draw slices with corner handles for transformation
            for (slice_idx, slice) in screen.slices.iter().enumerate() {
                let is_slice_selected = is_selected && *selected_slice == Some(slice_idx);
                self.draw_slice_with_handles(painter, transform, screen, slice, is_slice_selected, screen_idx, slice_idx);
            }
        }
    }

    /// Draw a slice with transformation handles (for Output Transformation tab)
    fn draw_slice_with_handles(
        &self,
        painter: &egui::Painter,
        transform: &CanvasTransform,
        screen: &Screen,
        slice: &crate::output::Slice,
        is_selected: bool,
        _screen_idx: usize,
        _slice_idx: usize,
    ) {
        let screen_rect = self.screen_to_canvas_rect(transform, screen);
        
        // Get the output vertices (potentially warped)
        let corners = slice.get_output_vertices();
        
        // Convert to canvas coordinates
        let canvas_corners: Vec<Pos2> = corners.iter().map(|c| {
            Pos2::new(
                screen_rect.min.x + c.x * screen_rect.width(),
                screen_rect.min.y + c.y * screen_rect.height(),
            )
        }).collect();
        
        // Draw slice outline
        let outline_color = if is_selected {
            Color32::from_rgb(100, 200, 200) // Teal like Resolume
        } else {
            Color32::from_rgba_unmultiplied(100, 200, 200, 100)
        };
        
        // Draw the quad outline
        if canvas_corners.len() == 4 {
            for i in 0..4 {
                let next = (i + 1) % 4;
                painter.line_segment(
                    [canvas_corners[i], canvas_corners[next]],
                    Stroke::new(if is_selected { 2.0 } else { 1.0 }, outline_color),
                );
            }
        }
        
        // Draw corner handles (only when selected)
        if is_selected {
            let handle_radius = 8.0;
            for corner in &canvas_corners {
                // Outer circle (teal)
                painter.circle_stroke(
                    *corner,
                    handle_radius,
                    Stroke::new(2.0, Color32::from_rgb(100, 200, 200)),
                );
                // Inner circle
                painter.circle_filled(
                    *corner,
                    handle_radius - 2.0,
                    Color32::from_rgba_unmultiplied(100, 200, 200, 50),
                );
            }
            
            // Draw slice label
            if !canvas_corners.is_empty() {
                let label_pos = Pos2::new(
                    (canvas_corners[2].x + canvas_corners[3].x) / 2.0,
                    canvas_corners[2].y + 15.0,
                );
                painter.text(
                    label_pos,
                    egui::Align2::CENTER_TOP,
                    &slice.name,
                    egui::FontId::proportional(11.0),
                    Color32::from_rgb(100, 200, 200),
                );
            }
        }
    }

    /// Handle screen dragging and resizing logic
    fn handle_screen_dragging(
        &mut self,
        response: &egui::Response,
        transform: &CanvasTransform,
        output_manager: &mut OutputManager,
        selected_screen: &mut Option<usize>,
        selected_slice: &mut Option<usize>,
    ) {
        // Check for hover over resize handles to show cursor
        if let Some(pos) = response.hover_pos() {
            if let Some(screen_idx) = *selected_screen {
                if let Some(screen) = output_manager.screens.get(screen_idx) {
                    let screen_rect = self.screen_to_canvas_rect(transform, screen);
                    for handle in ResizeHandle::all() {
                        if handle.contains(screen_rect, pos) {
                            response.ctx.set_cursor_icon(handle.cursor());
                            break;
                        }
                    }
                }
            }
        }

        // Check for drag start
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                // First check if we're clicking a resize handle on the selected screen
                if let Some(screen_idx) = *selected_screen {
                    if let Some(screen) = output_manager.screens.get(screen_idx) {
                        let screen_rect = self.screen_to_canvas_rect(transform, screen);
                        for handle in ResizeHandle::all() {
                            if handle.contains(screen_rect, pos) {
                                self.resizing_screen = Some(screen_idx);
                                self.resize_handle = Some(*handle);
                                self.drag_start_pos = Some(pos);
                                self.resize_start_resolution = Some(screen.resolution);
                                self.resize_start_position = Some(screen.position);
                                return; // Don't check for screen drag
                            }
                        }
                    }
                }

                // Otherwise, check for clicking on a screen to drag
                for (screen_idx, screen) in output_manager.screens.iter().enumerate() {
                    let screen_rect = self.screen_to_canvas_rect(transform, screen);
                    if screen_rect.contains(pos) {
                        self.dragging_screen = Some(screen_idx);
                        self.drag_start_pos = Some(pos);
                        self.drag_start_screen_pos = Some(screen.position);
                        *selected_screen = Some(screen_idx);
                        *selected_slice = None;
                        break;
                    }
                }
            }
        }

        // Handle ongoing resize
        if response.dragged_by(egui::PointerButton::Primary) && self.resizing_screen.is_some() {
            if let (Some(screen_idx), Some(handle), Some(start_pos), Some(start_res), Some(start_position)) = 
                (self.resizing_screen, self.resize_handle, self.drag_start_pos, 
                 self.resize_start_resolution, self.resize_start_position) 
            {
                if let Some(current_pos) = response.interact_pointer_pos() {
                    let delta = current_pos - start_pos;
                    // Convert canvas delta to screen coordinates
                    let delta_x = (delta.x / transform.scale) as i32;
                    let delta_y = (delta.y / transform.scale) as i32;
                    
                    if let Some(screen) = output_manager.screens.get_mut(screen_idx) {
                        let (mut new_w, mut new_h) = (start_res.0 as i32, start_res.1 as i32);
                        let (mut new_x, mut new_y) = (start_position.0, start_position.1);
                        
                        match handle {
                            ResizeHandle::Right => {
                                new_w += delta_x;
                            }
                            ResizeHandle::Bottom => {
                                new_h += delta_y;
                            }
                            ResizeHandle::BottomRight => {
                                new_w += delta_x;
                                new_h += delta_y;
                            }
                            ResizeHandle::Left => {
                                new_w -= delta_x;
                                new_x += delta_x as f32;
                            }
                            ResizeHandle::Top => {
                                new_h -= delta_y;
                                new_y += delta_y as f32;
                            }
                            ResizeHandle::TopLeft => {
                                new_w -= delta_x;
                                new_h -= delta_y;
                                new_x += delta_x as f32;
                                new_y += delta_y as f32;
                            }
                            ResizeHandle::TopRight => {
                                new_w += delta_x;
                                new_h -= delta_y;
                                new_y += delta_y as f32;
                            }
                            ResizeHandle::BottomLeft => {
                                new_w -= delta_x;
                                new_h += delta_y;
                                new_x += delta_x as f32;
                            }
                        }
                        
                        // Clamp to minimum size
                        new_w = new_w.max(100);
                        new_h = new_h.max(100);
                        
                        screen.resolution = (new_w as u32, new_h as u32);
                        screen.position = (new_x, new_y);
                    }
                }
            }
        }
        // Handle ongoing drag (move)
        else if response.dragged_by(egui::PointerButton::Primary) && self.dragging_screen.is_some() {
            if let (Some(screen_idx), Some(start_pos), Some(start_screen_pos)) = 
                (self.dragging_screen, self.drag_start_pos, self.drag_start_screen_pos) 
            {
                if let Some(current_pos) = response.interact_pointer_pos() {
                    let delta = current_pos - start_pos;
                    // Convert canvas delta to screen coordinates
                    let screen_delta = (delta.x / transform.scale, delta.y / transform.scale);
                    
                    if let Some(screen) = output_manager.screens.get_mut(screen_idx) {
                        screen.position = (
                            start_screen_pos.0 + screen_delta.0,
                            start_screen_pos.1 + screen_delta.1,
                        );
                    }
                }
            }
        }

        // Handle click without drag (selection only)
        if response.clicked() && self.dragging_screen.is_none() && self.resizing_screen.is_none() {
            if let Some(pos) = response.interact_pointer_pos() {
                let mut found = false;
                for (screen_idx, screen) in output_manager.screens.iter().enumerate() {
                    let screen_rect = self.screen_to_canvas_rect(transform, screen);
                    if screen_rect.contains(pos) {
                        *selected_screen = Some(screen_idx);
                        *selected_slice = None;
                        found = true;
                        break;
                    }
                }
                if !found {
                    // Clicked on empty space - deselect
                    *selected_screen = None;
                    *selected_slice = None;
                }
            }
        }

        // Handle drag end
        if response.drag_stopped() {
            self.dragging_screen = None;
            self.resizing_screen = None;
            self.resize_handle = None;
            self.drag_start_pos = None;
            self.drag_start_screen_pos = None;
            self.resize_start_resolution = None;
            self.resize_start_position = None;
        }
    }

    fn draw_grid(&self, painter: &egui::Painter, rect: EguiRect) {
        let grid_size = 50.0 * self.zoom;
        let grid_color = Color32::from_gray(40);
        
        let start_x = ((rect.min.x - self.pan.x) / grid_size).floor() as i32;
        let end_x = ((rect.max.x - self.pan.x) / grid_size).ceil() as i32;
        let start_y = ((rect.min.y - self.pan.y) / grid_size).floor() as i32;
        let end_y = ((rect.max.y - self.pan.y) / grid_size).ceil() as i32;

        for x in start_x..=end_x {
            let screen_x = x as f32 * grid_size + self.pan.x + rect.center().x;
            if screen_x >= rect.min.x && screen_x <= rect.max.x {
                painter.line_segment(
                    [Pos2::new(screen_x, rect.min.y), Pos2::new(screen_x, rect.max.y)],
                    Stroke::new(1.0, grid_color),
                );
            }
        }

        for y in start_y..=end_y {
            let screen_y = y as f32 * grid_size + self.pan.y + rect.center().y;
            if screen_y >= rect.min.y && screen_y <= rect.max.y {
                painter.line_segment(
                    [Pos2::new(rect.min.x, screen_y), Pos2::new(rect.max.x, screen_y)],
                    Stroke::new(1.0, grid_color),
                );
            }
        }
    }

    fn draw_screen(
        &self,
        painter: &egui::Painter,
        transform: &CanvasTransform,
        screen: &Screen,
        is_selected: bool,
        is_dragging: bool,
        player: &VideoPlayer,
        show_test_pattern: bool,
    ) {
        let rect = self.screen_to_canvas_rect(transform, screen);
        
        // Draw content based on mode
        if show_test_pattern {
            self.draw_test_pattern_on_screen(painter, rect);
        } else if player.is_loaded() {
            match self.preview_quality {
                PreviewQuality::ActualFrames => {
                    self.draw_video_frame_on_screen(painter, rect, player);
                }
                PreviewQuality::ActivityIndicator => {
                    self.draw_activity_indicator(painter, rect, player);
                }
            }
        } else {
            // No content - draw placeholder
            let bg_color = if screen.enabled {
                Color32::from_rgba_unmultiplied(40, 60, 80, 200)
            } else {
                Color32::from_rgba_unmultiplied(60, 60, 60, 150)
            };
            painter.rect_filled(rect, 4.0, bg_color);
            
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No Content",
                egui::FontId::proportional(14.0),
                Color32::from_gray(100),
            );
        }

        // Border
        let border_color = if is_dragging {
            Color32::from_rgb(255, 200, 100)
        } else if is_selected {
            Color32::from_rgb(100, 180, 255)
        } else {
            Color32::from_gray(100)
        };
        let border_width = if is_selected || is_dragging { 2.0 } else { 1.0 };
        painter.rect_stroke(rect, 4.0, Stroke::new(border_width, border_color));

        // Label background
        let label_bg_rect = EguiRect::from_min_size(rect.min, Vec2::new(rect.width(), 40.0));
        painter.rect_filled(label_bg_rect, egui::Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 }, 
            Color32::from_rgba_unmultiplied(0, 0, 0, 150));

        // Label
        let label_pos = rect.min + Vec2::new(8.0, 8.0);
        painter.text(
            label_pos,
            egui::Align2::LEFT_TOP,
            &screen.name,
            egui::FontId::proportional(14.0),
            Color32::WHITE,
        );

        // Resolution label
        let res_text = format!("{}×{}", screen.resolution.0, screen.resolution.1);
        painter.text(
            rect.min + Vec2::new(8.0, 24.0),
            egui::Align2::LEFT_TOP,
            &res_text,
            egui::FontId::proportional(10.0),
            Color32::from_gray(180),
        );

        // Blend indicators
        if screen.has_blending() {
            self.draw_blend_indicators(painter, rect, screen);
        }

        // Draw resize handles when selected
        if is_selected {
            self.draw_resize_handles(painter, rect);
        }
    }

    /// Draw resize handles around a screen
    fn draw_resize_handles(&self, painter: &egui::Painter, rect: EguiRect) {
        let handle_color = Color32::from_rgb(100, 180, 255);
        let handle_border = Color32::WHITE;
        
        for handle in ResizeHandle::all() {
            let pos = handle.position(rect);
            
            // Draw handle square
            let handle_rect = EguiRect::from_center_size(pos, Vec2::splat(RESIZE_HANDLE_SIZE));
            painter.rect_filled(handle_rect, 2.0, handle_color);
            painter.rect_stroke(handle_rect, 2.0, Stroke::new(1.0, handle_border));
        }
    }

    /// Draw SMPTE color bars test pattern on screen
    fn draw_test_pattern_on_screen(&self, painter: &egui::Painter, rect: EguiRect) {
        let colors = [
            Color32::WHITE,
            Color32::YELLOW,
            Color32::from_rgb(0, 255, 255), // Cyan
            Color32::GREEN,
            Color32::from_rgb(255, 0, 255), // Magenta
            Color32::RED,
            Color32::BLUE,
            Color32::BLACK,
        ];

        let bar_width = rect.width() / colors.len() as f32;
        let main_height = rect.height() * 0.75;
        
        // Main color bars
        for (i, color) in colors.iter().enumerate() {
            let x = rect.min.x + i as f32 * bar_width;
            let bar_rect = EguiRect::from_min_size(
                Pos2::new(x, rect.min.y),
                Vec2::new(bar_width + 1.0, main_height),
            );
            painter.rect_filled(bar_rect, 0.0, *color);
        }

        // Bottom gradient section
        let gradient_height = rect.height() * 0.15;
        let gradient_y = rect.min.y + main_height;
        let gradient_steps = 32;
        let step_width = rect.width() / gradient_steps as f32;
        
        for i in 0..gradient_steps {
            let gray = (i as f32 / gradient_steps as f32 * 255.0) as u8;
            let x = rect.min.x + i as f32 * step_width;
            let step_rect = EguiRect::from_min_size(
                Pos2::new(x, gradient_y),
                Vec2::new(step_width + 1.0, gradient_height),
            );
            painter.rect_filled(step_rect, 0.0, Color32::from_gray(gray));
        }

        // Bottom info bar
        let info_rect = EguiRect::from_min_size(
            Pos2::new(rect.min.x, gradient_y + gradient_height),
            Vec2::new(rect.width(), rect.height() * 0.10),
        );
        painter.rect_filled(info_rect, 0.0, Color32::from_gray(30));

        // Center crosshair
        let center = Pos2::new(rect.center().x, rect.min.y + main_height / 2.0);
        let cross_size = 15.0 * self.zoom.min(1.5);
        painter.line_segment(
            [Pos2::new(center.x - cross_size, center.y), Pos2::new(center.x + cross_size, center.y)],
            Stroke::new(2.0, Color32::WHITE),
        );
        painter.line_segment(
            [Pos2::new(center.x, center.y - cross_size), Pos2::new(center.x, center.y + cross_size)],
            Stroke::new(2.0, Color32::WHITE),
        );
        painter.circle_stroke(center, cross_size * 0.7, Stroke::new(1.0, Color32::WHITE));
    }

    /// Draw animated activity indicator when video is playing
    fn draw_activity_indicator(&self, painter: &egui::Painter, rect: EguiRect, player: &VideoPlayer) {
        let progress = player.progress();
        let time = self.animation_time;
        let is_playing = player.is_playing();
        
        // Animated gradient background
        let steps = 24;
        let step_width = rect.width() / steps as f32;
        
        for i in 0..steps {
            let t = i as f32 / steps as f32;
            let wave = if is_playing {
                ((t * 4.0 + time * 2.0).sin() * 0.5 + 0.5) * 0.3
            } else {
                0.1
            };
            
            let r = ((t + wave) * 80.0 + 40.0) as u8;
            let g = ((1.0 - t + wave) * 60.0 + 40.0) as u8;
            let b = ((wave * 2.0) * 100.0 + 80.0) as u8;
            
            let x = rect.min.x + i as f32 * step_width;
            let strip_rect = EguiRect::from_min_size(
                Pos2::new(x, rect.min.y),
                Vec2::new(step_width + 1.0, rect.height()),
            );
            
            painter.rect_filled(strip_rect, 0.0, Color32::from_rgb(r, g, b));
        }

        // Scanlines effect
        for y in (0..rect.height() as i32).step_by(4) {
            let line_y = rect.min.y + y as f32;
            painter.line_segment(
                [Pos2::new(rect.min.x, line_y), Pos2::new(rect.max.x, line_y)],
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 30)),
            );
        }

        // Progress bar at bottom
        let bar_height = 4.0;
        let bar_rect = EguiRect::from_min_size(
            Pos2::new(rect.min.x, rect.max.y - bar_height),
            Vec2::new(rect.width() * progress as f32, bar_height),
        );
        painter.rect_filled(bar_rect, 0.0, Color32::from_rgb(100, 200, 255));

        // Play/pause icon in center
        let icon = if is_playing { "▶" } else { "⏸" };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(24.0 * self.zoom.min(1.5)),
            Color32::from_rgba_unmultiplied(255, 255, 255, 150),
        );
    }

    /// Draw actual video frame (placeholder - would need texture integration)
    fn draw_video_frame_on_screen(&self, painter: &egui::Painter, rect: EguiRect, player: &VideoPlayer) {
        // For now, draw a more detailed placeholder that simulates video content
        // In a full implementation, this would render the actual video texture
        
        let progress = player.progress();
        let time = self.animation_time;
        let is_playing = player.is_playing();
        
        // Darker base for "video" content
        painter.rect_filled(rect, 4.0, Color32::from_gray(20));
        
        // Simulated video noise/content
        let grid_size = 8;
        let cell_w = rect.width() / grid_size as f32;
        let cell_h = rect.height() / grid_size as f32;
        
        for y in 0..grid_size {
            for x in 0..grid_size {
                let noise = ((x as f32 * 0.7 + y as f32 * 1.1 + time * 3.0).sin() * 0.5 + 0.5) * 40.0;
                let base = 30.0 + progress as f32 * 20.0;
                let gray = (base + noise) as u8;
                
                let cell_rect = EguiRect::from_min_size(
                    Pos2::new(rect.min.x + x as f32 * cell_w, rect.min.y + y as f32 * cell_h),
                    Vec2::new(cell_w, cell_h),
                );
                painter.rect_filled(cell_rect, 0.0, Color32::from_gray(gray));
            }
        }
        
        // "HD" badge
        let badge_rect = EguiRect::from_min_size(
            Pos2::new(rect.max.x - 35.0, rect.min.y + 45.0),
            Vec2::new(30.0, 16.0),
        );
        painter.rect_filled(badge_rect, 2.0, Color32::from_rgb(200, 50, 50));
        painter.text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            "HD",
            egui::FontId::proportional(10.0),
            Color32::WHITE,
        );

        // Progress bar
        let bar_height = 4.0;
        let bar_rect = EguiRect::from_min_size(
            Pos2::new(rect.min.x, rect.max.y - bar_height),
            Vec2::new(rect.width() * progress as f32, bar_height),
        );
        painter.rect_filled(bar_rect, 0.0, Color32::from_rgb(50, 200, 50));

        // Play indicator
        if is_playing {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "▶ LIVE",
                egui::FontId::proportional(16.0 * self.zoom.min(1.5)),
                Color32::from_rgba_unmultiplied(255, 255, 255, 180),
            );
        }
    }

    fn draw_slice(
        &self,
        painter: &egui::Painter,
        transform: &CanvasTransform,
        screen: &Screen,
        slice: &crate::output::Slice,
        is_selected: bool,
    ) {
        let screen_rect = self.screen_to_canvas_rect(transform, screen);
        let slice_rect = self.slice_to_canvas_rect(transform, screen, slice);

        // Slice border (only draw border, content is already shown on screen)
        let border_color = if is_selected {
            Color32::from_rgb(255, 200, 100)
        } else {
            Color32::from_rgba_unmultiplied(150, 150, 150, 80)
        };
        painter.rect_stroke(slice_rect, 2.0, Stroke::new(if is_selected { 2.0 } else { 1.0 }, border_color));

        // Corner handles for selected slice
        if is_selected {
            let corners = slice.get_output_vertices();
            for (_i, corner) in corners.iter().enumerate() {
                let pos = Pos2::new(
                    screen_rect.min.x + corner.x * screen_rect.width(),
                    screen_rect.min.y + corner.y * screen_rect.height(),
                );
                painter.circle_filled(pos, 6.0, Color32::from_rgb(255, 200, 100));
                painter.circle_stroke(pos, 6.0, Stroke::new(1.0, Color32::WHITE));
            }
        }
    }

    fn draw_blend_indicators(&self, painter: &egui::Painter, rect: EguiRect, screen: &Screen) {
        let blend_color = Color32::from_rgba_unmultiplied(255, 100, 100, 80);
        
        if let Some(left) = &screen.blend_config.left {
            let width = (left.width as f32 / screen.resolution.0 as f32) * rect.width();
            let blend_rect = EguiRect::from_min_size(rect.min, Vec2::new(width, rect.height()));
            painter.rect_filled(blend_rect, 0.0, blend_color);
            
            // Gradient lines to indicate blend
            for i in 0..5 {
                let x = rect.min.x + (i as f32 / 5.0) * width;
                let alpha = ((5 - i) as f32 / 5.0 * 100.0) as u8;
                painter.line_segment(
                    [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 100, 100, alpha)),
                );
            }
        }
        
        if let Some(right) = &screen.blend_config.right {
            let width = (right.width as f32 / screen.resolution.0 as f32) * rect.width();
            let blend_rect = EguiRect::from_min_size(
                Pos2::new(rect.max.x - width, rect.min.y),
                Vec2::new(width, rect.height()),
            );
            painter.rect_filled(blend_rect, 0.0, blend_color);
        }
        
        if let Some(top) = &screen.blend_config.top {
            let height = (top.width as f32 / screen.resolution.1 as f32) * rect.height();
            let blend_rect = EguiRect::from_min_size(rect.min, Vec2::new(rect.width(), height));
            painter.rect_filled(blend_rect, 0.0, blend_color);
        }
        
        if let Some(bottom) = &screen.blend_config.bottom {
            let height = (bottom.width as f32 / screen.resolution.1 as f32) * rect.height();
            let blend_rect = EguiRect::from_min_size(
                Pos2::new(rect.min.x, rect.max.y - height),
                Vec2::new(rect.width(), height),
            );
            painter.rect_filled(blend_rect, 0.0, blend_color);
        }
    }

    fn screen_to_canvas_rect(&self, transform: &CanvasTransform, screen: &Screen) -> EguiRect {
        // Use screen's actual position
        let min = Pos2::new(
            transform.offset.x + screen.position.0 * transform.scale,
            transform.offset.y + screen.position.1 * transform.scale,
        );
        let size = Vec2::new(
            screen.resolution.0 as f32 * transform.scale,
            screen.resolution.1 as f32 * transform.scale,
        );
        
        EguiRect::from_min_size(min, size)
    }

    fn slice_to_canvas_rect(
        &self,
        transform: &CanvasTransform,
        screen: &Screen,
        slice: &crate::output::Slice,
    ) -> EguiRect {
        let screen_rect = self.screen_to_canvas_rect(transform, screen);
        
        let input = &slice.input_rect;
        let screen_w = screen.resolution.0 as f32;
        let screen_h = screen.resolution.1 as f32;
        
        let x = input.x / screen_w;
        let y = input.y / screen_h;
        let w = input.width / screen_w;
        let h = input.height / screen_h;
        
        EguiRect::from_min_size(
            Pos2::new(
                screen_rect.min.x + x * screen_rect.width(),
                screen_rect.min.y + y * screen_rect.height(),
            ),
            Vec2::new(
                w * screen_rect.width(),
                h * screen_rect.height(),
            ),
        )
    }

}

struct CanvasTransform {
    offset: Vec2,
    scale: f32,
}

/// Size of resize handles in pixels
const RESIZE_HANDLE_SIZE: f32 = 8.0;

impl ResizeHandle {
    /// Get all handles for iteration
    fn all() -> &'static [ResizeHandle] {
        &[
            ResizeHandle::TopLeft,
            ResizeHandle::Top,
            ResizeHandle::TopRight,
            ResizeHandle::Right,
            ResizeHandle::BottomRight,
            ResizeHandle::Bottom,
            ResizeHandle::BottomLeft,
            ResizeHandle::Left,
        ]
    }

    /// Get the position of this handle relative to a rect
    fn position(&self, rect: EguiRect) -> Pos2 {
        match self {
            ResizeHandle::TopLeft => rect.left_top(),
            ResizeHandle::Top => Pos2::new(rect.center().x, rect.top()),
            ResizeHandle::TopRight => rect.right_top(),
            ResizeHandle::Right => Pos2::new(rect.right(), rect.center().y),
            ResizeHandle::BottomRight => rect.right_bottom(),
            ResizeHandle::Bottom => Pos2::new(rect.center().x, rect.bottom()),
            ResizeHandle::BottomLeft => rect.left_bottom(),
            ResizeHandle::Left => Pos2::new(rect.left(), rect.center().y),
        }
    }

    /// Get the cursor style for this handle
    fn cursor(&self) -> egui::CursorIcon {
        match self {
            ResizeHandle::TopLeft | ResizeHandle::BottomRight => egui::CursorIcon::ResizeNwSe,
            ResizeHandle::TopRight | ResizeHandle::BottomLeft => egui::CursorIcon::ResizeNeSw,
            ResizeHandle::Top | ResizeHandle::Bottom => egui::CursorIcon::ResizeVertical,
            ResizeHandle::Left | ResizeHandle::Right => egui::CursorIcon::ResizeHorizontal,
        }
    }

    /// Check if a point is within this handle's hit area
    fn contains(&self, rect: EguiRect, point: Pos2) -> bool {
        let handle_pos = self.position(rect);
        let handle_rect = EguiRect::from_center_size(handle_pos, Vec2::splat(RESIZE_HANDLE_SIZE * 2.0));
        handle_rect.contains(point)
    }
}
