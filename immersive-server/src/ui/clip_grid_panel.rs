//! Clip Grid Panel
//!
//! Displays a unified clip grid where:
//! - Rows = layers
//! - Columns = clip slots per layer
//!
//! This is the primary interface for triggering clips in a VJ-style workflow.

use crate::compositor::{ClipCell, ClipSource, Layer};
use crate::ui::ThumbnailCache;
use std::path::PathBuf;

/// Size of clip grid cells in pixels
const CELL_SIZE: f32 = 80.0;
/// Spacing between cells
const CELL_SPACING: f32 = 3.0;
/// Width of the layer name column
const LAYER_NAME_WIDTH: f32 = 80.0;
/// Width of the opacity slider column
const OPACITY_SLIDER_WIDTH: f32 = 40.0;
/// Padding inside the panel
const PANEL_PADDING: f32 = 8.0;

/// Actions that can be returned from the clip grid panel
#[derive(Debug, Clone)]
pub enum ClipGridAction {
    /// User clicked a cell to trigger a clip
    TriggerClip {
        layer_id: u32,
        slot: usize,
    },
    /// User wants to assign a clip to a cell (via context menu or drag-drop)
    AssignClip {
        layer_id: u32,
        slot: usize,
    },
    /// User wants to assign a clip with a specific path (from drag-drop)
    AssignClipWithPath {
        layer_id: u32,
        slot: usize,
        path: PathBuf,
    },
    /// User wants to assign an OMT source to a cell
    AssignOmtSource {
        layer_id: u32,
        slot: usize,
        address: String,
        name: String,
    },
    /// User wants to assign an NDI source to a cell
    AssignNdiSource {
        layer_id: u32,
        slot: usize,
        ndi_name: String,
        url_address: Option<String>,
    },
    /// User wants to clear a clip from a cell
    ClearClip {
        layer_id: u32,
        slot: usize,
    },
    /// User wants to stop the current clip on a layer
    StopClip {
        layer_id: u32,
    },
    /// User wants to set the transition mode for a layer
    SetLayerTransition {
        layer_id: u32,
        transition: crate::compositor::ClipTransition,
    },
    /// Add a new layer
    AddLayer,
    /// Delete a layer
    DeleteLayer {
        layer_id: u32,
    },
    /// Add a new column (clip slot) to all layers
    AddColumn,
    /// Delete a column from all layers
    DeleteColumn {
        column_index: usize,
    },
    /// User changed a layer's opacity
    SetLayerOpacity {
        layer_id: u32,
        opacity: f32,
    },
    /// Copy a clip to the clipboard
    CopyClip {
        layer_id: u32,
        slot: usize,
    },
    /// Paste clipboard clip to a slot
    PasteClip {
        layer_id: u32,
        slot: usize,
    },
    /// Clone (duplicate) an entire layer
    CloneLayer {
        layer_id: u32,
    },
    /// Select a layer (show in properties panel)
    SelectLayer {
        layer_id: u32,
    },
    /// Select a clip for preview (without triggering it live)
    SelectClipForPreview {
        layer_id: u32,
        slot: usize,
    },
}

/// State for the clip grid panel
#[derive(Default)]
pub struct ClipGridPanel {
    /// Whether the panel is open
    pub open: bool,
    /// Pending file picker for clip assignment
    pub pending_clip_assignment: Option<(u32, usize)>, // (layer_id, slot)
    /// Cell currently being hovered during drag-drop (layer_id, slot)
    drag_hover_cell: Option<(u32, usize)>,
    /// Clipboard for copy/paste operations
    clipboard: Option<ClipCell>,
}

impl ClipGridPanel {
    /// Create a new clip grid panel
    pub fn new() -> Self {
        Self {
            open: true, // Start open by default
            pending_clip_assignment: None,
            drag_hover_cell: None,
            clipboard: None,
        }
    }

    /// Copy a clip to the clipboard
    pub fn copy_clip(&mut self, clip: ClipCell) {
        self.clipboard = Some(clip);
    }

    /// Check if clipboard has a clip
    pub fn has_clipboard(&self) -> bool {
        self.clipboard.is_some()
    }

    /// Get the clipboard clip (for pasting)
    pub fn get_clipboard(&self) -> Option<&ClipCell> {
        self.clipboard.as_ref()
    }

    /// Take the clipboard clip (consumes it)
    pub fn take_clipboard(&mut self) -> Option<ClipCell> {
        self.clipboard.take()
    }

    /// Render the clip grid panel in its own side panel (default behavior).
    ///
    /// Returns a list of actions to be processed by the app.
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        layers: &[Layer],
        thumbnail_cache: &mut ThumbnailCache,
    ) -> Vec<ClipGridAction> {
        if !self.open {
            return Vec::new();
        }

        // Calculate the number of clip columns (use max from all layers)
        let max_clips = layers.iter().map(|l| l.clip_count()).max().unwrap_or(8);
        let grid_width = LAYER_NAME_WIDTH + OPACITY_SLIDER_WIDTH + CELL_SPACING + max_clips as f32 * (CELL_SIZE + CELL_SPACING) + PANEL_PADDING * 2.0;

        let mut actions = Vec::new();
        egui::SidePanel::right("clip_grid_panel")
            .default_width(grid_width.min(500.0))
            .min_width(300.0)
            .resizable(true)
            .show(ctx, |ui| {
                // Header with controls
                ui.horizontal(|ui| {
                    ui.heading("Clip Grid");
                });
                ui.separator();
                actions = self.render_contents(ui, layers, thumbnail_cache);
            });

        actions
    }

    /// Render the clip grid contents (used by dock system or embedded in a window).
    ///
    /// Returns a list of actions to be processed by the app.
    pub fn render_contents(
        &mut self,
        ui: &mut egui::Ui,
        layers: &[Layer],
        thumbnail_cache: &mut ThumbnailCache,
    ) -> Vec<ClipGridAction> {
        let mut actions = Vec::new();

        // Check for source drag-drop events (from Sources panel or File Browser panel)
        let is_dragging = crate::ui::sources_panel::is_source_being_dragged(ui.ctx());

        // Handle dropped sources (from Sources panel or File Browser panel)
        if let Some(dropped_source) = crate::ui::sources_panel::get_dropped_source(ui.ctx()) {
            if let Some((layer_id, slot)) = self.drag_hover_cell.take() {
                match dropped_source {
                    crate::ui::DraggableSource::Omt { address, name, .. } => {
                        actions.push(ClipGridAction::AssignOmtSource {
                            layer_id,
                            slot,
                            address,
                            name,
                        });
                    }
                    crate::ui::DraggableSource::File { path, .. } => {
                        actions.push(ClipGridAction::AssignClipWithPath {
                            layer_id,
                            slot,
                            path,
                        });
                    }
                    crate::ui::DraggableSource::Ndi { ndi_name, url_address, .. } => {
                        actions.push(ClipGridAction::AssignNdiSource {
                            layer_id,
                            slot,
                            ndi_name,
                            url_address,
                        });
                    }
                }
            }
        }

        // Clear drag hover if not dragging
        if !is_dragging {
            self.drag_hover_cell = None;
        }

        // Calculate the number of clip columns (use max from all layers)
        let max_clips = layers.iter().map(|l| l.clip_count()).max().unwrap_or(8);

        // Header with controls
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Add column button
                if ui.small_button("+ Col").on_hover_text("Add a new column").clicked() {
                    actions.push(ClipGridAction::AddColumn);
                }
                // Add layer button
                if ui.small_button("+ Layer").on_hover_text("Add a new layer").clicked() {
                    actions.push(ClipGridAction::AddLayer);
                }
                ui.separator();
                if ui.small_button("⏹ Stop All").clicked() {
                    for layer in layers {
                        if layer.has_active_clip() {
                            actions.push(ClipGridAction::StopClip { layer_id: layer.id });
                        }
                    }
                }
            });
        });
        ui.separator();

        if layers.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label("No layers yet.");
                ui.add_space(10.0);
                if ui.button("+ Add Layer").clicked() {
                    actions.push(ClipGridAction::AddLayer);
                }
            });
            return actions;
        }

        // Column headers (clip slot numbers) - right-click to delete
        ui.horizontal(|ui| {
            // Use consistent spacing with grid rows
            ui.spacing_mut().item_spacing.x = CELL_SPACING;

            // Space for stop button (matches row layout)
            ui.allocate_space(egui::vec2(28.0, 20.0));
            // Space for layer name (matches LAYER_NAME_WIDTH - 28.0 in rows)
            ui.allocate_space(egui::vec2(LAYER_NAME_WIDTH - 28.0, 20.0));
            // Space for opacity slider column
            ui.allocate_space(egui::vec2(OPACITY_SLIDER_WIDTH, 20.0));

            for slot in 0..max_clips {
                let label = format!("{}", slot + 1);
                let response = ui.allocate_ui(egui::vec2(CELL_SIZE, 20.0), |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.label(egui::RichText::new(label).size(11.0).color(egui::Color32::GRAY))
                    }).inner
                }).inner;

                // Right-click context menu on column header
                response.context_menu(|ui| {
                    if ui.button("🗑 Delete Column").clicked() {
                        actions.push(ClipGridAction::DeleteColumn { column_index: slot });
                        ui.close_menu();
                    }
                });
            }
        });

        ui.add_space(2.0);

        // Grid content
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Iterate in REVERSE order so top layer (rendered last) appears at top of UI
                for layer in layers.iter().rev() {
                    let row_actions = self.render_layer_row(ui, layer, max_clips, is_dragging, thumbnail_cache);
                    actions.extend(row_actions);
                }
            });

        actions
    }

    /// Render a single layer row in the grid
    fn render_layer_row(
        &mut self,
        ui: &mut egui::Ui,
        layer: &Layer,
        max_clips: usize,
        is_dragging: bool,
        thumbnail_cache: &mut ThumbnailCache,
    ) -> Vec<ClipGridAction> {
        let mut actions = Vec::new();
        let has_active = layer.has_active_clip();
        let layer_id = layer.id;

        ui.horizontal(|ui| {
            // Use consistent spacing with header row
            ui.spacing_mut().item_spacing.x = CELL_SPACING;

            // Stop button (X) on the LEFT - always visible, vertically centered
            let (button_fill, button_text_color) = if has_active {
                (egui::Color32::from_rgb(180, 60, 60), egui::Color32::WHITE)
            } else {
                (egui::Color32::from_rgb(60, 60, 65), egui::Color32::from_rgb(100, 100, 100))
            };
            
            // Wrap in a container that matches the row height for vertical centering
            ui.allocate_ui(egui::vec2(28.0, CELL_SIZE), |ui| {
                ui.centered_and_justified(|ui| {
                    let stop_button = egui::Button::new(
                        egui::RichText::new("X")
                            .size(12.0)
                            .strong()
                            .color(button_text_color)
                    )
                    .min_size(egui::vec2(24.0, 24.0))
                    .fill(button_fill)
                    .corner_radius(4.0);
                    
                    if ui.add(stop_button).on_hover_text("Stop clip").clicked() && has_active {
                        actions.push(ClipGridAction::StopClip { layer_id });
                    }
                });
            });
            
            // Layer name (clickable to select layer, right-click for context menu)
            let label_response = ui.allocate_ui(egui::vec2(LAYER_NAME_WIDTH - 28.0, CELL_SIZE), |ui| {
                let name_color = if has_active {
                    egui::Color32::from_rgb(100, 200, 100) // Green when playing
                } else {
                    egui::Color32::WHITE
                };

                ui.vertical_centered(|ui| {
                    ui.add_space((CELL_SIZE - 20.0) / 2.0);
                    let label = egui::RichText::new(&layer.name)
                        .size(11.0)
                        .color(name_color);
                    let response = ui.selectable_label(false, label);
                    
                    // Handle left-click to select layer (show in properties panel)
                    if response.clicked() {
                        actions.push(ClipGridAction::SelectLayer { layer_id });
                    }
                    
                    // Right-click context menu directly on the label
                    let current_transition = layer.transition;
                    response.context_menu(|ui| {
                        if has_active {
                            if ui.button("⏹ Stop Clip").clicked() {
                                actions.push(ClipGridAction::StopClip { layer_id });
                                ui.close_menu();
                            }
                            ui.separator();
                        }
                        
                        if ui.button("📋 Clone Layer").clicked() {
                            actions.push(ClipGridAction::CloneLayer { layer_id });
                            ui.close_menu();
                        }
                        
                        ui.separator();
                        
                        // Transition submenu
                        ui.menu_button(format!("Transition: {}", current_transition.name()), |ui| {
                            use crate::compositor::ClipTransition;
                            
                            let is_cut = matches!(current_transition, ClipTransition::Cut);
                            if ui.selectable_label(is_cut, "Cut (instant)").clicked() {
                                actions.push(ClipGridAction::SetLayerTransition {
                                    layer_id,
                                    transition: ClipTransition::Cut,
                                });
                                ui.close_menu();
                            }
                            
                            let is_fade = matches!(current_transition, ClipTransition::Fade(_));
                            if ui.selectable_label(is_fade, "Fade (0.5s)").clicked() {
                                actions.push(ClipGridAction::SetLayerTransition {
                                    layer_id,
                                    transition: ClipTransition::fade(),
                                });
                                ui.close_menu();
                            }
                        });
                        
                        ui.separator();
                        if ui.button("🗑 Delete Layer").clicked() {
                            actions.push(ClipGridAction::DeleteLayer { layer_id });
                            ui.close_menu();
                        }
                    });
                    
                    response
                }).inner
            });

            // Show tooltip on layer name area
            label_response.response.on_hover_text("Click to select • Right-click for options");

            // Opacity slider (vertical fader)
            let mut opacity = layer.opacity;
            let slider_result = ui.allocate_ui(egui::vec2(OPACITY_SLIDER_WIDTH, CELL_SIZE), |ui| {
                // Remove internal spacing so slider fills full height
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                let slider = egui::Slider::new(&mut opacity, 0.0..=1.0)
                    .vertical()
                    .show_value(false)
                    .clamping(egui::SliderClamping::Always);
                ui.add_sized([OPACITY_SLIDER_WIDTH, CELL_SIZE], slider)
            });

            // Emit action if opacity changed
            if slider_result.inner.changed() {
                actions.push(ClipGridAction::SetLayerOpacity { layer_id, opacity });
            }

            // Tooltip for the slider
            slider_result.inner.on_hover_text("Layer opacity (0% = invisible, 100% = opaque)");

            // Clip cells
            let layer_has_active_clip = layer.has_active_clip();
            for slot in 0..max_clips {
                let cell = layer.get_clip(slot);
                let is_active = layer.active_clip == Some(slot);
                let cell_actions = self.render_cell(ui, layer.id, slot, cell, is_active, layer_has_active_clip, is_dragging, thumbnail_cache);
                actions.extend(cell_actions);
            }
        });

        ui.add_space(CELL_SPACING);

        actions
    }

    /// Render a single cell in the grid
    fn render_cell(
        &mut self,
        ui: &mut egui::Ui,
        layer_id: u32,
        slot: usize,
        cell: Option<&ClipCell>,
        is_active: bool,
        layer_has_active_clip: bool,
        is_dragging: bool,
        thumbnail_cache: &mut ThumbnailCache,
    ) -> Vec<ClipGridAction> {
        let mut actions = Vec::new();

        let size = egui::vec2(CELL_SIZE, CELL_SIZE);

        // Check if this cell is being hovered during drag (for drop indicator)
        let is_drag_hover = self.drag_hover_cell == Some((layer_id, slot));

        // Determine cell appearance and get thumbnail if available
        let (bg_color, text_color, label, thumbnail) = if is_drag_hover {
            // Highlight during drag hover
            let bg = egui::Color32::from_rgb(80, 120, 200); // Blue highlight
            let text = egui::Color32::WHITE;
            let label = "⬇".to_string(); // Drop indicator
            (bg, text, label, None)
        } else if let Some(clip) = cell {
            let bg = if is_active {
                egui::Color32::from_rgb(40, 160, 80) // Green for active
            } else {
                egui::Color32::from_rgb(55, 55, 70) // Dark blue-gray for clips
            };
            let text = egui::Color32::WHITE;
            // Truncate label to fit cell (use chars() for unicode safety)
            let name = clip.display_name();
            let char_count = name.chars().count();
            let label = if char_count > 6 {
                let truncated: String = name.chars().take(5).collect();
                format!("{}…", truncated)
            } else {
                name
            };

            // Try to get thumbnail for file sources (returns id and size for proper centering)
            let thumbnail = if let ClipSource::File { path } = &clip.source {
                let mode = thumbnail_cache.mode();
                // Include mode in cache key so switching modes regenerates thumbnails
                let cache_key = format!("{}:{:?}", path.to_string_lossy(), mode);
                if let Some(tex) = thumbnail_cache.get(&cache_key) {
                    let size = tex.size();
                    Some((tex.id(), egui::vec2(size[0] as f32, size[1] as f32)))
                } else {
                    // Request thumbnail generation with current mode
                    thumbnail_cache.request(cache_key, path.clone(), mode);
                    None
                }
            } else {
                None // No thumbnails for OMT sources
            };

            (bg, text, label, thumbnail)
        } else {
            // Empty cell
            let bg = if is_dragging {
                egui::Color32::from_rgb(50, 50, 60) // Slightly highlighted during drag
            } else {
                egui::Color32::from_rgb(35, 35, 40)
            };
            let text = egui::Color32::from_rgb(80, 80, 80);
            let label = String::new(); // Empty cells have no text
            (bg, text, label, None)
        };

        // Create the cell with fixed size
        let response = ui.allocate_ui_with_layout(
            size,
            egui::Layout::centered_and_justified(egui::Direction::TopDown),
            |ui| {
                // Draw background
                let rect = ui.available_rect_before_wrap();
                ui.painter().rect_filled(rect, 3.0, bg_color);

                // Draw thumbnail if available
                if let Some((texture_id, tex_size)) = thumbnail {
                    let padding = 2.0;
                    let available = rect.shrink(padding);

                    // Calculate centered rect that maintains aspect ratio
                    let tex_aspect = tex_size.x / tex_size.y;
                    let cell_aspect = available.width() / available.height();

                    let image_rect = if (tex_aspect - cell_aspect).abs() < 0.01 {
                        // Aspect ratios match, fill the cell
                        available
                    } else if tex_aspect > cell_aspect {
                        // Thumbnail is wider - fit width, center vertically
                        let height = available.width() / tex_aspect;
                        let y_offset = (available.height() - height) / 2.0;
                        egui::Rect::from_min_size(
                            egui::pos2(available.left(), available.top() + y_offset),
                            egui::vec2(available.width(), height),
                        )
                    } else {
                        // Thumbnail is taller - fit height, center horizontally
                        let width = available.height() * tex_aspect;
                        let x_offset = (available.width() - width) / 2.0;
                        egui::Rect::from_min_size(
                            egui::pos2(available.left() + x_offset, available.top()),
                            egui::vec2(width, available.height()),
                        )
                    };

                    ui.painter().image(
                        texture_id,
                        image_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );

                    // Draw active indicator border
                    if is_active {
                        ui.painter().rect_stroke(
                            rect,
                            3.0,
                            egui::Stroke::new(3.0, egui::Color32::from_rgb(40, 160, 80)),
                            egui::StrokeKind::Outside,
                        );
                    }
                }

                // Draw label at bottom
                if !label.is_empty() {
                    let label_pos = egui::pos2(
                        rect.center().x,
                        rect.bottom() - 10.0,
                    );
                    ui.painter().text(
                        label_pos,
                        egui::Align2::CENTER_CENTER,
                        &label,
                        egui::FontId::proportional(9.0),
                        text_color,
                    );
                }

                // Return sense response for the whole area
                ui.allocate_rect(rect, egui::Sense::click())
            }
        ).inner;

        // Track drag hover state for visual feedback during drags
        if is_dragging {
            if let Some(pos) = ui.ctx().pointer_latest_pos() {
                if response.rect.contains(pos) {
                    self.drag_hover_cell = Some((layer_id, slot));
                }
            }
        }

        // Handle left-click: trigger clip, stop layer, or open file picker
        // The bottom 18px is the "label area" - clicking there selects for preview only
        if response.clicked() {
            if cell.is_some() {
                // Check if click was in the label area (bottom 18px of cell)
                let label_area_top = response.rect.bottom() - 18.0;
                let click_in_label = response
                    .interact_pointer_pos()
                    .map(|pos| pos.y >= label_area_top)
                    .unwrap_or(false);

                if click_in_label {
                    // Label click: select for preview only (don't trigger)
                    actions.push(ClipGridAction::SelectClipForPreview { layer_id, slot });
                } else {
                    // Main cell click: trigger the clip
                    actions.push(ClipGridAction::TriggerClip { layer_id, slot });
                }
            } else if layer_has_active_clip {
                // Empty cell clicked while layer is playing - stop the layer
                actions.push(ClipGridAction::StopClip { layer_id });
            } else {
                // Empty cell - open file picker to assign
                actions.push(ClipGridAction::AssignClip { layer_id, slot });
            }
        }

        // Handle right-click: context menu
        let has_clipboard = self.clipboard.is_some();
        response.context_menu(|ui| {
            if cell.is_some() {
                if ui.button("▶ Play").clicked() {
                    actions.push(ClipGridAction::TriggerClip { layer_id, slot });
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("📋 Copy").clicked() {
                    actions.push(ClipGridAction::CopyClip { layer_id, slot });
                    ui.close_menu();
                }
                if has_clipboard {
                    if ui.button("📋 Paste").clicked() {
                        actions.push(ClipGridAction::PasteClip { layer_id, slot });
                        ui.close_menu();
                    }
                }
                ui.separator();
                if ui.button("📁 Replace...").clicked() {
                    actions.push(ClipGridAction::AssignClip { layer_id, slot });
                    ui.close_menu();
                }
                if ui.button("🗑 Clear").clicked() {
                    actions.push(ClipGridAction::ClearClip { layer_id, slot });
                    ui.close_menu();
                }
            } else {
                if ui.button("📁 Assign Video...").clicked() {
                    actions.push(ClipGridAction::AssignClip { layer_id, slot });
                    ui.close_menu();
                }
                if has_clipboard {
                    ui.separator();
                    if ui.button("📋 Paste").clicked() {
                        actions.push(ClipGridAction::PasteClip { layer_id, slot });
                        ui.close_menu();
                    }
                }
            }
        });

        // Tooltip with full name and source info
        if let Some(clip) = cell {
            let active_text = if is_active { " (playing)" } else { "" };
            let source_info = match &clip.source {
                crate::compositor::ClipSource::File { path } => format!("📁 {}", path.display()),
                crate::compositor::ClipSource::Omt { address, .. } => format!("📡 OMT: {}", address),
                crate::compositor::ClipSource::Ndi { ndi_name, .. } => format!("📺 NDI: {}", ndi_name),
            };
            response.on_hover_text(format!(
                "{}{}\n{}",
                clip.display_name(),
                active_text,
                source_info
            ));
        } else {
            let tooltip = if layer_has_active_clip {
                "Click to stop layer"
            } else {
                "Click to assign a clip\nDrag from Files or Sources panel"
            };
            response.on_hover_text(tooltip);
        }

        actions
    }

    /// Set pending clip assignment (after file picker is opened)
    pub fn set_pending_assignment(&mut self, layer_id: u32, slot: usize) {
        self.pending_clip_assignment = Some((layer_id, slot));
    }

    /// Take the pending clip assignment, if any
    pub fn take_pending_assignment(&mut self) -> Option<(u32, usize)> {
        self.pending_clip_assignment.take()
    }

    /// Complete a pending clip assignment with a path
    pub fn complete_assignment(&mut self, path: PathBuf) -> Option<(u32, usize, PathBuf)> {
        self.pending_clip_assignment.take().map(|(layer_id, slot)| {
            (layer_id, slot, path)
        })
    }
}
