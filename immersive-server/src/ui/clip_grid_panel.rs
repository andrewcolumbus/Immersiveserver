//! Clip Grid Panel
//!
//! Displays a unified clip grid where:
//! - Rows = layers
//! - Columns = clip slots per layer
//!
//! This is the primary interface for triggering clips in a VJ-style workflow.

use crate::compositor::{ClipCell, Layer};
use std::path::PathBuf;

/// Size of clip grid cells in pixels
const CELL_SIZE: f32 = 50.0;
/// Spacing between cells
const CELL_SPACING: f32 = 3.0;
/// Width of the layer name column
const LAYER_NAME_WIDTH: f32 = 80.0;
/// Width of the opacity slider column
const OPACITY_SLIDER_WIDTH: f32 = 50.0;
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
                actions = self.render_contents(ui, layers);
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
    ) -> Vec<ClipGridAction> {
        let mut actions = Vec::new();

        // Check for drag-drop events
        let is_dragging = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
        let dropped_files: Vec<PathBuf> = ui.ctx().input(|i| {
            i.raw.dropped_files.iter()
                .filter_map(|f| f.path.clone())
                .filter(|p| Self::is_video_file(p))
                .collect()
        });

        // Handle dropped files
        if let Some(path) = dropped_files.first() {
            if let Some((layer_id, slot)) = self.drag_hover_cell.take() {
                actions.push(ClipGridAction::AssignClipWithPath {
                    layer_id,
                    slot,
                    path: path.clone(),
                });
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
            // Empty space for layer name column
            ui.allocate_space(egui::vec2(LAYER_NAME_WIDTH, 20.0));
            // Empty space for opacity slider column
            ui.allocate_space(egui::vec2(OPACITY_SLIDER_WIDTH + CELL_SPACING, 20.0));
            
            for slot in 0..max_clips {
                let label = format!("{}", slot + 1);
                let response = ui.allocate_ui(egui::vec2(CELL_SIZE + CELL_SPACING, 20.0), |ui| {
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
                    let row_actions = self.render_layer_row(ui, layer, max_clips, is_dragging);
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
    ) -> Vec<ClipGridAction> {
        let mut actions = Vec::new();

        ui.horizontal(|ui| {
            // Layer name (clickable to select layer, right-click for context menu)
            let has_active = layer.has_active_clip();
            let layer_id = layer.id;
            
            let label_response = ui.allocate_ui(egui::vec2(LAYER_NAME_WIDTH - 20.0, CELL_SIZE), |ui| {
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
            
            // Stop button (X) - only show when layer has active clip
            if has_active {
                if ui.add(egui::Button::new(
                    egui::RichText::new("✕").size(10.0).color(egui::Color32::from_rgb(255, 100, 100))
                ).min_size(egui::vec2(18.0, 18.0))).on_hover_text("Stop clip").clicked() {
                    actions.push(ClipGridAction::StopClip { layer_id });
                }
            } else {
                // Add spacer to keep alignment consistent
                ui.allocate_space(egui::vec2(18.0, 18.0));
            }

            // Opacity slider
            let mut opacity = layer.opacity;
            let slider_response = ui.allocate_ui(egui::vec2(OPACITY_SLIDER_WIDTH, CELL_SIZE), |ui| {
                ui.vertical_centered(|ui| {
                    // Show percentage label
                    let opacity_pct = (opacity * 100.0).round() as u32;
                    ui.label(
                        egui::RichText::new(format!("{}%", opacity_pct))
                            .size(9.0)
                            .color(egui::Color32::GRAY)
                    );
                    
                    // Compact slider (no labels, just the slider control)
                    let slider = egui::Slider::new(&mut opacity, 0.0..=1.0)
                        .show_value(false)
                        .clamping(egui::SliderClamping::Always);
                    ui.add(slider)
                }).inner
            });
            
            // Emit action if opacity changed
            if slider_response.inner.changed() {
                actions.push(ClipGridAction::SetLayerOpacity { layer_id, opacity });
            }
            
            // Tooltip for the slider
            slider_response.response.on_hover_text("Layer opacity (0% = invisible, 100% = opaque)");

            // Clip cells
            let layer_has_active_clip = layer.has_active_clip();
            for slot in 0..max_clips {
                let cell = layer.get_clip(slot);
                let is_active = layer.active_clip == Some(slot);
                let cell_actions = self.render_cell(ui, layer.id, slot, cell, is_active, layer_has_active_clip, is_dragging);
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
    ) -> Vec<ClipGridAction> {
        let mut actions = Vec::new();

        let size = egui::vec2(CELL_SIZE, CELL_SIZE);

        // Check if this cell is being hovered during drag
        let is_drag_hover = self.drag_hover_cell == Some((layer_id, slot));

        // Determine cell appearance
        let (bg_color, text_color, label) = if is_drag_hover {
            // Highlight during drag hover
            let bg = egui::Color32::from_rgb(80, 120, 200); // Blue highlight
            let text = egui::Color32::WHITE;
            let label = "⬇".to_string(); // Drop indicator
            (bg, text, label)
        } else if let Some(clip) = cell {
            let bg = if is_active {
                egui::Color32::from_rgb(40, 160, 80) // Green for active
            } else {
                egui::Color32::from_rgb(55, 55, 70) // Dark blue-gray for clips
            };
            let text = egui::Color32::WHITE;
            // Truncate label to fit cell
            let name = clip.display_name();
            let label = if name.len() > 8 {
                format!("{}…", &name[..7])
            } else {
                name
            };
            (bg, text, label)
        } else {
            // Empty cell
            let bg = if is_dragging {
                egui::Color32::from_rgb(50, 50, 60) // Slightly highlighted during drag
            } else {
                egui::Color32::from_rgb(35, 35, 40)
            };
            let text = egui::Color32::from_rgb(80, 80, 80);
            let label = String::new(); // Empty cells have no text
            (bg, text, label)
        };

        // Create the button
        let button = egui::Button::new(
            egui::RichText::new(&label)
                .color(text_color)
                .size(9.0)
        )
        .min_size(size)
        .fill(bg_color)
        .corner_radius(3.0);

        let response = ui.add(button);

        // Track drag hover state
        if is_dragging && response.hovered() {
            self.drag_hover_cell = Some((layer_id, slot));
        }

        // Handle left-click: trigger clip, stop layer, or open file picker
        if response.clicked() {
            if cell.is_some() {
                actions.push(ClipGridAction::TriggerClip { layer_id, slot });
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
                    if ui.button("📋 Paste").clicked() {
                        actions.push(ClipGridAction::PasteClip { layer_id, slot });
                        ui.close_menu();
                    }
                }
            }
        });

        // Tooltip with full name and path
        if let Some(clip) = cell {
            let active_text = if is_active { " (playing)" } else { "" };
            response.on_hover_text(format!(
                "{}{}\n{}",
                clip.display_name(),
                active_text,
                clip.source_path.display()
            ));
        } else {
            let tooltip = if layer_has_active_clip {
                "Click to stop layer"
            } else {
                "Click to assign a video clip"
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

    /// Check if a file path is a supported video file
    fn is_video_file(path: &std::path::Path) -> bool {
        let video_extensions = ["mp4", "mov", "avi", "mkv", "webm", "m4v", "wmv", "flv"];
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| video_extensions.contains(&ext.to_lowercase().as_str()))
            .unwrap_or(false)
    }
}
