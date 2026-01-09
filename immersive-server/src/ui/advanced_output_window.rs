//! Advanced Output Window
//!
//! A modal window for configuring multi-screen outputs with slice-based input selection.
//! Accessible via View → Advanced Output.

use std::collections::HashSet;

use crate::compositor::Viewport;
use crate::output::{DisplayInfo, EdgeBlendConfig, MaskShape, OutputDevice, OutputManager, OutputPresetManager, Point2D as MaskPoint2D, Screen, ScreenId, Slice, SliceId, SliceInput, SliceMask, WarpMesh};
use crate::output::slice::{Point2D, Rect};
use egui::PointerButton;
use super::viewport_widget::{self, ViewportConfig};

/// Color palette for screen/slice visualization - used across all preview modes
/// Each screen gets a color from this palette (cycling if more than 6 screens)
fn screen_colors() -> [egui::Color32; 6] {
    [
        egui::Color32::from_rgb(100, 149, 237), // Cornflower blue
        egui::Color32::from_rgb(50, 205, 50),   // Lime green
        egui::Color32::from_rgb(255, 165, 0),   // Orange
        egui::Color32::from_rgb(186, 85, 211),  // Medium orchid
        egui::Color32::from_rgb(255, 99, 71),   // Tomato
        egui::Color32::from_rgb(64, 224, 208),  // Turquoise
    ]
}

/// Tab selection for the Advanced Output window
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AdvancedOutputTab {
    #[default]
    Screens,
    OutputTransformation,
}

/// Which part of a rectangle is being dragged
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RectHandle {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    TopEdge,
    BottomEdge,
    LeftEdge,
    RightEdge,
    Body,
}

/// State for interactive input_rect editing
#[derive(Debug, Clone, Default)]
pub struct InputRectDragState {
    /// Which screen is being dragged (if any)
    pub dragging_screen: Option<ScreenId>,
    /// Which slice is being dragged (if any)
    pub dragging_slice: Option<SliceId>,
    /// Which handle of the rect is being dragged
    pub dragging_handle: Option<RectHandle>,
    /// Original rect when drag started (for computing delta)
    pub original_rect: Option<Rect>,
    /// Starting pointer position (normalized 0-1) for body drag
    pub start_pos: Option<[f32; 2]>,
}

/// State for interactive output_rect editing in Output Transformation tab
#[derive(Debug, Clone, Default)]
pub struct OutputRectDragState {
    /// Which slice is being dragged (if any)
    pub dragging_slice: Option<SliceId>,
    /// Which handle of the rect is being dragged
    pub dragging_handle: Option<RectHandle>,
    /// Original rect when drag started (for computing delta)
    pub original_rect: Option<Rect>,
    /// Starting pointer position (normalized 0-1) for body drag
    pub start_pos: Option<[f32; 2]>,
}

/// Editing mode for Output Transformation tab
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputEditMode {
    /// Resize mode: drag handles to resize slice output_rect
    #[default]
    Resize,
    /// Mesh Warp mode: drag points to adjust warp mesh
    MeshWarp,
}

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
    /// Move a slice up (earlier in list, lower index)
    MoveSliceUp {
        screen_id: ScreenId,
        slice_id: SliceId,
    },
    /// Move a slice down (later in list, higher index)
    MoveSliceDown {
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
    /// Update the input_rect for all slices of a screen (from environment view drag)
    UpdateScreenInputRect { screen_id: ScreenId, input_rect: Rect },
    /// Update the input_rect for a specific slice (from environment view drag)
    UpdateSliceInputRect { screen_id: ScreenId, slice_id: SliceId, input_rect: Rect },
    /// Save the composition (triggered when window is closed)
    SaveComposition,
    /// Load an output preset by name
    LoadPreset { name: String },
    /// Save current configuration as a new preset
    SaveAsPreset { name: String },
    /// Delete a user preset
    DeletePreset { name: String },
    /// Create a new configuration with a single virtual screen
    NewConfiguration,
}

/// Pending action when the user has unsaved changes and tries to switch presets
#[derive(Debug, Clone)]
pub enum PendingPresetAction {
    /// Load a preset (after confirming discard)
    LoadPreset { name: String },
    /// Close the window (after confirming discard)
    CloseWindow,
    /// Create new configuration (after confirming discard)
    NewConfiguration,
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
    /// egui texture ID for the live preview (screen output)
    pub preview_texture_id: Option<egui::TextureId>,
    /// Currently dragged warp point (col, row)
    dragging_warp_point: Option<(usize, usize)>,
    /// Currently dragged mask vertex index
    dragging_mask_vertex: Option<usize>,
    /// Temporary device name for streaming outputs
    temp_device_name: String,
    /// Temporary OMT port
    temp_omt_port: String,
    /// Currently selected tab
    current_tab: AdvancedOutputTab,
    /// egui texture ID for the environment preview (Screens tab)
    pub environment_texture_id: Option<egui::TextureId>,
    /// State for dragging input_rect handles in environment view
    input_rect_drag: InputRectDragState,
    /// State for dragging output_rect handles in Output Transformation tab
    output_rect_drag: OutputRectDragState,
    /// Current editing mode for Output Transformation tab
    output_edit_mode: OutputEditMode,
    /// Viewport for environment preview (Screens tab) pan/zoom
    env_viewport: Viewport,
    /// Viewport for screen output preview (Output Transformation tab) pan/zoom
    output_viewport: Viewport,
    /// Track last frame time for viewport animation
    last_viewport_update: std::time::Instant,

    // Preset state
    /// Current preset name (if loaded from a preset)
    pub current_preset_name: Option<String>,
    /// Whether current config differs from loaded preset
    is_dirty: bool,
    /// Whether to show the save preset dialog
    show_save_dialog: bool,
    /// Preset name being entered in save dialog
    save_dialog_name: String,
    /// Pending action when showing unsaved changes dialog
    pending_action: Option<PendingPresetAction>,
    /// Preset name to save (set by dialog, processed outside nested closures)
    save_requested: Option<String>,
    /// Set of screens that are expanded to show their slices in the Screens tab
    expanded_screens: HashSet<ScreenId>,
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
            current_tab: AdvancedOutputTab::default(),
            environment_texture_id: None,
            input_rect_drag: InputRectDragState::default(),
            output_rect_drag: OutputRectDragState::default(),
            output_edit_mode: OutputEditMode::default(),
            env_viewport: Viewport::new(),
            output_viewport: Viewport::new(),
            last_viewport_update: std::time::Instant::now(),

            // Preset state
            current_preset_name: None,
            is_dirty: false,
            show_save_dialog: false,
            save_dialog_name: String::new(),
            pending_action: None,
            save_requested: None,
            expanded_screens: HashSet::new(),
        }
    }

    /// Get the currently selected screen ID (for texture registration in app.rs)
    pub fn selected_screen_id(&self) -> Option<ScreenId> {
        self.selected_screen
    }

    /// Select a screen and update temp fields with its values
    pub fn select_screen(&mut self, screen_id: ScreenId, name: &str, width: u32, height: u32) {
        self.selected_screen = Some(screen_id);
        self.selected_slice = None;
        self.temp_screen_name = name.to_string();
        self.temp_width = width.to_string();
        self.temp_height = height.to_string();
    }

    /// Toggle the window open/closed
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    /// Set the current preset name (called after loading a preset)
    pub fn set_current_preset(&mut self, name: Option<String>) {
        self.current_preset_name = name;
        self.is_dirty = false;
    }

    /// Mark the configuration as dirty (modified since last load/save)
    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    /// Clear the dirty flag
    pub fn clear_dirty(&mut self) {
        self.is_dirty = false;
    }

    /// Check if there are unsaved changes
    pub fn has_unsaved_changes(&self) -> bool {
        self.is_dirty
    }

    // ========== Shared Slice Visualization Helpers ==========

    /// Get the color for a screen by index (cycles through palette)
    fn screen_color(screen_idx: usize) -> egui::Color32 {
        screen_colors()[screen_idx % screen_colors().len()]
    }

    /// Format a slice label like "1.A", "2.B", etc.
    fn slice_label(screen_idx: usize, slice_idx: usize) -> String {
        let slice_char = (b'A' + slice_idx as u8) as char;
        format!("{}.{}", screen_idx + 1, slice_char)
    }

    /// Draw a slice rectangle with label overlay
    /// Used by both Screens tab (input_rect) and Output Transformation tab (output_rect)
    fn draw_slice_overlay(
        painter: &egui::Painter,
        rect: egui::Rect,
        screen_idx: usize,
        slice_idx: usize,
        is_selected: bool,
        zoom: f32,
    ) {
        let base_color = Self::screen_color(screen_idx);

        // Stroke styling based on selection
        let stroke_width = if is_selected { 3.0 } else { 1.5 };
        let alpha = if is_selected { 255 } else { 150 };
        let stroke_color = egui::Color32::from_rgba_unmultiplied(
            base_color.r(), base_color.g(), base_color.b(), alpha
        );

        // Draw rectangle outline
        painter.rect_stroke(
            rect,
            2.0,
            egui::Stroke::new(stroke_width, stroke_color),
            egui::StrokeKind::Outside,
        );

        // Draw slice label in center
        let text_pos = rect.center();
        let label_text = Self::slice_label(screen_idx, slice_idx);
        let font_size = (16.0 * zoom).clamp(10.0, 32.0);
        let font_id = egui::FontId::proportional(font_size);

        // Background pill
        let galley = painter.layout_no_wrap(label_text.clone(), font_id.clone(), egui::Color32::WHITE);
        let text_size = galley.size();
        let padding = egui::vec2(6.0, 3.0);
        let bg_rect = egui::Rect::from_center_size(text_pos, text_size + padding * 2.0);

        let bg_color = if is_selected {
            base_color
        } else {
            egui::Color32::from_rgba_unmultiplied(base_color.r(), base_color.g(), base_color.b(), 180)
        };
        painter.rect_filled(bg_rect, 4.0, bg_color);
        painter.text(text_pos, egui::Align2::CENTER_CENTER, &label_text, font_id, egui::Color32::WHITE);
    }

    /// Render the preset selector bar at the top of the window
    fn render_preset_selector(
        &mut self,
        ui: &mut egui::Ui,
        preset_manager: &OutputPresetManager,
        actions: &mut Vec<AdvancedOutputAction>,
    ) {
        ui.horizontal(|ui| {
            ui.label("Preset:");

            // Current preset display label
            let current_label = self
                .current_preset_name
                .as_deref()
                .unwrap_or("(Custom)");

            // Preset dropdown
            egui::ComboBox::from_id_salt("output_preset_selector")
                .selected_text(current_label)
                .width(180.0)
                .show_ui(ui, |ui| {
                    // Built-in presets section
                    ui.label(egui::RichText::new("Built-in").weak().small());
                    for (_index, preset) in preset_manager.builtin_presets() {
                        let is_selected = self.current_preset_name.as_deref() == Some(&preset.name);
                        if ui.selectable_label(is_selected, &preset.name).clicked() {
                            if self.is_dirty {
                                // Show unsaved changes warning - store pending action
                                self.pending_action = Some(PendingPresetAction::LoadPreset {
                                    name: preset.name.clone(),
                                });
                            } else {
                                actions.push(AdvancedOutputAction::LoadPreset {
                                    name: preset.name.clone(),
                                });
                            }
                        }
                    }

                    // User presets section (if any)
                    let user_presets: Vec<_> = preset_manager.user_presets().collect();
                    tracing::trace!("Rendering preset dropdown: {} user presets", user_presets.len());
                    if !user_presets.is_empty() {
                        ui.separator();
                        ui.label(egui::RichText::new("User Presets").weak().small());
                        for (_index, preset) in user_presets {
                            let is_selected = self.current_preset_name.as_deref() == Some(&preset.name);
                            ui.horizontal(|ui| {
                                if ui.selectable_label(is_selected, &preset.name).clicked() {
                                    if self.is_dirty {
                                        self.pending_action = Some(PendingPresetAction::LoadPreset {
                                            name: preset.name.clone(),
                                        });
                                    } else {
                                        actions.push(AdvancedOutputAction::LoadPreset {
                                            name: preset.name.clone(),
                                        });
                                    }
                                }
                                // Delete button for user presets
                                if ui.small_button("🗑").on_hover_text("Delete preset").clicked() {
                                    actions.push(AdvancedOutputAction::DeletePreset {
                                        name: preset.name.clone(),
                                    });
                                }
                            });
                        }
                    }
                });

            // New button
            if ui.button("New").on_hover_text("Create new configuration with single screen").clicked() {
                if self.is_dirty {
                    self.pending_action = Some(PendingPresetAction::NewConfiguration);
                } else {
                    actions.push(AdvancedOutputAction::NewConfiguration);
                }
            }

            // Save button
            if ui.button("Save...").clicked() {
                // Pre-fill with current preset name if saving over existing
                self.save_dialog_name = self.current_preset_name.clone().unwrap_or_default();
                self.show_save_dialog = true;
            }

            // Dirty indicator
            if self.is_dirty {
                ui.label(egui::RichText::new("*").color(egui::Color32::YELLOW))
                    .on_hover_text("Unsaved changes");
            }
        });

        // Unsaved changes dialog
        if self.pending_action.is_some() {
            self.render_unsaved_changes_dialog(ui, actions);
        }
    }

    /// Render the unsaved changes confirmation dialog
    fn render_unsaved_changes_dialog(
        &mut self,
        ui: &mut egui::Ui,
        _actions: &mut Vec<AdvancedOutputAction>,
    ) {
        egui::Window::new("Unsaved Changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.label("You have unsaved changes to the output configuration.");
                ui.label("What would you like to do?");
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("Save First...").clicked() {
                        // Open save dialog, keep pending action for after save
                        self.save_dialog_name = self.current_preset_name.clone().unwrap_or_default();
                        self.show_save_dialog = true;
                    }
                    if ui.button("Discard").clicked() {
                        // Proceed with pending action, discarding changes
                        self.is_dirty = false;
                        // pending_action will be processed in render()
                    }
                    if ui.button("Cancel").clicked() {
                        // Cancel the pending action
                        self.pending_action = None;
                    }
                });
            });
    }

    /// Render the save preset dialog
    fn render_save_preset_dialog(
        &mut self,
        ui: &mut egui::Ui,
        preset_manager: &OutputPresetManager,
        _actions: &mut Vec<AdvancedOutputAction>,
    ) {
        if !self.show_save_dialog {
            return;
        }

        egui::Window::new("Save Output Preset")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.label("Preset name:");
                    ui.text_edit_singleline(&mut self.save_dialog_name);
                });

                // Warning if name conflicts with built-in
                let is_builtin = preset_manager
                    .builtin_presets()
                    .any(|(_, p)| p.name == self.save_dialog_name);
                if is_builtin {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        "Cannot overwrite built-in presets",
                    );
                }

                // Warning if will overwrite existing user preset
                let exists_user = preset_manager
                    .user_presets()
                    .any(|(_, p)| p.name == self.save_dialog_name);
                if exists_user {
                    ui.colored_label(
                        egui::Color32::LIGHT_BLUE,
                        "Will overwrite existing preset",
                    );
                }

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    let can_save = !self.save_dialog_name.trim().is_empty() && !is_builtin;
                    ui.add_enabled_ui(can_save, |ui| {
                        if ui.button("Save").clicked() {
                            // Set flag instead of pushing action here (nested closure issue)
                            // The action will be pushed outside the dialog closure
                            self.save_requested = Some(self.save_dialog_name.trim().to_string());
                            self.show_save_dialog = false;
                        }
                    });
                    if ui.button("Cancel").clicked() {
                        self.show_save_dialog = false;
                    }
                });
            });
    }

    /// Generate a unique NDI name that doesn't conflict with existing screens
    fn unique_ndi_name(base_name: &str, current_screen_id: ScreenId, all_screens: &[&Screen]) -> String {
        // Collect existing NDI names from other screens
        let existing_names: Vec<&str> = all_screens
            .iter()
            .filter(|s| s.id != current_screen_id)
            .filter_map(|s| {
                if let OutputDevice::Ndi { name } = &s.device {
                    Some(name.as_str())
                } else {
                    None
                }
            })
            .collect();

        // If base name doesn't conflict, use it
        if !existing_names.contains(&base_name) {
            return base_name.to_string();
        }

        // Otherwise, append a number to make it unique
        for i in 2..100 {
            let candidate = format!("{} ({})", base_name, i);
            if !existing_names.contains(&candidate.as_str()) {
                return candidate;
            }
        }

        // Fallback (shouldn't happen in practice)
        format!("{} (99)", base_name)
    }

    /// Render the Advanced Output window
    ///
    /// Returns a list of actions to be processed by the app.
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        output_manager: Option<&OutputManager>,
        preset_manager: &OutputPresetManager,
        layer_count: usize,
        available_displays: &[DisplayInfo],
        env_dimensions: (u32, u32),
    ) -> Vec<AdvancedOutputAction> {
        let mut actions = Vec::new();

        if !self.open {
            return actions;
        }

        // Update viewport animations
        let env_content_size = (env_dimensions.0 as f32, env_dimensions.1 as f32);
        let output_content_size = output_manager
            .and_then(|m| {
                self.selected_screen
                    .and_then(|id| m.screens().find(|s| s.id == id))
                    .map(|s| (s.width as f32, s.height as f32))
            });
        self.update_viewports(env_content_size, output_content_size);

        // Request repaint if viewports are animating
        if self.viewports_need_update() {
            ctx.request_repaint();
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
                self.render_contents(ui, output_manager, preset_manager, layer_count, available_displays, env_dimensions, &mut actions);
            });

        // Handle pending actions (from unsaved changes dialog)
        if let Some(pending) = self.pending_action.take() {
            match pending {
                PendingPresetAction::LoadPreset { name } => {
                    actions.push(AdvancedOutputAction::LoadPreset { name });
                }
                PendingPresetAction::CloseWindow => {
                    open = false;
                    actions.push(AdvancedOutputAction::SaveComposition);
                }
                PendingPresetAction::NewConfiguration => {
                    actions.push(AdvancedOutputAction::NewConfiguration);
                }
            }
        }

        // Handle save preset request (set by dialog, processed here outside all closures)
        if let Some(name) = self.save_requested.take() {
            tracing::info!("Pushing SaveAsPreset action for preset: {}", name);
            actions.push(AdvancedOutputAction::SaveAsPreset { name: name.clone() });
            self.current_preset_name = Some(name);
            self.is_dirty = false;
        }

        // Detect window close and trigger save
        if self.open && !open {
            actions.push(AdvancedOutputAction::SaveComposition);
        }
        self.open = open;

        actions
    }

    /// Render the window contents
    fn render_contents(
        &mut self,
        ui: &mut egui::Ui,
        output_manager: Option<&OutputManager>,
        preset_manager: &OutputPresetManager,
        layer_count: usize,
        available_displays: &[DisplayInfo],
        env_dimensions: (u32, u32),
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

        // Preset selector at top
        self.render_preset_selector(ui, preset_manager, actions);
        ui.separator();
        ui.add_space(4.0);

        // Save preset dialog (sets save_requested flag, processed in render() outside closures)
        self.render_save_preset_dialog(ui, preset_manager, actions);

        // Tab bar at top
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.current_tab, AdvancedOutputTab::Screens, "Screens");
            ui.selectable_value(&mut self.current_tab, AdvancedOutputTab::OutputTransformation, "Output Transformation");
        });
        ui.separator();
        ui.add_space(4.0);

        // Dispatch to appropriate tab
        match self.current_tab {
            AdvancedOutputTab::Screens => {
                self.render_screens_tab(ui, &screens, available_displays, env_dimensions, actions);
            }
            AdvancedOutputTab::OutputTransformation => {
                self.render_output_transformation_tab(ui, &screens, layer_count, actions);
            }
        }
    }

    /// Render the hierarchical screen/slice list with expand/collapse and +/- buttons
    /// Used by both Screens tab and Output Transformation tab
    fn render_screen_slice_list(
        &mut self,
        ui: &mut egui::Ui,
        screens: &[&Screen],
        actions: &mut Vec<AdvancedOutputAction>,
        id_salt: &str,
    ) {
        ui.heading("Screens");
        ui.add_space(4.0);

        egui::ScrollArea::vertical()
            .id_salt(format!("{}_screens", id_salt))
            .max_height(300.0)
            .show(ui, |ui| {
                for (index, screen) in screens.iter().enumerate() {
                    let is_expanded = self.expanded_screens.contains(&screen.id);
                    let is_screen_selected = self.selected_screen == Some(screen.id)
                        && self.selected_slice.is_none();

                    // Screen row with expand toggle
                    ui.horizontal(|ui| {
                        let arrow = if is_expanded { "▼" } else { "▶" };
                        if ui.small_button(arrow).clicked() {
                            if is_expanded {
                                self.expanded_screens.remove(&screen.id);
                            } else {
                                self.expanded_screens.insert(screen.id);
                            }
                        }

                        let response = ui.selectable_label(
                            is_screen_selected,
                            format!("{}. {} ({}x{})", index + 1, screen.name, screen.width, screen.height),
                        );
                        if response.clicked() {
                            self.selected_screen = Some(screen.id);
                            self.selected_slice = None;
                            self.temp_screen_name = screen.name.clone();
                            self.temp_width = screen.width.to_string();
                            self.temp_height = screen.height.to_string();
                        }
                    });

                    // Show slices if expanded
                    if is_expanded {
                        let slice_count = screen.slices.len();
                        for (slice_idx, slice) in screen.slices.iter().enumerate() {
                            let is_slice_selected = self.selected_screen == Some(screen.id)
                                && self.selected_slice == Some(slice.id);

                            ui.indent(format!("{}_{}", id_salt, slice.id.0), |ui| {
                                ui.horizontal(|ui| {
                                    // Up/down reorder buttons
                                    let can_move_up = slice_idx > 0;
                                    let can_move_down = slice_idx < slice_count - 1;

                                    if ui.add_enabled(can_move_up, egui::Button::new("↑").small())
                                        .on_hover_text("Move up")
                                        .clicked()
                                    {
                                        actions.push(AdvancedOutputAction::MoveSliceUp {
                                            screen_id: screen.id,
                                            slice_id: slice.id,
                                        });
                                    }
                                    if ui.add_enabled(can_move_down, egui::Button::new("↓").small())
                                        .on_hover_text("Move down")
                                        .clicked()
                                    {
                                        actions.push(AdvancedOutputAction::MoveSliceDown {
                                            screen_id: screen.id,
                                            slice_id: slice.id,
                                        });
                                    }

                                    // Slice label (selectable)
                                    let slice_label_text = Self::slice_label(index, slice_idx);
                                    let label_with_name = format!("{} {}", slice_label_text, slice.name);
                                    let response = ui.selectable_label(is_slice_selected, label_with_name);
                                    if response.clicked() {
                                        self.selected_screen = Some(screen.id);
                                        self.selected_slice = Some(slice.id);
                                        self.temp_slice_name = slice.name.clone();
                                    }
                                });
                            });
                        }

                        // Slice +/- buttons (indented)
                        ui.indent(format!("{}_slice_btns_{}", id_salt, screen.id.0), |ui| {
                            ui.horizontal(|ui| {
                                if ui.small_button("+").on_hover_text("Add slice").clicked() {
                                    actions.push(AdvancedOutputAction::AddSlice {
                                        screen_id: screen.id
                                    });
                                }
                                let can_remove = self.selected_screen == Some(screen.id)
                                    && self.selected_slice.is_some()
                                    && screen.slices.len() > 1;
                                if ui
                                    .add_enabled(can_remove, egui::Button::new("-").small())
                                    .on_hover_text("Remove selected slice")
                                    .clicked()
                                {
                                    if let Some(slice_id) = self.selected_slice {
                                        actions.push(AdvancedOutputAction::RemoveSlice {
                                            screen_id: screen.id,
                                            slice_id,
                                        });
                                        self.selected_slice = None;
                                    }
                                }
                            });
                        });
                    }
                }
            });

        // Screen +/- buttons
        ui.horizontal(|ui| {
            if ui.small_button("+").on_hover_text("Add screen").clicked() {
                actions.push(AdvancedOutputAction::AddScreen);
            }
            if ui
                .add_enabled(
                    self.selected_screen.is_some() && screens.len() > 1,
                    egui::Button::new("-").small(),
                )
                .on_hover_text("Remove selected screen")
                .clicked()
            {
                if let Some(screen_id) = self.selected_screen {
                    actions.push(AdvancedOutputAction::RemoveScreen { screen_id });
                    self.selected_screen = None;
                    self.selected_slice = None;
                }
            }
        });
    }

    /// Render the Screens tab with environment preview and input_rect editing
    fn render_screens_tab(
        &mut self,
        ui: &mut egui::Ui,
        screens: &[&Screen],
        available_displays: &[DisplayInfo],
        env_dimensions: (u32, u32),
        actions: &mut Vec<AdvancedOutputAction>,
    ) {
        // Three-column layout: Screens list | Environment Preview | Screen Properties
        let available = ui.available_size();
        ui.allocate_ui_with_layout(
            available,
            egui::Layout::left_to_right(egui::Align::TOP),
            |ui| {
            // LEFT COLUMN: Screens list with expandable slices
            ui.vertical(|ui| {
                ui.set_min_width(150.0);
                ui.set_max_width(180.0);
                self.render_screen_slice_list(ui, screens, actions, "screens_tab");
            });

            ui.separator();

            // MIDDLE COLUMN: Environment preview with input_rect overlays
            self.render_environment_preview(ui, screens, actions, env_dimensions);

            ui.separator();

            // RIGHT COLUMN: Screen properties (simplified for this tab)
            ui.vertical(|ui| {
                ui.set_min_width(200.0);
                ui.heading("Screen Properties");
                ui.add_space(4.0);

                egui::ScrollArea::vertical()
                    .id_salt("screen_props_scroll")
                    .show(ui, |ui| {
                        if let Some(screen_id) = self.selected_screen {
                            if let Some(screen) = screens.iter().find(|s| s.id == screen_id) {
                                self.render_screen_properties_simplified(ui, screen, screens, available_displays, env_dimensions, actions);
                            }
                        } else {
                            ui.label(egui::RichText::new("Select a screen to edit").weak().italics());
                        }
                    });
            });
        });
    }

    /// Render the environment preview with numbered screen input_rect overlays
    fn render_environment_preview(
        &mut self,
        ui: &mut egui::Ui,
        screens: &[&Screen],
        actions: &mut Vec<AdvancedOutputAction>,
        env_dimensions: (u32, u32),
    ) {
        ui.vertical(|ui| {
            ui.set_min_width(300.0);
            ui.heading("Environment");
            ui.add_space(4.0);

            // DEBUG: Log available space to diagnose window growth
            let raw_available_width = ui.available_width();
            let raw_available_height = ui.available_height();
            let clip_rect = ui.clip_rect();
            let ctx_screen_rect = ui.ctx().screen_rect();
            tracing::debug!(
                "AdvOutput: env={}x{}, available={}x{}, clip={:?}, screen={:?}",
                env_dimensions.0, env_dimensions.1,
                raw_available_width, raw_available_height,
                clip_rect,
                ctx_screen_rect
            );

            // Calculate preview size to fit available space while maintaining aspect ratio
            let env_aspect = env_dimensions.0 as f32 / env_dimensions.1 as f32;

            // Use the smaller of available_width or a reasonable fraction of screen width
            // to prevent unbounded growth
            let screen_based_max = ctx_screen_rect.width() * 0.5; // Max 50% of screen width
            let max_width = raw_available_width.min(screen_based_max);
            let max_height = (raw_available_height - 50.0).max(100.0).min(500.0);

            tracing::debug!(
                "AdvOutput: aspect={:.2}, screen_max={}, max_w={}, max_h={}",
                env_aspect, screen_based_max, max_width, max_height
            );

            // Fit to whichever constraint is tighter while maintaining aspect ratio
            let (preview_width, preview_height) = if max_height * env_aspect <= max_width {
                // Height is the limiting factor
                (max_height * env_aspect, max_height)
            } else {
                // Width is the limiting factor
                (max_width, max_width / env_aspect)
            };

            tracing::debug!(
                "AdvOutput: final preview={}x{}",
                preview_width, preview_height
            );

            let preview_size = egui::vec2(preview_width, preview_height);

            let (rect, response) = ui.allocate_exact_size(preview_size, egui::Sense::click_and_drag());

            // Environment content size (actual dimensions)
            let env_content_size = (env_dimensions.0 as f32, env_dimensions.1 as f32);

            // Handle viewport pan/zoom (right-click drag, scroll wheel)
            viewport_widget::handle_viewport_input(
                ui,
                &response,
                rect,
                &mut self.env_viewport,
                env_content_size,
                &ViewportConfig::default(),
                "adv_output_screens",
            );

            // Draw environment texture background
            ui.painter().rect_filled(rect, 4.0, egui::Color32::from_rgb(30, 30, 30));

            if let Some(tex_id) = self.environment_texture_id {
                // Transform full environment (0,0)-(1,1) through viewport to get destination rect
                let full_env_rect = Rect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 };
                let env_dest_rect = self.transform_rect_to_screen(
                    &self.env_viewport,
                    &full_env_rect,
                    rect,
                    preview_size,
                    env_content_size,
                );

                // Calculate intersection with preview rect (clip)
                let visible_rect = env_dest_rect.intersect(rect);
                if visible_rect.width() > 0.0 && visible_rect.height() > 0.0 {
                    // Calculate UV coords for the visible portion
                    let uv_left = (visible_rect.left() - env_dest_rect.left()) / env_dest_rect.width();
                    let uv_top = (visible_rect.top() - env_dest_rect.top()) / env_dest_rect.height();
                    let uv_right = (visible_rect.right() - env_dest_rect.left()) / env_dest_rect.width();
                    let uv_bottom = (visible_rect.bottom() - env_dest_rect.top()) / env_dest_rect.height();

                    let uv_rect = egui::Rect::from_min_max(
                        egui::pos2(uv_left, uv_top),
                        egui::pos2(uv_right, uv_bottom),
                    );

                    ui.painter().image(tex_id, visible_rect, uv_rect, egui::Color32::WHITE);
                }
            } else {
                // Show placeholder text if environment texture not yet registered
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Environment Preview",
                    egui::FontId::proportional(14.0),
                    egui::Color32::GRAY,
                );
            }

            // Draw input_rect rectangles for each screen (with viewport transform)
            self.draw_screen_input_rects(ui, rect, preview_size, screens, env_content_size);

            // Draw blend gradient overlays where screens overlap with blending enabled
            self.draw_screen_blend_overlaps(ui, rect, preview_size, screens, env_content_size);

            // Handle interactions (click to select, drag to edit)
            self.handle_environment_interactions(ui, rect, preview_size, screens, actions, &response, env_content_size);

            // Draw zoom indicator in bottom-right corner
            viewport_widget::draw_zoom_indicator(ui, rect, &self.env_viewport);

            // Show help text
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Right-drag to pan, scroll to zoom").small().weak());
        });
    }

    /// Draw numbered rectangles for each screen's input_rect
    fn draw_screen_input_rects(
        &self,
        ui: &egui::Ui,
        preview_rect: egui::Rect,
        preview_size: egui::Vec2,
        screens: &[&Screen],
        content_size: (f32, f32),
    ) {
        // Create a clipped painter that only draws within preview_rect
        let painter = ui.painter().with_clip_rect(preview_rect);
        let zoom = self.env_viewport.zoom();

        for (screen_idx, screen) in screens.iter().enumerate() {
            for (slice_idx, slice) in screen.slices.iter().enumerate() {
                // Convert normalized input_rect to screen coordinates using viewport transform
                let screen_rect = self.transform_rect_to_screen(
                    &self.env_viewport,
                    &slice.input_rect,
                    preview_rect,
                    preview_size,
                    content_size,
                );

                // Skip if completely outside preview area
                if !screen_rect.intersects(preview_rect) {
                    continue;
                }

                let is_selected = self.selected_screen == Some(screen.id)
                    && self.selected_slice == Some(slice.id);

                // Only draw full overlay (with label) if center is within preview
                if preview_rect.contains(screen_rect.center()) {
                    Self::draw_slice_overlay(&painter, screen_rect, screen_idx, slice_idx, is_selected, zoom);
                } else {
                    // Just draw rectangle outline if label would be clipped
                    let base_color = Self::screen_color(screen_idx);
                    let stroke_width = if is_selected { 3.0 } else { 1.5 };
                    let alpha = if is_selected { 255 } else { 150 };
                    painter.rect_stroke(
                        screen_rect,
                        2.0,
                        egui::Stroke::new(stroke_width,
                            egui::Color32::from_rgba_unmultiplied(base_color.r(), base_color.g(), base_color.b(), alpha)),
                        egui::StrokeKind::Outside,
                    );
                }

                // Draw corner/edge handles for selected slice
                if is_selected {
                    self.draw_rect_handles(&painter, screen_rect, zoom);
                }
            }
        }
    }

    /// Draw corner and edge handles for the selected screen's input_rect
    fn draw_rect_handles(&self, painter: &egui::Painter, rect: egui::Rect, zoom: f32) {
        let handle_radius = (8.0 * zoom).clamp(4.0, 16.0);
        let handle_color = egui::Color32::WHITE;
        let handle_stroke = egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 149, 237));

        // Corner handles (painter is already clipped, so just draw)
        let corners = [
            rect.left_top(),
            rect.right_top(),
            rect.left_bottom(),
            rect.right_bottom(),
        ];

        for pos in corners {
            painter.circle_filled(pos, handle_radius, handle_color);
            painter.circle_stroke(pos, handle_radius, handle_stroke);
        }

        // Edge midpoint handles (scale with zoom)
        let edge_handle_radius = (6.0 * zoom).clamp(3.0, 12.0);
        let edges = [
            egui::pos2(rect.center().x, rect.top()),
            egui::pos2(rect.center().x, rect.bottom()),
            egui::pos2(rect.left(), rect.center().y),
            egui::pos2(rect.right(), rect.center().y),
        ];

        for pos in edges {
            painter.circle_filled(pos, edge_handle_radius, handle_color);
            painter.circle_stroke(pos, edge_handle_radius, handle_stroke);
        }
    }

    /// Draw blend gradient overlays for overlapping screens in the environment preview
    fn draw_screen_blend_overlaps(
        &self,
        ui: &egui::Ui,
        preview_rect: egui::Rect,
        preview_size: egui::Vec2,
        screens: &[&Screen],
        content_size: (f32, f32),
    ) {
        // Create a clipped painter that only draws within preview_rect
        let painter = ui.painter().with_clip_rect(preview_rect);

        // Get transform rects and edge blend configs for all screens
        let screen_data: Vec<_> = screens
            .iter()
            .filter_map(|screen| {
                let slice = screen.slices.first()?;
                let transform_rect = Rect {
                    x: slice.output.rect.x,
                    y: slice.output.rect.y,
                    width: slice.output.rect.width,
                    height: slice.output.rect.height,
                };
                Some((screen.id, transform_rect, &slice.output.edge_blend))
            })
            .collect();

        // Check each pair of screens for overlaps
        for i in 0..screen_data.len() {
            for j in (i + 1)..screen_data.len() {
                let (_id_a, rect_a, blend_a) = &screen_data[i];
                let (_id_b, rect_b, blend_b) = &screen_data[j];

                // Compute intersection of the two input_rects
                let intersection = Self::rect_intersection(rect_a, rect_b);
                if intersection.is_none() {
                    continue;
                }
                let intersection = intersection.unwrap();

                // Skip very small overlaps
                if intersection.width < 0.01 || intersection.height < 0.01 {
                    continue;
                }

                // Convert intersection to screen coordinates using viewport transform
                let overlap_rect = self.transform_rect_to_screen(
                    &self.env_viewport,
                    &intersection,
                    preview_rect,
                    preview_size,
                    content_size,
                );

                // Skip if completely outside preview area
                if !overlap_rect.intersects(preview_rect) {
                    continue;
                }

                // Determine which blend edges are active between these screens
                // Horizontal overlap: A's right meets B's left, or B's right meets A's left
                let a_right_of_b = rect_a.x > rect_b.x;
                let (left_blend, right_blend, left_gamma, right_gamma) = if a_right_of_b {
                    // B is to the left, A is to the right
                    (blend_b.right.enabled, blend_a.left.enabled, blend_b.right.gamma, blend_a.left.gamma)
                } else {
                    // A is to the left, B is to the right
                    (blend_a.right.enabled, blend_b.left.enabled, blend_a.right.gamma, blend_b.left.gamma)
                };

                // Vertical overlap: A's bottom meets B's top, or B's bottom meets A's top
                let a_below_b = rect_a.y > rect_b.y;
                let (top_blend, bottom_blend, top_gamma, bottom_gamma) = if a_below_b {
                    // B is above, A is below
                    (blend_b.bottom.enabled, blend_a.top.enabled, blend_b.bottom.gamma, blend_a.top.gamma)
                } else {
                    // A is above, B is below
                    (blend_a.bottom.enabled, blend_b.top.enabled, blend_a.bottom.gamma, blend_b.top.gamma)
                };

                // Draw horizontal blend gradient if both edges are enabled
                if left_blend && right_blend {
                    let avg_gamma = (left_gamma + right_gamma) / 2.0;
                    self.draw_blend_gradient(
                        &painter,
                        overlap_rect,
                        true, // horizontal
                        16,
                        avg_gamma,
                    );
                }

                // Draw vertical blend gradient if both edges are enabled
                if top_blend && bottom_blend {
                    let avg_gamma = (top_gamma + bottom_gamma) / 2.0;
                    self.draw_blend_gradient(
                        &painter,
                        overlap_rect,
                        false, // vertical
                        16,
                        avg_gamma,
                    );
                }
            }
        }
    }

    /// Compute intersection of two normalized Rects, returning None if no overlap
    fn rect_intersection(a: &Rect, b: &Rect) -> Option<Rect> {
        let left = a.x.max(b.x);
        let top = a.y.max(b.y);
        let right = (a.x + a.width).min(b.x + b.width);
        let bottom = (a.y + a.height).min(b.y + b.height);

        if left < right && top < bottom {
            Some(Rect {
                x: left,
                y: top,
                width: right - left,
                height: bottom - top,
            })
        } else {
            None
        }
    }

    /// Draw a blend gradient using strips of varying alpha
    fn draw_blend_gradient(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        horizontal: bool,
        num_strips: usize,
        gamma: f32,
    ) {
        let base_color = egui::Color32::from_rgb(255, 200, 0); // Amber
        let num_strips = num_strips.max(4);

        for i in 0..num_strips {
            let t = i as f32 / (num_strips - 1) as f32;

            // Compute alpha based on blend curve
            // Two overlapping gradients: one from left/top, one from right/bottom
            // Left/top gradient: alpha = pow(t, gamma) (increases from 0 to 1)
            // Right/bottom gradient: alpha = pow(1-t, gamma) (decreases from 1 to 0)
            // Combined: multiply them together for the characteristic blend curve
            let alpha_left = t.powf(gamma);
            let alpha_right = (1.0 - t).powf(gamma);
            let combined_alpha = (alpha_left * alpha_right * 4.0).min(1.0); // Scale and clamp

            let alpha = (combined_alpha * 80.0) as u8; // Max alpha of 80 for visibility
            let color = egui::Color32::from_rgba_unmultiplied(
                base_color.r(),
                base_color.g(),
                base_color.b(),
                alpha,
            );

            let strip_rect = if horizontal {
                let strip_width = rect.width() / num_strips as f32;
                egui::Rect::from_min_size(
                    egui::pos2(rect.left() + i as f32 * strip_width, rect.top()),
                    egui::vec2(strip_width + 1.0, rect.height()), // +1 to avoid gaps
                )
            } else {
                let strip_height = rect.height() / num_strips as f32;
                egui::Rect::from_min_size(
                    egui::pos2(rect.left(), rect.top() + i as f32 * strip_height),
                    egui::vec2(rect.width(), strip_height + 1.0), // +1 to avoid gaps
                )
            };

            painter.rect_filled(strip_rect, 0.0, color);
        }
    }

    /// Hit test to find which handle (if any) is under the pointer
    fn hit_test_rect_handle(
        &self,
        pointer_pos: egui::Pos2,
        rect: egui::Rect,
    ) -> Option<RectHandle> {
        let handle_radius = 15.0; // Larger radius for easier grabbing

        // Check corner handles first (priority)
        let corners = [
            (rect.left_top(), RectHandle::TopLeft),
            (rect.right_top(), RectHandle::TopRight),
            (rect.left_bottom(), RectHandle::BottomLeft),
            (rect.right_bottom(), RectHandle::BottomRight),
        ];

        for (pos, handle) in corners {
            if pointer_pos.distance(pos) < handle_radius {
                return Some(handle);
            }
        }

        // Check edge midpoints
        let edges = [
            (egui::pos2(rect.center().x, rect.top()), RectHandle::TopEdge),
            (egui::pos2(rect.center().x, rect.bottom()), RectHandle::BottomEdge),
            (egui::pos2(rect.left(), rect.center().y), RectHandle::LeftEdge),
            (egui::pos2(rect.right(), rect.center().y), RectHandle::RightEdge),
        ];

        for (pos, handle) in edges {
            if pointer_pos.distance(pos) < handle_radius {
                return Some(handle);
            }
        }

        // Check if inside the rectangle (for body drag)
        if rect.contains(pointer_pos) {
            return Some(RectHandle::Body);
        }

        None
    }

    /// Get the appropriate cursor for a handle
    fn cursor_for_handle(handle: RectHandle) -> egui::CursorIcon {
        match handle {
            RectHandle::TopLeft | RectHandle::BottomRight => egui::CursorIcon::ResizeNwSe,
            RectHandle::TopRight | RectHandle::BottomLeft => egui::CursorIcon::ResizeNeSw,
            RectHandle::TopEdge | RectHandle::BottomEdge => egui::CursorIcon::ResizeVertical,
            RectHandle::LeftEdge | RectHandle::RightEdge => egui::CursorIcon::ResizeHorizontal,
            RectHandle::Body => egui::CursorIcon::Grab,
        }
    }

    /// Handle click-to-select and drag-to-edit interactions in environment preview
    fn handle_environment_interactions(
        &mut self,
        ui: &egui::Ui,
        preview_rect: egui::Rect,
        preview_size: egui::Vec2,
        screens: &[&Screen],
        actions: &mut Vec<AdvancedOutputAction>,
        response: &egui::Response,
        content_size: (f32, f32),
    ) {
        // Skip left-click handling if right-click dragging (viewport pan takes priority)
        if response.dragged_by(egui::PointerButton::Secondary) {
            return;
        }

        // Helper to find slice at position (using viewport transform)
        // Returns (screen_id, slice_id, input_rect, screen_rect)
        // Iterates in reverse so topmost (last drawn) wins on overlap
        let find_slice_at = |this: &Self, pos: egui::Pos2| -> Option<(ScreenId, SliceId, Rect, egui::Rect)> {
            for screen in screens.iter().rev() {
                for slice in screen.slices.iter().rev() {
                    let screen_rect = this.transform_rect_to_screen(
                        &this.env_viewport,
                        &slice.input_rect,
                        preview_rect,
                        preview_size,
                        content_size,
                    );

                    if screen_rect.contains(pos) {
                        return Some((screen.id, slice.id, slice.input_rect, screen_rect));
                    }
                }
            }
            None
        };

        // Get the currently selected slice's rect for handle detection
        let get_selected_slice_rect = |this: &Self| -> Option<(Rect, egui::Rect)> {
            if let (Some(screen_id), Some(slice_id)) = (this.selected_screen, this.selected_slice) {
                if let Some(screen) = screens.iter().find(|s| s.id == screen_id) {
                    if let Some(slice) = screen.slices.iter().find(|sl| sl.id == slice_id) {
                        let screen_rect = this.transform_rect_to_screen(
                            &this.env_viewport,
                            &slice.input_rect,
                            preview_rect,
                            preview_size,
                            content_size,
                        );
                        return Some((slice.input_rect, screen_rect));
                    }
                }
            }
            None
        };

        // Set cursor based on what's under the pointer
        if let Some(pointer_pos) = response.hover_pos() {
            let mut cursor_set = false;

            // If we're dragging, show the appropriate cursor
            if self.input_rect_drag.dragging_handle.is_some() {
                if let Some(handle) = self.input_rect_drag.dragging_handle {
                    let cursor = if handle == RectHandle::Body {
                        egui::CursorIcon::Grabbing
                    } else {
                        Self::cursor_for_handle(handle)
                    };
                    ui.ctx().set_cursor_icon(cursor);
                    cursor_set = true;
                }
            }

            // Check hover over selected slice's handles
            if !cursor_set {
                if let Some((_, screen_rect)) = get_selected_slice_rect(self) {
                    if let Some(handle) = self.hit_test_rect_handle(pointer_pos, screen_rect) {
                        ui.ctx().set_cursor_icon(Self::cursor_for_handle(handle));
                        cursor_set = true;
                    }
                }
            }

            // Check hover over any slice (for potential selection)
            if !cursor_set {
                if find_slice_at(self, pointer_pos).is_some() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }
        }

        // Handle click to select (fires on mouse release without drag)
        // Skip if we're in an active drag to prevent focus switch when dragging over other slices
        if response.clicked() && self.input_rect_drag.dragging_screen.is_none() {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                if let Some((screen_id, slice_id, _, _)) = find_slice_at(self, pointer_pos) {
                    self.selected_screen = Some(screen_id);
                    self.selected_slice = Some(slice_id);
                    // Expand the screen to show slices
                    self.expanded_screens.insert(screen_id);
                    if let Some(screen) = screens.iter().find(|s| s.id == screen_id) {
                        if let Some(slice) = screen.slices.iter().find(|sl| sl.id == slice_id) {
                            self.temp_slice_name = slice.name.clone();
                        }
                        self.temp_screen_name = screen.name.clone();
                        self.temp_width = screen.width.to_string();
                        self.temp_height = screen.height.to_string();
                    }
                }
            }
        }

        // Handle drag start - also select the slice if clicking on one
        // Only for left-click drags (primary button)
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                // First, try to find a slice under the pointer
                if let Some((screen_id, slice_id, input_rect, screen_rect)) = find_slice_at(self, pointer_pos) {
                    // Select this slice
                    self.selected_screen = Some(screen_id);
                    self.selected_slice = Some(slice_id);
                    self.expanded_screens.insert(screen_id);
                    if let Some(screen) = screens.iter().find(|s| s.id == screen_id) {
                        if let Some(slice) = screen.slices.iter().find(|sl| sl.id == slice_id) {
                            self.temp_slice_name = slice.name.clone();
                        }
                        self.temp_screen_name = screen.name.clone();
                        self.temp_width = screen.width.to_string();
                        self.temp_height = screen.height.to_string();
                    }

                    // Check if we're on a handle of this slice
                    if let Some(handle) = self.hit_test_rect_handle(pointer_pos, screen_rect) {
                        // Store starting position (normalized, using viewport inverse transform)
                        let (start_norm_x, start_norm_y) = self.transform_point_from_screen(
                            &self.env_viewport,
                            pointer_pos,
                            preview_rect,
                            preview_size,
                            content_size,
                        );

                        self.input_rect_drag = InputRectDragState {
                            dragging_screen: Some(screen_id),
                            dragging_slice: Some(slice_id),
                            dragging_handle: Some(handle),
                            original_rect: Some(input_rect),
                            start_pos: Some([start_norm_x, start_norm_y]),
                        };
                    }
                } else if let Some((input_rect, screen_rect)) = get_selected_slice_rect(self) {
                    // No slice under pointer, but we have a selected slice
                    // Check if we're on its handles
                    if let Some(handle) = self.hit_test_rect_handle(pointer_pos, screen_rect) {
                        let (start_norm_x, start_norm_y) = self.transform_point_from_screen(
                            &self.env_viewport,
                            pointer_pos,
                            preview_rect,
                            preview_size,
                            content_size,
                        );

                        self.input_rect_drag = InputRectDragState {
                            dragging_screen: self.selected_screen,
                            dragging_slice: self.selected_slice,
                            dragging_handle: Some(handle),
                            original_rect: Some(input_rect),
                            start_pos: Some([start_norm_x, start_norm_y]),
                        };
                    }
                }
            }
        }

        // Handle drag (left-click only)
        if response.dragged_by(egui::PointerButton::Primary) {
            if let (Some(screen_id), Some(slice_id)) = (self.input_rect_drag.dragging_screen, self.input_rect_drag.dragging_slice) {
                if let Some(handle) = self.input_rect_drag.dragging_handle {
                    if let Some(original_rect) = self.input_rect_drag.original_rect {
                        if let Some(start_pos) = self.input_rect_drag.start_pos {
                            if let Some(pointer_pos) = response.interact_pointer_pos() {
                                let new_rect = self.compute_dragged_rect_with_viewport(
                                    pointer_pos,
                                    preview_rect,
                                    preview_size,
                                    original_rect,
                                    handle,
                                    start_pos,
                                    content_size,
                                    false, // use env_viewport
                                );

                                actions.push(AdvancedOutputAction::UpdateSliceInputRect {
                                    screen_id,
                                    slice_id,
                                    input_rect: new_rect,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Handle drag end
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            self.input_rect_drag = InputRectDragState::default();
        }

        // Handle right-click to reset slice input_rect to full
        if response.clicked_by(egui::PointerButton::Secondary) {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                // Check if clicking on a slice
                if let Some((screen_id, slice_id, _, screen_rect)) = find_slice_at(self, pointer_pos) {
                    if self.hit_test_rect_handle(pointer_pos, screen_rect).is_some() {
                        actions.push(AdvancedOutputAction::UpdateSliceInputRect {
                            screen_id,
                            slice_id,
                            input_rect: Rect::full(),
                        });
                    }
                }
            }
        }
    }

    /// Compute the new rect based on which handle is being dragged (viewport-aware)
    /// If use_output_viewport is true, uses output_viewport; otherwise uses env_viewport
    fn compute_dragged_rect_with_viewport(
        &self,
        pointer_pos: egui::Pos2,
        preview_rect: egui::Rect,
        preview_size: egui::Vec2,
        original_rect: Rect,
        handle: RectHandle,
        start_pos: [f32; 2],
        content_size: (f32, f32),
        use_output_viewport: bool,
    ) -> Rect {
        // Convert pointer position to normalized coordinates using viewport inverse transform
        let viewport = if use_output_viewport { &self.output_viewport } else { &self.env_viewport };
        let (norm_x, norm_y) = self.transform_point_from_screen(
            viewport,
            pointer_pos,
            preview_rect,
            preview_size,
            content_size,
        );

        let mut new_rect = original_rect;
        let min_size = 0.05; // Minimum 5% size

        match handle {
            RectHandle::TopLeft => {
                let right = original_rect.x + original_rect.width;
                let bottom = original_rect.y + original_rect.height;
                new_rect.x = norm_x.min(right - min_size);
                new_rect.y = norm_y.min(bottom - min_size);
                new_rect.width = right - new_rect.x;
                new_rect.height = bottom - new_rect.y;
            }
            RectHandle::TopRight => {
                let bottom = original_rect.y + original_rect.height;
                new_rect.y = norm_y.min(bottom - min_size);
                new_rect.width = (norm_x - original_rect.x).max(min_size);
                new_rect.height = bottom - new_rect.y;
            }
            RectHandle::BottomLeft => {
                let right = original_rect.x + original_rect.width;
                new_rect.x = norm_x.min(right - min_size);
                new_rect.width = right - new_rect.x;
                new_rect.height = (norm_y - original_rect.y).max(min_size);
            }
            RectHandle::BottomRight => {
                new_rect.width = (norm_x - original_rect.x).max(min_size);
                new_rect.height = (norm_y - original_rect.y).max(min_size);
            }
            RectHandle::TopEdge => {
                let bottom = original_rect.y + original_rect.height;
                new_rect.y = norm_y.min(bottom - min_size);
                new_rect.height = bottom - new_rect.y;
            }
            RectHandle::BottomEdge => {
                new_rect.height = (norm_y - original_rect.y).max(min_size);
            }
            RectHandle::LeftEdge => {
                let right = original_rect.x + original_rect.width;
                new_rect.x = norm_x.min(right - min_size);
                new_rect.width = right - new_rect.x;
            }
            RectHandle::RightEdge => {
                new_rect.width = (norm_x - original_rect.x).max(min_size);
            }
            RectHandle::Body => {
                // Move the entire rect by total delta from start position
                let delta_norm_x = norm_x - start_pos[0];
                let delta_norm_y = norm_y - start_pos[1];
                new_rect.x = original_rect.x + delta_norm_x;
                new_rect.y = original_rect.y + delta_norm_y;
            }
        }

        new_rect.clamped()
    }

    /// Render simplified screen properties for the Screens tab
    fn render_screen_properties_simplified(
        &mut self,
        ui: &mut egui::Ui,
        screen: &Screen,
        all_screens: &[&Screen],
        available_displays: &[DisplayInfo],
        env_dimensions: (u32, u32),
        actions: &mut Vec<AdvancedOutputAction>,
    ) {
        let mut changed = false;
        let mut screen_copy = screen.clone();
        let (env_width, env_height) = env_dimensions;

        // Name
        ui.horizontal(|ui| {
            ui.label("Name:");
            if ui.text_edit_singleline(&mut self.temp_screen_name).changed() {
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
                egui::TextEdit::singleline(&mut self.temp_width).desired_width(50.0),
            );
            ui.label("H:");
            let height_response = ui.add(
                egui::TextEdit::singleline(&mut self.temp_height).desired_width(50.0),
            );

            if width_response.lost_focus() || height_response.lost_focus() {
                if let (Ok(w), Ok(h)) = (self.temp_width.parse::<u32>(), self.temp_height.parse::<u32>()) {
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

        // Device type selector (full options)
        // Show actual display name when Display is selected
        let current_device_type = match &screen.device {
            OutputDevice::Display { display_id } => {
                available_displays
                    .iter()
                    .find(|d| d.id == *display_id)
                    .map(|d| d.label())
                    .unwrap_or_else(|| format!("Display {} (disconnected)", display_id))
            }
            _ => screen.device.type_name().to_string(),
        };
        egui::ComboBox::from_id_salt("device_type_screens_tab")
            .selected_text(&current_device_type)
            .show_ui(ui, |ui| {
                // Virtual
                if ui.selectable_label(matches!(screen_copy.device, OutputDevice::Virtual), "Virtual").clicked() {
                    screen_copy.device = OutputDevice::Virtual;
                    changed = true;
                }

                // Display header with individual displays below (only if displays available)
                if !available_displays.is_empty() {
                    ui.separator();
                    ui.add_enabled(false, egui::SelectableLabel::new(false,
                        egui::RichText::new("Display").strong()));

                    // Individual displays indented
                    for display in available_displays {
                        let is_selected = matches!(&screen_copy.device, OutputDevice::Display { display_id } if *display_id == display.id);
                        let label = format!("  {}", display.label());
                        if ui.selectable_label(is_selected, &label).clicked() {
                            screen_copy.device = OutputDevice::Display { display_id: display.id };
                            changed = true;
                        }
                    }
                    ui.separator();
                }

                // NDI
                if ui.selectable_label(matches!(screen_copy.device, OutputDevice::Ndi { .. }), "NDI").clicked() {
                    let base_name = if self.temp_device_name.is_empty() {
                        format!("{} NDI", screen.name)
                    } else {
                        self.temp_device_name.clone()
                    };
                    // Ensure unique NDI name to avoid conflicts
                    let name = Self::unique_ndi_name(&base_name, screen.id, all_screens);
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
            // Display warnings (selection now done in main device dropdown)
            let current_display = available_displays.iter().find(|d| d.id == display_id);
            let is_disconnected = current_display.is_none();

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

            // Show warning if primary display is selected (UI may be covered)
            if let Some(display) = current_display {
                if display.is_primary {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("⚠").color(egui::Color32::YELLOW));
                        ui.label(
                            egui::RichText::new("Primary display - UI may be covered (press Escape to close)")
                                .color(egui::Color32::YELLOW)
                                .italics()
                        );
                    });
                }
            }
        } else if matches!(screen_copy.device, OutputDevice::Ndi { .. }) {
            // NDI name editor
            if let Some(ref name) = current_name {
                if self.temp_device_name.is_empty() || !self.temp_device_name.eq(name) {
                    self.temp_device_name = name.clone();
                }
            }

            // Check for name conflicts with other screens
            let has_conflict = all_screens.iter()
                .filter(|s| s.id != screen.id)
                .any(|s| {
                    if let OutputDevice::Ndi { name } = &s.device {
                        name == &self.temp_device_name
                    } else {
                        false
                    }
                });

            ui.horizontal(|ui| {
                ui.label("Name:");
                let response = ui.text_edit_singleline(&mut self.temp_device_name);
                if response.changed() {
                    screen_copy.device = OutputDevice::Ndi {
                        name: self.temp_device_name.clone(),
                    };
                    changed = true;
                }
                if has_conflict {
                    ui.colored_label(egui::Color32::from_rgb(255, 180, 0), "⚠");
                }
            });
            if has_conflict {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 180, 0),
                    "Warning: Name conflicts with another screen's NDI output"
                );
            }
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

        // Transform section (displayed in pixels)
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Transform").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Full").on_hover_text("Reset to full environment").clicked() {
                    actions.push(AdvancedOutputAction::UpdateScreenInputRect {
                        screen_id: screen.id,
                        input_rect: Rect::full(),
                    });
                }
            });
        });
        ui.add_space(4.0);

        // Show current input_rect (which part of environment to capture), converted to pixels
        let transform_rect = screen.slices.first()
            .map(|s| s.input_rect)
            .unwrap_or_else(Rect::full);

        // Convert normalized values to pixels for display (allow negative for overscan)
        let x_px = (transform_rect.x * env_width as f32).round() as i32;
        let y_px = (transform_rect.y * env_height as f32).round() as i32;
        let w_px = (transform_rect.width * env_width as f32).round() as i32;
        let h_px = (transform_rect.height * env_height as f32).round() as i32;

        ui.horizontal(|ui| {
            ui.label("X:");
            let mut x = x_px;
            if ui.add(egui::DragValue::new(&mut x).range(-(env_width as i32)..=(env_width as i32 * 2)).speed(1.0).suffix("px")).changed() {
                let mut new_rect = transform_rect;
                new_rect.x = x as f32 / env_width as f32;
                actions.push(AdvancedOutputAction::UpdateScreenInputRect {
                    screen_id: screen.id,
                    input_rect: new_rect,
                });
            }
            ui.label("Y:");
            let mut y = y_px;
            if ui.add(egui::DragValue::new(&mut y).range(-(env_height as i32)..=(env_height as i32 * 2)).speed(1.0).suffix("px")).changed() {
                let mut new_rect = transform_rect;
                new_rect.y = y as f32 / env_height as f32;
                actions.push(AdvancedOutputAction::UpdateScreenInputRect {
                    screen_id: screen.id,
                    input_rect: new_rect,
                });
            }
        });

        ui.horizontal(|ui| {
            ui.label("W:");
            let mut w = w_px;
            if ui.add(egui::DragValue::new(&mut w).range(1..=(env_width as i32 * 3)).speed(1.0).suffix("px")).changed() {
                let mut new_rect = transform_rect;
                new_rect.width = (w as f32 / env_width as f32).max(0.01);
                actions.push(AdvancedOutputAction::UpdateScreenInputRect {
                    screen_id: screen.id,
                    input_rect: new_rect,
                });
            }
            ui.label("H:");
            let mut h = h_px;
            if ui.add(egui::DragValue::new(&mut h).range(1..=(env_height as i32 * 3)).speed(1.0).suffix("px")).changed() {
                let mut new_rect = transform_rect;
                new_rect.height = (h as f32 / env_height as f32).max(0.01);
                actions.push(AdvancedOutputAction::UpdateScreenInputRect {
                    screen_id: screen.id,
                    input_rect: new_rect,
                });
            }
        });

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

            let response = ui.add(slider);
            if response.changed() {
                screen_copy.delay_ms = delay_val.max(0) as u32;
                changed = true;
            }
            // Right-click instantly resets to 0
            if response.clicked_by(PointerButton::Secondary) {
                screen_copy.delay_ms = 0;
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
            let response = ui.add(egui::Slider::new(&mut screen_copy.color.brightness, -1.0..=1.0).max_decimals(2));
            if response.changed() {
                changed = true;
            }
            // Right-click instantly resets to 0
            if response.clicked_by(PointerButton::Secondary) {
                screen_copy.color.brightness = 0.0;
                changed = true;
            }
        });

        // Contrast slider
        ui.horizontal(|ui| {
            ui.label("Contrast:");
            let response = ui.add(egui::Slider::new(&mut screen_copy.color.contrast, 0.0..=2.0).max_decimals(2));
            if response.changed() {
                changed = true;
            }
            // Right-click instantly resets to 1
            if response.clicked_by(PointerButton::Secondary) {
                screen_copy.color.contrast = 1.0;
                changed = true;
            }
        });

        // Gamma slider
        ui.horizontal(|ui| {
            ui.label("Gamma:");
            let response = ui.add(egui::Slider::new(&mut screen_copy.color.gamma, 0.1..=4.0).logarithmic(true).max_decimals(2));
            if response.changed() {
                changed = true;
            }
            // Right-click instantly resets to 1
            if response.clicked_by(PointerButton::Secondary) {
                screen_copy.color.gamma = 1.0;
                changed = true;
            }
        });

        // Saturation slider
        ui.horizontal(|ui| {
            ui.label("Saturation:");
            let response = ui.add(egui::Slider::new(&mut screen_copy.color.saturation, 0.0..=2.0).max_decimals(2));
            if response.changed() {
                changed = true;
            }
            // Right-click instantly resets to 1
            if response.clicked_by(PointerButton::Secondary) {
                screen_copy.color.saturation = 1.0;
                changed = true;
            }
        });

        // RGB Channels (collapsing section)
        ui.collapsing("RGB Channels", |ui| {
            ui.horizontal(|ui| {
                ui.label("Red:");
                let response = ui.add(egui::Slider::new(&mut screen_copy.color.red, 0.0..=2.0).max_decimals(2));
                if response.changed() {
                    changed = true;
                }
                // Right-click instantly resets to 1
                if response.clicked_by(PointerButton::Secondary) {
                    screen_copy.color.red = 1.0;
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Green:");
                let response = ui.add(egui::Slider::new(&mut screen_copy.color.green, 0.0..=2.0).max_decimals(2));
                if response.changed() {
                    changed = true;
                }
                // Right-click instantly resets to 1
                if response.clicked_by(PointerButton::Secondary) {
                    screen_copy.color.green = 1.0;
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Blue:");
                let response = ui.add(egui::Slider::new(&mut screen_copy.color.blue, 0.0..=2.0).max_decimals(2));
                if response.changed() {
                    changed = true;
                }
                // Right-click instantly resets to 1
                if response.clicked_by(PointerButton::Secondary) {
                    screen_copy.color.blue = 1.0;
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

        // Edge Blend section (for first slice) - dynamic based on overlaps
        if let Some(first_slice) = screen.slices.first() {
            let mut slice_copy = first_slice.clone();
            let mut blend_changed = false;

            // Calculate which edges overlap with other screens and by how much
            let my_rect = first_slice.input_rect;
            let mut left_overlap_width: f32 = 0.0;
            let mut right_overlap_width: f32 = 0.0;
            let mut top_overlap_width: f32 = 0.0;
            let mut bottom_overlap_width: f32 = 0.0;

            for other_screen in all_screens.iter() {
                if other_screen.id == screen.id {
                    continue;
                }
                if let Some(other_slice) = other_screen.slices.first() {
                    let other_rect = other_slice.input_rect;
                    if let Some(intersection) = Self::rect_intersection(&my_rect, &other_rect) {
                        // Determine which edge based on relative positions
                        let my_center_x = my_rect.x + my_rect.width / 2.0;
                        let other_center_x = other_rect.x + other_rect.width / 2.0;
                        let my_center_y = my_rect.y + my_rect.height / 2.0;
                        let other_center_y = other_rect.y + other_rect.height / 2.0;

                        // Calculate overlap as fraction of screen dimension (clamped to 0.5 max)
                        let h_overlap = (intersection.width / my_rect.width).min(0.5);
                        let v_overlap = (intersection.height / my_rect.height).min(0.5);

                        if other_center_x < my_center_x {
                            left_overlap_width = left_overlap_width.max(h_overlap);
                        }
                        if other_center_x > my_center_x {
                            right_overlap_width = right_overlap_width.max(h_overlap);
                        }
                        if other_center_y < my_center_y {
                            top_overlap_width = top_overlap_width.max(v_overlap);
                        }
                        if other_center_y > my_center_y {
                            bottom_overlap_width = bottom_overlap_width.max(v_overlap);
                        }
                    }
                }
            }

            let has_left_overlap = left_overlap_width > 0.0;
            let has_right_overlap = right_overlap_width > 0.0;
            let has_top_overlap = top_overlap_width > 0.0;
            let has_bottom_overlap = bottom_overlap_width > 0.0;
            let has_any_overlap = has_left_overlap || has_right_overlap || has_top_overlap || has_bottom_overlap;

            // Always show blend section
            ui.label(egui::RichText::new("Edge Blend").strong());
            ui.add_space(4.0);

            // Show overlap info
            if has_any_overlap {
                let mut overlap_labels = Vec::new();
                if has_left_overlap { overlap_labels.push(format!("Left {:.0}%", left_overlap_width * 100.0)); }
                if has_right_overlap { overlap_labels.push(format!("Right {:.0}%", right_overlap_width * 100.0)); }
                if has_top_overlap { overlap_labels.push(format!("Top {:.0}%", top_overlap_width * 100.0)); }
                if has_bottom_overlap { overlap_labels.push(format!("Bottom {:.0}%", bottom_overlap_width * 100.0)); }
                ui.label(egui::RichText::new(format!("Overlaps: {}", overlap_labels.join(", "))).small().weak());
            } else {
                ui.label(egui::RichText::new("No overlaps detected").small().weak());
            }
            ui.add_space(4.0);

            // Master enable toggle - always visible and always works
            let is_enabled = slice_copy.output.edge_blend.is_any_enabled();
            let mut enable_blend = is_enabled;
            if ui.checkbox(&mut enable_blend, "Enable Blending").changed() {
                if enable_blend {
                    // Enable overlapping edges with width based on actual overlap
                    // If no overlaps, enable all edges with default (will have no effect until overlap happens)
                    let edge = &mut slice_copy.output.edge_blend;
                    if has_any_overlap {
                        if has_left_overlap {
                            edge.left.enabled = true;
                            edge.left.width = left_overlap_width;
                            edge.left.gamma = 2.2;
                        }
                        if has_right_overlap {
                            edge.right.enabled = true;
                            edge.right.width = right_overlap_width;
                            edge.right.gamma = 2.2;
                        }
                        if has_top_overlap {
                            edge.top.enabled = true;
                            edge.top.width = top_overlap_width;
                            edge.top.gamma = 2.2;
                        }
                        if has_bottom_overlap {
                            edge.bottom.enabled = true;
                            edge.bottom.width = bottom_overlap_width;
                            edge.bottom.gamma = 2.2;
                        }
                    } else {
                        // No overlaps yet - enable all edges so blending kicks in when overlaps happen
                        *edge = EdgeBlendConfig::all(0.15, 2.2);
                    }
                } else {
                    slice_copy.output.edge_blend.disable_all();
                }
                blend_changed = true;
            }

            // Show sliders when blending is enabled
            if enable_blend {
                ui.add_space(4.0);

                // Get current values (use first enabled edge as reference)
                let edge = &mut slice_copy.output.edge_blend;
                let ref_edge = if edge.left.enabled { &edge.left }
                    else if edge.right.enabled { &edge.right }
                    else if edge.top.enabled { &edge.top }
                    else { &edge.bottom };
                let mut gamma = ref_edge.gamma;
                let mut luminance = ref_edge.black_level;
                let mut power = ref_edge.width;

                // Gamma slider
                ui.horizontal(|ui| {
                    ui.label("Gamma:");
                    let response = ui.add(egui::Slider::new(&mut gamma, 0.1..=4.0).max_decimals(1));
                    if response.changed() {
                        edge.left.gamma = gamma;
                        edge.right.gamma = gamma;
                        edge.top.gamma = gamma;
                        edge.bottom.gamma = gamma;
                        blend_changed = true;
                    }
                    // Right-click instantly resets to 2.2
                    if response.clicked_by(PointerButton::Secondary) {
                        edge.left.gamma = 2.2;
                        edge.right.gamma = 2.2;
                        edge.top.gamma = 2.2;
                        edge.bottom.gamma = 2.2;
                        blend_changed = true;
                    }
                });

                // Luminance slider
                ui.horizontal(|ui| {
                    ui.label("Luminance:");
                    let response = ui.add(egui::Slider::new(&mut luminance, 0.0..=1.0).max_decimals(2));
                    if response.changed() {
                        edge.left.black_level = luminance;
                        edge.right.black_level = luminance;
                        edge.top.black_level = luminance;
                        edge.bottom.black_level = luminance;
                        blend_changed = true;
                    }
                    // Right-click instantly resets to 0
                    if response.clicked_by(PointerButton::Secondary) {
                        edge.left.black_level = 0.0;
                        edge.right.black_level = 0.0;
                        edge.top.black_level = 0.0;
                        edge.bottom.black_level = 0.0;
                        blend_changed = true;
                    }
                });

                // Power slider (blend width)
                ui.horizontal(|ui| {
                    ui.label("Power:");
                    let response = ui.add(egui::Slider::new(&mut power, 0.0..=0.5).max_decimals(2));
                    if response.changed() {
                        edge.left.width = power;
                        edge.right.width = power;
                        edge.top.width = power;
                        edge.bottom.width = power;
                        blend_changed = true;
                    }
                    // Right-click instantly resets to 0.15
                    if response.clicked_by(PointerButton::Secondary) {
                        edge.left.width = 0.15;
                        edge.right.width = 0.15;
                        edge.top.width = 0.15;
                        edge.bottom.width = 0.15;
                        blend_changed = true;
                    }
                });
            }

            if blend_changed {
                actions.push(AdvancedOutputAction::UpdateSlice {
                    screen_id: screen.id,
                    slice_id: first_slice.id,
                    slice: slice_copy,
                });
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
        }

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

    /// Render the Output Transformation tab (existing functionality)
    fn render_output_transformation_tab(
        &mut self,
        ui: &mut egui::Ui,
        screens: &[&Screen],
        layer_count: usize,
        actions: &mut Vec<AdvancedOutputAction>,
    ) {
        // Three-column layout: Screens | Preview | Properties
        // Use allocate_ui_with_layout to fill available vertical space
        let available = ui.available_size();
        ui.allocate_ui_with_layout(
            available,
            egui::Layout::left_to_right(egui::Align::TOP),
            |ui| {
            // LEFT COLUMN: Screens and Slices list (hierarchical)
            ui.vertical(|ui| {
                ui.set_min_width(150.0);
                ui.set_max_width(180.0);
                self.render_screen_slice_list(ui, screens, actions, "output_tab");
            });

            ui.separator();

            // MIDDLE COLUMN: Preview
            ui.vertical(|ui| {
                ui.set_min_width(200.0);
                ui.heading("Preview");

                // Edit mode toggle
                ui.horizontal(|ui| {
                    ui.label("Edit:");
                    ui.selectable_value(&mut self.output_edit_mode, OutputEditMode::Resize, "Resize");
                    ui.selectable_value(&mut self.output_edit_mode, OutputEditMode::MeshWarp, "Mesh Warp");
                });

                // Auto-create mesh when switching to MeshWarp mode if none exists
                if self.output_edit_mode == OutputEditMode::MeshWarp {
                    if let Some(screen_id) = self.selected_screen {
                        if let Some(slice_id) = self.selected_slice {
                            if let Some(screen) = screens.iter().find(|s| s.id == screen_id) {
                                if let Some(slice) = screen.slices.iter().find(|s| s.id == slice_id) {
                                    if slice.output.mesh.is_none() {
                                        // Create a default 4x4 mesh
                                        let mut updated_slice = slice.clone();
                                        updated_slice.output.mesh = Some(WarpMesh::new(4, 4));
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
                ui.add_space(4.0);

                // Preview area - use live texture if available
                // Get screen aspect ratio and dimensions (default to 16:9 1920x1080 if no screen selected)
                let (screen_width, screen_height) = self.selected_screen
                    .and_then(|screen_id| screens.iter().find(|s| s.id == screen_id))
                    .map(|screen| (screen.width as f32, screen.height as f32))
                    .unwrap_or((1920.0, 1080.0));
                let aspect_ratio = screen_width / screen_height;
                let output_content_size = (screen_width, screen_height);

                // Calculate preview size based on available space using screen's aspect ratio
                let available_height = (ui.available_height() - 100.0).max(100.0).min(400.0);
                let preview_size = egui::vec2(available_height * aspect_ratio, available_height);
                let (rect, response) =
                    ui.allocate_exact_size(preview_size, egui::Sense::click_and_drag());

                // Handle viewport pan/zoom (right-click drag, scroll wheel)
                viewport_widget::handle_viewport_input(
                    ui,
                    &response,
                    rect,
                    &mut self.output_viewport,
                    output_content_size,
                    &ViewportConfig::default(),
                    "adv_output_transform",
                );

                // Draw preview background
                ui.painter().rect_filled(
                    rect,
                    4.0,
                    egui::Color32::from_rgb(30, 30, 30),
                );

                // Create a clipped painter that only draws within preview rect
                let painter = ui.painter().with_clip_rect(rect);

                if let Some(screen_id) = self.selected_screen {
                    if let Some(screen) = screens.iter().find(|s| s.id == screen_id) {
                        // Draw live texture if available and screen is enabled
                        if screen.enabled {
                            if let Some(tex_id) = self.preview_texture_id {
                                // Transform full screen (0,0)-(1,1) through viewport to get destination rect
                                let full_screen_rect = Rect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 };
                                let screen_dest_rect = self.transform_rect_to_screen(
                                    &self.output_viewport,
                                    &full_screen_rect,
                                    rect,
                                    preview_size,
                                    output_content_size,
                                );

                                // Calculate intersection with preview rect (clip)
                                let visible_rect = screen_dest_rect.intersect(rect);
                                if visible_rect.width() > 0.0 && visible_rect.height() > 0.0 {
                                    // Calculate UV coords for the visible portion
                                    let uv_left = (visible_rect.left() - screen_dest_rect.left()) / screen_dest_rect.width();
                                    let uv_top = (visible_rect.top() - screen_dest_rect.top()) / screen_dest_rect.height();
                                    let uv_right = (visible_rect.right() - screen_dest_rect.left()) / screen_dest_rect.width();
                                    let uv_bottom = (visible_rect.bottom() - screen_dest_rect.top()) / screen_dest_rect.height();
                                    let uv_rect = egui::Rect::from_min_max(
                                        egui::pos2(uv_left, uv_top),
                                        egui::pos2(uv_right, uv_bottom),
                                    );

                                    // Draw live preview texture
                                    painter.image(tex_id, visible_rect, uv_rect, egui::Color32::WHITE);
                                }
                            } else {
                                // Texture not yet registered
                                painter.text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    format!("{}x{}\n(loading...)", screen.width, screen.height),
                                    egui::FontId::proportional(12.0),
                                    egui::Color32::GRAY,
                                );
                            }
                        } else {
                            // Screen disabled
                            painter.text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                format!("{}x{}\n(disabled)", screen.width, screen.height),
                                egui::FontId::proportional(12.0),
                                egui::Color32::DARK_GRAY,
                            );
                        }

                        // Draw slice rectangles overlay with labels (using shared visualization)
                        // Find screen index for color assignment
                        let screen_idx = screens.iter().position(|s| s.id == screen.id).unwrap_or(0);
                        let zoom = self.output_viewport.zoom();

                        for (slice_idx, slice) in screen.slices.iter().enumerate() {
                            if slice.enabled {
                                // Convert slice.output.rect to our Rect for transform
                                let slice_output_rect = Rect {
                                    x: slice.output.rect.x,
                                    y: slice.output.rect.y,
                                    width: slice.output.rect.width,
                                    height: slice.output.rect.height,
                                };
                                let slice_rect = self.transform_rect_to_screen(
                                    &self.output_viewport,
                                    &slice_output_rect,
                                    rect,
                                    preview_size,
                                    output_content_size,
                                );

                                let is_selected = self.selected_slice == Some(slice.id);

                                // Use shared slice overlay drawing (consistent colors and labels)
                                Self::draw_slice_overlay(&painter, slice_rect, screen_idx, slice_idx, is_selected, zoom);

                                // Draw corner/edge handles for selected slice's output rect (only in Resize mode)
                                if is_selected && self.output_edit_mode == OutputEditMode::Resize {
                                    self.draw_rect_handles(&painter, slice_rect, zoom);
                                }

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
                                        painter.rect_filled(blend_rect, 0.0, blend_color);
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
                                        painter.rect_filled(blend_rect, 0.0, blend_color);
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
                                        painter.rect_filled(blend_rect, 0.0, blend_color);
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
                                        painter.rect_filled(blend_rect, 0.0, blend_color);
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
                                                    painter.line_segment(
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
                                                    painter.line_segment(
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
                                            painter.circle_filled(pos, 3.0, point_color);
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
                                                painter.rect_stroke(
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
                                                    painter.line_segment([p1, p2], egui::Stroke::new(2.0, mask_color));
                                                }
                                                // Draw center point
                                                painter.circle_filled(center_pos, 4.0, point_color);
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
                                                    painter.line_segment([pos1, pos2], egui::Stroke::new(2.0, mask_color));
                                                }
                                                // Draw vertices as draggable points
                                                for (i, p) in points.iter().enumerate() {
                                                    let pos = slice_rect.min + egui::vec2(
                                                        p.x * slice_rect.width(),
                                                        p.y * slice_rect.height(),
                                                    );
                                                    let is_dragging = self.dragging_mask_vertex == Some(i);
                                                    let radius = if is_dragging { 6.0 } else { 4.0 };
                                                    painter.circle_filled(pos, radius, point_color);
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
                                                        painter.line_segment([pos1, pos2], egui::Stroke::new(2.0, bezier_color));
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
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "No screen selected",
                        egui::FontId::proportional(12.0),
                        egui::Color32::DARK_GRAY,
                    );
                }

                // Handle output rect dragging for selected slice (only in Resize mode)
                // Skip left-click handling if right-click dragging (viewport pan takes priority)
                if self.output_edit_mode == OutputEditMode::Resize && !response.dragged_by(egui::PointerButton::Secondary) {
                    if let Some(screen_id) = self.selected_screen {
                        if let Some(slice_id) = self.selected_slice {
                            if let Some(screen) = screens.iter().find(|s| s.id == screen_id) {
                                if let Some(slice) = screen.slices.iter().find(|s| s.id == slice_id) {
                                    // Calculate slice rect in preview coordinates (with viewport transform)
                                    let slice_output_rect = Rect {
                                        x: slice.output.rect.x,
                                        y: slice.output.rect.y,
                                        width: slice.output.rect.width,
                                        height: slice.output.rect.height,
                                    };
                                    let slice_rect = self.transform_rect_to_screen(
                                        &self.output_viewport,
                                        &slice_output_rect,
                                        rect,
                                        preview_size,
                                        output_content_size,
                                    );

                                    // Set cursor based on what's under the pointer
                                    if let Some(pointer_pos) = response.hover_pos() {
                                        // If actively dragging, show appropriate cursor
                                        if self.output_rect_drag.dragging_handle.is_some() {
                                            if let Some(handle) = self.output_rect_drag.dragging_handle {
                                                let cursor = if handle == RectHandle::Body {
                                                    egui::CursorIcon::Grabbing
                                                } else {
                                                    Self::cursor_for_handle(handle)
                                                };
                                                ui.ctx().set_cursor_icon(cursor);
                                            }
                                        } else if let Some(handle) = self.hit_test_rect_handle(pointer_pos, slice_rect) {
                                            // Hovering over a handle
                                            ui.ctx().set_cursor_icon(Self::cursor_for_handle(handle));
                                        }
                                    }

                                    // Handle drag start
                                    if response.drag_started_by(egui::PointerButton::Primary) {
                                        // Only start output rect drag if not dragging warp points or mask vertices
                                        if self.dragging_warp_point.is_none() && self.dragging_mask_vertex.is_none() {
                                            if let Some(pointer_pos) = response.interact_pointer_pos() {
                                                if let Some(handle) = self.hit_test_rect_handle(pointer_pos, slice_rect) {
                                                    // Store starting position (normalized, using viewport inverse transform)
                                                    let (start_norm_x, start_norm_y) = self.transform_point_from_screen(
                                                        &self.output_viewport,
                                                        pointer_pos,
                                                        rect,
                                                        preview_size,
                                                        output_content_size,
                                                    );

                                                    self.output_rect_drag = OutputRectDragState {
                                                        dragging_slice: Some(slice_id),
                                                        dragging_handle: Some(handle),
                                                        original_rect: Some(slice_output_rect),
                                                        start_pos: Some([start_norm_x, start_norm_y]),
                                                    };
                                                }
                                            }
                                        }
                                    }

                                    // Handle drag
                                    if response.dragged_by(egui::PointerButton::Primary) {
                                        if let Some(dragging_slice_id) = self.output_rect_drag.dragging_slice {
                                            if dragging_slice_id == slice_id {
                                                if let Some(handle) = self.output_rect_drag.dragging_handle {
                                                    if let Some(original_rect) = self.output_rect_drag.original_rect {
                                                        if let Some(start_pos) = self.output_rect_drag.start_pos {
                                                            if let Some(pointer_pos) = response.interact_pointer_pos() {
                                                                let new_rect = self.compute_dragged_rect_with_viewport(
                                                                    pointer_pos,
                                                                    rect,
                                                                    preview_size,
                                                                    original_rect,
                                                                    handle,
                                                                    start_pos,
                                                                    output_content_size,
                                                                    true, // use output_viewport
                                                                );

                                                                // Update the slice with new output rect
                                                                let mut updated_slice = slice.clone();
                                                                updated_slice.output.rect.x = new_rect.x;
                                                                updated_slice.output.rect.y = new_rect.y;
                                                                updated_slice.output.rect.width = new_rect.width;
                                                                updated_slice.output.rect.height = new_rect.height;

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
                                    }

                                    // Handle drag end
                                    if response.drag_stopped_by(egui::PointerButton::Primary) {
                                        self.output_rect_drag = OutputRectDragState::default();
                                    }

                                    // Handle right-click to reset slice output_rect to full (only in Resize mode)
                                    if self.output_edit_mode == OutputEditMode::Resize && response.clicked_by(egui::PointerButton::Secondary) {
                                        if let Some(pointer_pos) = response.interact_pointer_pos() {
                                            if self.hit_test_rect_handle(pointer_pos, slice_rect).is_some() {
                                                let mut updated_slice = slice.clone();
                                                updated_slice.output.rect = Rect::full();
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
                    }
                }

                // Handle warp point dragging (only in MeshWarp mode)
                if self.output_edit_mode == OutputEditMode::MeshWarp {
                if let Some(screen_id) = self.selected_screen {
                    if let Some(slice_id) = self.selected_slice {
                        if let Some(screen) = screens.iter().find(|s| s.id == screen_id) {
                            if let Some(slice) = screen.slices.iter().find(|s| s.id == slice_id) {
                                if let Some(mesh) = &slice.output.mesh {
                                    // Calculate slice rect in preview coordinates (with viewport transform)
                                    let slice_input_rect = Rect {
                                        x: slice.output.rect.x,
                                        y: slice.output.rect.y,
                                        width: slice.output.rect.width,
                                        height: slice.output.rect.height,
                                    };
                                    let slice_rect = self.transform_rect_to_screen(
                                        &self.output_viewport,
                                        &slice_input_rect,
                                        rect,
                                        preview_size,
                                        output_content_size,
                                    );

                                    // On click start, find nearest warp point (only if not dragging output rect)
                                    if response.drag_started() && self.output_rect_drag.dragging_slice.is_none() {
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

                                    // Handle right-click to reset warp point to original UV position
                                    if response.clicked_by(egui::PointerButton::Secondary) {
                                        if let Some(pointer_pos) = response.interact_pointer_pos() {
                                            let click_radius = 15.0_f32;
                                            let mut clicked_point: Option<(usize, usize)> = None;

                                            for col in 0..mesh.columns {
                                                for row in 0..mesh.rows {
                                                    if let Some(point) = mesh.get_point(col, row) {
                                                        let pos = slice_rect.min + egui::vec2(
                                                            point.position[0] * slice_rect.width(),
                                                            point.position[1] * slice_rect.height(),
                                                        );
                                                        if pointer_pos.distance(pos) < click_radius {
                                                            clicked_point = Some((col, row));
                                                            break;
                                                        }
                                                    }
                                                }
                                                if clicked_point.is_some() { break; }
                                            }

                                            // Reset the clicked point to its original UV position
                                            if let Some((col, row)) = clicked_point {
                                                let mut updated_slice = slice.clone();
                                                if let Some(mesh) = &mut updated_slice.output.mesh {
                                                    if let Some(point) = mesh.get_point(col, row) {
                                                        mesh.set_point_position(col, row, point.uv[0], point.uv[1]);
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

                                            // On click start, find nearest mask vertex (only if not dragging output rect or warp point)
                                            if response.drag_started() && self.dragging_warp_point.is_none() && self.output_rect_drag.dragging_slice.is_none() {
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
                } // End of MeshWarp mode check

                // Draw zoom indicator in bottom-right corner
                viewport_widget::draw_zoom_indicator(ui, rect, &self.output_viewport);

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
                                    // Prompt to select a slice (screen properties are on Screens tab)
                                    ui.label(
                                        egui::RichText::new("Select a slice to edit transformations")
                                            .weak()
                                            .italics(),
                                    );
                                    ui.add_space(8.0);
                                    ui.label(
                                        egui::RichText::new("Screen properties (Output Device, Timing, Color) are on the Screens tab.")
                                            .small()
                                            .weak(),
                                    );
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

        // Transform rect (position and size on screen)
        ui.horizontal(|ui| {
            ui.label("Transform");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Preset button
                if ui.small_button("Full").on_hover_text("Fill entire screen (0,0,1,1)").clicked() {
                    slice_copy.output.rect.x = 0.0;
                    slice_copy.output.rect.y = 0.0;
                    slice_copy.output.rect.width = 1.0;
                    slice_copy.output.rect.height = 1.0;
                    changed = true;
                }
            });
        });
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("X:");
            let mut x = slice_copy.output.rect.x;
            if ui
                .add(egui::DragValue::new(&mut x).range(-1.0..=2.0).speed(0.01))
                .changed()
            {
                slice_copy.output.rect.x = x;
                changed = true;
            }
            ui.label("Y:");
            let mut y = slice_copy.output.rect.y;
            if ui
                .add(egui::DragValue::new(&mut y).range(-1.0..=2.0).speed(0.01))
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
                .add(egui::DragValue::new(&mut w).range(0.01..=3.0).speed(0.01))
                .changed()
            {
                slice_copy.output.rect.width = w;
                changed = true;
            }
            ui.label("H:");
            let mut h = slice_copy.output.rect.height;
            if ui
                .add(egui::DragValue::new(&mut h).range(0.01..=3.0).speed(0.01))
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

        // Mesh Warp (only show controls when in MeshWarp mode)
        if self.output_edit_mode == OutputEditMode::MeshWarp {
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

        // Mesh controls
        if slice_copy.output.mesh.is_some() {
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
        } // End MeshWarp mode section

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
                let response = ui.add(egui::Slider::new(&mut mask.feather, 0.0..=0.1).max_decimals(3));
                if response.changed() {
                    changed = true;
                }
                // Right-click instantly resets to 0
                if response.clicked_by(PointerButton::Secondary) {
                    mask.feather = 0.0;
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
                        let response = ui.add(egui::DragValue::new(x).range(0.0..=1.0).speed(0.01));
                        if response.changed() {
                            changed = true;
                        }
                        // Right-click instantly resets to 0.25
                        if response.clicked_by(PointerButton::Secondary) {
                            *x = 0.25;
                            changed = true;
                        }
                        ui.label("Y:");
                        let response = ui.add(egui::DragValue::new(y).range(0.0..=1.0).speed(0.01));
                        if response.changed() {
                            changed = true;
                        }
                        // Right-click instantly resets to 0.25
                        if response.clicked_by(PointerButton::Secondary) {
                            *y = 0.25;
                            changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("W:");
                        let response = ui.add(egui::DragValue::new(width).range(0.0..=1.0).speed(0.01));
                        if response.changed() {
                            changed = true;
                        }
                        // Right-click instantly resets to 0.5
                        if response.clicked_by(PointerButton::Secondary) {
                            *width = 0.5;
                            changed = true;
                        }
                        ui.label("H:");
                        let response = ui.add(egui::DragValue::new(height).range(0.0..=1.0).speed(0.01));
                        if response.changed() {
                            changed = true;
                        }
                        // Right-click instantly resets to 0.5
                        if response.clicked_by(PointerButton::Secondary) {
                            *height = 0.5;
                            changed = true;
                        }
                    });
                }
                MaskShape::Ellipse { center, radius_x, radius_y } => {
                    ui.label(egui::RichText::new("Ellipse").weak());
                    ui.horizontal(|ui| {
                        ui.label("Center X:");
                        let response = ui.add(egui::DragValue::new(&mut center.x).range(0.0..=1.0).speed(0.01));
                        if response.changed() {
                            changed = true;
                        }
                        // Right-click instantly resets to 0.5
                        if response.clicked_by(PointerButton::Secondary) {
                            center.x = 0.5;
                            changed = true;
                        }
                        ui.label("Y:");
                        let response = ui.add(egui::DragValue::new(&mut center.y).range(0.0..=1.0).speed(0.01));
                        if response.changed() {
                            changed = true;
                        }
                        // Right-click instantly resets to 0.5
                        if response.clicked_by(PointerButton::Secondary) {
                            center.y = 0.5;
                            changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Radius X:");
                        let response = ui.add(egui::DragValue::new(radius_x).range(0.0..=1.0).speed(0.01));
                        if response.changed() {
                            changed = true;
                        }
                        // Right-click instantly resets to 0.25
                        if response.clicked_by(PointerButton::Secondary) {
                            *radius_x = 0.25;
                            changed = true;
                        }
                        ui.label("Y:");
                        let response = ui.add(egui::DragValue::new(radius_y).range(0.0..=1.0).speed(0.01));
                        if response.changed() {
                            changed = true;
                        }
                        // Right-click instantly resets to 0.25
                        if response.clicked_by(PointerButton::Secondary) {
                            *radius_y = 0.25;
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
            let response = ui.add(egui::Slider::new(&mut slice_copy.color.opacity, 0.0..=1.0).max_decimals(2));
            if response.changed() {
                changed = true;
            }
            // Right-click instantly resets to 1
            if response.clicked_by(PointerButton::Secondary) {
                slice_copy.color.opacity = 1.0;
                changed = true;
            }
        });

        // Brightness slider
        ui.horizontal(|ui| {
            ui.label("Brightness:");
            let response = ui.add(egui::Slider::new(&mut slice_copy.color.brightness, -1.0..=1.0).max_decimals(2));
            if response.changed() {
                changed = true;
            }
            // Right-click instantly resets to 0
            if response.clicked_by(PointerButton::Secondary) {
                slice_copy.color.brightness = 0.0;
                changed = true;
            }
        });

        // Contrast slider
        ui.horizontal(|ui| {
            ui.label("Contrast:");
            let response = ui.add(egui::Slider::new(&mut slice_copy.color.contrast, 0.0..=2.0).max_decimals(2));
            if response.changed() {
                changed = true;
            }
            // Right-click instantly resets to 1
            if response.clicked_by(PointerButton::Secondary) {
                slice_copy.color.contrast = 1.0;
                changed = true;
            }
        });

        // Gamma slider
        ui.horizontal(|ui| {
            ui.label("Gamma:");
            let response = ui.add(egui::Slider::new(&mut slice_copy.color.gamma, 0.1..=4.0).logarithmic(true).max_decimals(2));
            if response.changed() {
                changed = true;
            }
            // Right-click instantly resets to 1
            if response.clicked_by(PointerButton::Secondary) {
                slice_copy.color.gamma = 1.0;
                changed = true;
            }
        });

        // RGB Channels (collapsing section for less common adjustments)
        ui.collapsing("RGB Channels", |ui| {
            ui.horizontal(|ui| {
                ui.label("Red:");
                let response = ui.add(egui::Slider::new(&mut slice_copy.color.red, 0.0..=2.0).max_decimals(2));
                if response.changed() {
                    changed = true;
                }
                // Right-click instantly resets to 1
                if response.clicked_by(PointerButton::Secondary) {
                    slice_copy.color.red = 1.0;
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Green:");
                let response = ui.add(egui::Slider::new(&mut slice_copy.color.green, 0.0..=2.0).max_decimals(2));
                if response.changed() {
                    changed = true;
                }
                // Right-click instantly resets to 1
                if response.clicked_by(PointerButton::Secondary) {
                    slice_copy.color.green = 1.0;
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Blue:");
                let response = ui.add(egui::Slider::new(&mut slice_copy.color.blue, 0.0..=2.0).max_decimals(2));
                if response.changed() {
                    changed = true;
                }
                // Right-click instantly resets to 1
                if response.clicked_by(PointerButton::Secondary) {
                    slice_copy.color.blue = 1.0;
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

    // =============================================================================
    // Viewport Pan/Zoom Support Methods
    // =============================================================================

    /// Transform a normalized coordinate (0-1) to screen space, accounting for viewport pan/zoom
    fn transform_point_to_screen(
        &self,
        viewport: &Viewport,
        normalized: (f32, f32),
        preview_rect: egui::Rect,
        preview_size: egui::Vec2,
        content_size: (f32, f32),
    ) -> egui::Pos2 {
        let (scale_x, scale_y, offset_x, offset_y) = viewport.get_shader_params(
            (preview_size.x, preview_size.y),
            content_size,
        );

        // Transform: apply zoom and offset
        // The viewport centers content, so we map normalized (0-1) to centered (-0.5 to 0.5)
        let centered_x = normalized.0 - 0.5;
        let centered_y = normalized.1 - 0.5;

        // Apply scale and offset
        let transformed_x = centered_x * scale_x + 0.5 + offset_x;
        let transformed_y = centered_y * scale_y + 0.5 + offset_y;

        // Convert to screen pixels
        preview_rect.min + egui::vec2(
            transformed_x * preview_size.x,
            transformed_y * preview_size.y,
        )
    }

    /// Transform a screen space coordinate back to normalized (0-1) coordinate
    fn transform_point_from_screen(
        &self,
        viewport: &Viewport,
        screen_pos: egui::Pos2,
        preview_rect: egui::Rect,
        preview_size: egui::Vec2,
        content_size: (f32, f32),
    ) -> (f32, f32) {
        let (scale_x, scale_y, offset_x, offset_y) = viewport.get_shader_params(
            (preview_size.x, preview_size.y),
            content_size,
        );

        // Convert from screen pixels to normalized viewport space (0-1)
        let viewport_x = (screen_pos.x - preview_rect.left()) / preview_size.x;
        let viewport_y = (screen_pos.y - preview_rect.top()) / preview_size.y;

        // Inverse transform: remove offset and zoom
        // viewport_x = centered_x * scale_x + 0.5 + offset_x
        // centered_x = (viewport_x - 0.5 - offset_x) / scale_x
        // normalized_x = centered_x + 0.5
        let centered_x = (viewport_x - 0.5 - offset_x) / scale_x;
        let centered_y = (viewport_y - 0.5 - offset_y) / scale_y;

        let norm_x = centered_x + 0.5;
        let norm_y = centered_y + 0.5;

        (norm_x, norm_y)
    }

    /// Transform a normalized rect to screen space rect
    fn transform_rect_to_screen(
        &self,
        viewport: &Viewport,
        input_rect: &Rect,
        preview_rect: egui::Rect,
        preview_size: egui::Vec2,
        content_size: (f32, f32),
    ) -> egui::Rect {
        let top_left = self.transform_point_to_screen(
            viewport,
            (input_rect.x, input_rect.y),
            preview_rect,
            preview_size,
            content_size,
        );
        let bottom_right = self.transform_point_to_screen(
            viewport,
            (input_rect.x + input_rect.width, input_rect.y + input_rect.height),
            preview_rect,
            preview_size,
            content_size,
        );
        egui::Rect::from_min_max(top_left, bottom_right)
    }

    /// Update viewport animations (call each frame when window is open)
    pub fn update_viewports(&mut self, env_content_size: (f32, f32), output_content_size: Option<(f32, f32)>) {
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.last_viewport_update).as_secs_f32();
        self.last_viewport_update = now;

        // Environment viewport
        if self.env_viewport.needs_update() {
            // Use a reasonable default preview size for animation calculations
            let preview_size = (400.0, 225.0);
            self.env_viewport.update(dt, preview_size, env_content_size);
        }

        // Output viewport
        if self.output_viewport.needs_update() {
            let content = output_content_size.unwrap_or((1920.0, 1080.0));
            let preview_size = (400.0, 300.0);
            self.output_viewport.update(dt, preview_size, content);
        }
    }

    /// Check if any viewport needs animation update
    pub fn viewports_need_update(&self) -> bool {
        self.env_viewport.needs_update() || self.output_viewport.needs_update()
    }
}
