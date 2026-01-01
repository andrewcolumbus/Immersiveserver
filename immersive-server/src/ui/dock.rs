//! Docking system for detachable/resizable panels
//!
//! Provides a professional docking UI system where panels can be:
//! - Docked to edges (left, right, top, bottom)
//! - Floated as independent windows
//! - Dragged between dock zones
//! - Snapped to magnetic dock zones
//! - Snapped to other floating windows

use serde::{Deserialize, Serialize};

/// Distance threshold for window-to-window snapping (in pixels)
const WINDOW_SNAP_THRESHOLD: f32 = 15.0;

/// Dock zone where a panel can be placed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum DockZone {
    /// Docked to the left edge
    Left,
    /// Docked to the right edge
    #[default]
    Right,
    /// Docked to the top edge
    Top,
    /// Docked to the bottom edge
    Bottom,
    /// Floating as an independent window
    Floating,
}

impl DockZone {
    /// Check if this is a floating zone
    pub fn is_floating(&self) -> bool {
        matches!(self, DockZone::Floating)
    }

    /// Check if this is a horizontal dock (left/right)
    pub fn is_horizontal(&self) -> bool {
        matches!(self, DockZone::Left | DockZone::Right)
    }

    /// Check if this is a vertical dock (top/bottom)
    pub fn is_vertical(&self) -> bool {
        matches!(self, DockZone::Top | DockZone::Bottom)
    }
}

/// State for a dockable panel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockablePanel {
    /// Unique identifier for this panel
    pub id: String,
    /// Display title for the panel
    pub title: String,
    /// Current dock zone
    pub zone: DockZone,
    /// Position when floating (window coordinates)
    #[serde(default)]
    pub floating_pos: Option<(f32, f32)>,
    /// Size when floating
    #[serde(default)]
    pub floating_size: Option<(f32, f32)>,
    /// Whether the panel is open/visible
    pub open: bool,
    /// Default width when docked horizontally
    #[serde(default = "default_dock_width")]
    pub dock_width: f32,
    /// Default height when docked vertically
    #[serde(default = "default_dock_height")]
    pub dock_height: f32,
}

fn default_dock_width() -> f32 {
    300.0
}

fn default_dock_height() -> f32 {
    200.0
}

impl DockablePanel {
    /// Create a new dockable panel
    pub fn new(id: impl Into<String>, title: impl Into<String>, zone: DockZone) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            zone,
            floating_pos: None,
            floating_size: None,
            open: true,
            dock_width: default_dock_width(),
            dock_height: default_dock_height(),
        }
    }

    /// Check if this panel is floating
    pub fn is_floating(&self) -> bool {
        self.zone.is_floating()
    }

    /// Toggle between floating and a specific dock zone
    pub fn toggle_float(&mut self, default_dock: DockZone) {
        if self.is_floating() {
            self.zone = default_dock;
        } else {
            self.zone = DockZone::Floating;
        }
    }

    /// Dock to a specific zone
    pub fn dock_to(&mut self, zone: DockZone) {
        self.zone = zone;
    }

    /// Float at a specific position
    pub fn float_at(&mut self, pos: (f32, f32), size: (f32, f32)) {
        self.zone = DockZone::Floating;
        self.floating_pos = Some(pos);
        self.floating_size = Some(size);
    }
}

/// Magnetic dock zone indicator shown during drag operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DockZoneIndicator {
    pub zone: DockZone,
    pub highlighted: bool,
}

/// Rectangle representing a floating window's bounds
#[derive(Debug, Clone, Copy)]
pub struct WindowRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl WindowRect {
    /// Create from position and size
    pub fn from_pos_size(pos: (f32, f32), size: (f32, f32)) -> Self {
        Self {
            left: pos.0,
            top: pos.1,
            right: pos.0 + size.0,
            bottom: pos.1 + size.1,
        }
    }

    /// Get all edges as snap targets
    pub fn edges(&self) -> [f32; 4] {
        [self.left, self.top, self.right, self.bottom]
    }

    /// Calculate snapped position for another window being placed at `pos` with `size`
    /// Returns the adjusted position if snapping occurred
    pub fn snap_position(&self, pos: (f32, f32), size: (f32, f32), threshold: f32) -> (f32, f32) {
        let mut snapped_x = pos.0;
        let mut snapped_y = pos.1;

        let other_left = pos.0;
        let other_right = pos.0 + size.0;
        let other_top = pos.1;
        let other_bottom = pos.1 + size.1;

        // Snap X: other's left to this right
        if (other_left - self.right).abs() < threshold {
            snapped_x = self.right;
        }
        // Snap X: other's right to this left
        else if (other_right - self.left).abs() < threshold {
            snapped_x = self.left - size.0;
        }
        // Snap X: other's left to this left (align left edges)
        else if (other_left - self.left).abs() < threshold {
            snapped_x = self.left;
        }
        // Snap X: other's right to this right (align right edges)
        else if (other_right - self.right).abs() < threshold {
            snapped_x = self.right - size.0;
        }

        // Snap Y: other's top to this bottom
        if (other_top - self.bottom).abs() < threshold {
            snapped_y = self.bottom;
        }
        // Snap Y: other's bottom to this top
        else if (other_bottom - self.top).abs() < threshold {
            snapped_y = self.top - size.1;
        }
        // Snap Y: other's top to this top (align top edges)
        else if (other_top - self.top).abs() < threshold {
            snapped_y = self.top;
        }
        // Snap Y: other's bottom to this bottom (align bottom edges)
        else if (other_bottom - self.bottom).abs() < threshold {
            snapped_y = self.bottom - size.1;
        }

        (snapped_x, snapped_y)
    }
}

/// Manages all dockable panels and their states
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DockManager {
    /// All registered panels
    panels: Vec<DockablePanel>,
    /// Panel currently being dragged (by index)
    #[serde(skip)]
    dragging_panel: Option<usize>,
    /// Dock zone currently being hovered during drag
    #[serde(skip)]
    hover_zone: Option<DockZone>,
    /// Whether we're in a drag operation
    #[serde(skip)]
    is_dragging: bool,
    /// Drag start position
    #[serde(skip)]
    drag_start_pos: Option<(f32, f32)>,
    /// Pending snap position to apply on next frame (panel_id -> position)
    #[serde(skip)]
    pending_snap: Option<(String, (f32, f32))>,
}

impl DockManager {
    /// Create a new dock manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new panel with the dock manager
    pub fn register_panel(&mut self, panel: DockablePanel) {
        // Don't register duplicates
        if !self.panels.iter().any(|p| p.id == panel.id) {
            self.panels.push(panel);
        }
    }

    /// Get a panel by ID
    pub fn get_panel(&self, id: &str) -> Option<&DockablePanel> {
        self.panels.iter().find(|p| p.id == id)
    }

    /// Get a mutable panel by ID
    pub fn get_panel_mut(&mut self, id: &str) -> Option<&mut DockablePanel> {
        self.panels.iter_mut().find(|p| p.id == id)
    }

    /// Get all panels docked to a specific zone
    pub fn panels_in_zone(&self, zone: DockZone) -> Vec<&DockablePanel> {
        self.panels
            .iter()
            .filter(|p| p.zone == zone && p.open)
            .collect()
    }

    /// Get all floating panels
    pub fn floating_panels(&self) -> Vec<&DockablePanel> {
        self.panels_in_zone(DockZone::Floating)
    }

    /// Get rectangles of all floating panels except the one being dragged
    pub fn floating_window_rects(&self, exclude_panel_id: Option<&str>) -> Vec<WindowRect> {
        self.panels
            .iter()
            .filter(|p| {
                p.zone == DockZone::Floating
                    && p.open
                    && exclude_panel_id.map(|id| p.id != id).unwrap_or(true)
            })
            .filter_map(|p| {
                let pos = p.floating_pos?;
                let size = p.floating_size.unwrap_or((300.0, 400.0));
                Some(WindowRect::from_pos_size(pos, size))
            })
            .collect()
    }

    /// Calculate snapped position for a floating window
    /// Snaps to edges of other floating windows if within threshold
    pub fn calculate_snapped_position(
        &self,
        panel_id: &str,
        pos: (f32, f32),
        size: (f32, f32),
    ) -> (f32, f32) {
        let other_windows = self.floating_window_rects(Some(panel_id));
        
        let mut snapped_pos = pos;
        let mut best_snap_dist_x = WINDOW_SNAP_THRESHOLD;
        let mut best_snap_dist_y = WINDOW_SNAP_THRESHOLD;

        for window in &other_windows {
            let candidate = window.snap_position(pos, size, WINDOW_SNAP_THRESHOLD);
            
            // Check if this snap is closer than previous
            let dx = (candidate.0 - pos.0).abs();
            let dy = (candidate.1 - pos.1).abs();
            
            if dx > 0.0 && dx < best_snap_dist_x {
                snapped_pos.0 = candidate.0;
                best_snap_dist_x = dx;
            }
            if dy > 0.0 && dy < best_snap_dist_y {
                snapped_pos.1 = candidate.1;
                best_snap_dist_y = dy;
            }
        }

        snapped_pos
    }

    /// Check if any panel is docked to a zone
    pub fn has_panel_in_zone(&self, zone: DockZone) -> bool {
        self.panels.iter().any(|p| p.zone == zone && p.open)
    }

    /// Start dragging a panel
    pub fn start_drag(&mut self, panel_id: &str, pos: (f32, f32)) {
        if let Some(idx) = self.panels.iter().position(|p| p.id == panel_id) {
            self.dragging_panel = Some(idx);
            self.is_dragging = true;
            self.drag_start_pos = Some(pos);
        }
    }

    /// Update drag position and detect hover zones
    pub fn update_drag(&mut self, pos: (f32, f32), window_size: (f32, f32)) {
        if !self.is_dragging {
            return;
        }

        // Detect which dock zone we're hovering over
        // Note: Only Left/Right are supported for docking. Top/Bottom zones are
        // disabled because they don't have rendering implementations yet.
        let margin = 80.0; // Size of magnetic zone at edges

        self.hover_zone = if pos.0 < margin {
            Some(DockZone::Left)
        } else if pos.0 > window_size.0 - margin {
            Some(DockZone::Right)
        } else {
            None // Floating zone
        };
    }

    /// End drag operation - dock or float the panel
    /// Returns the snapped position if window-to-window snapping occurred
    pub fn end_drag(&mut self, pos: (f32, f32)) -> Option<(f32, f32)> {
        let hover_zone = self.hover_zone.take();
        let mut snapped_pos = None;
        
        if let Some(idx) = self.dragging_panel.take() {
            // Get panel info and calculate snap BEFORE mutating
            let (panel_id, panel_size) = {
                let panel = self.panels.get(idx);
                (
                    panel.map(|p| p.id.clone()),
                    panel.and_then(|p| p.floating_size).unwrap_or((300.0, 400.0)),
                )
            };

            // Calculate snapped position if we're floating (not docking to an edge)
            let final_pos = if hover_zone.is_none() {
                if let Some(ref id) = panel_id {
                    let calculated = self.calculate_snapped_position(id, pos, panel_size);
                    if calculated != pos {
                        snapped_pos = Some(calculated);
                    }
                    calculated
                } else {
                    pos
                }
            } else {
                pos
            };

            // Now apply the changes
            if let Some(panel) = self.panels.get_mut(idx) {
                if let Some(zone) = hover_zone {
                    // Dock to the hovered zone
                    panel.zone = zone;
                } else {
                    // Float at the snapped position
                    panel.float_at(final_pos, panel_size);
                }
            }
        }
        self.is_dragging = false;
        self.drag_start_pos = None;
        
        snapped_pos
    }

    /// End drag with the window's actual rect (from egui)
    /// This allows proper snapping based on the window's true position and size
    /// Stores any snap in pending_snap to be applied on next frame
    pub fn end_drag_with_rect(
        &mut self,
        window_pos: (f32, f32),
        window_size: (f32, f32),
    ) {
        let hover_zone = self.hover_zone.take();
        
        if let Some(idx) = self.dragging_panel.take() {
            // Get panel ID before mutating
            let panel_id = self.panels.get(idx).map(|p| p.id.clone());

            // Calculate snapped position if we're floating (not docking to an edge)
            let final_pos = if hover_zone.is_none() {
                if let Some(ref id) = panel_id {
                    let calculated = self.calculate_snapped_position(id, window_pos, window_size);
                    // Store pending snap if position changed significantly
                    if (calculated.0 - window_pos.0).abs() > 0.5 
                        || (calculated.1 - window_pos.1).abs() > 0.5 
                    {
                        self.pending_snap = Some((id.clone(), calculated));
                    }
                    calculated
                } else {
                    window_pos
                }
            } else {
                window_pos
            };

            // Now apply the changes
            if let Some(panel) = self.panels.get_mut(idx) {
                if let Some(zone) = hover_zone {
                    // Dock to the hovered zone
                    panel.zone = zone;
                } else {
                    // Float at the snapped position, update stored size too
                    panel.float_at(final_pos, window_size);
                }
            }
        }
        self.is_dragging = false;
        self.drag_start_pos = None;
    }

    /// Get and clear any pending snap for a panel
    /// Returns Some((x, y)) if this panel has a pending snap position
    pub fn take_pending_snap(&mut self, panel_id: &str) -> Option<(f32, f32)> {
        if let Some((ref id, pos)) = self.pending_snap {
            if id == panel_id {
                let result = Some(pos);
                self.pending_snap = None;
                return result;
            }
        }
        None
    }

    /// Check if there's a pending snap for a panel (without consuming it)
    pub fn has_pending_snap(&self, panel_id: &str) -> bool {
        self.pending_snap.as_ref().map(|(id, _)| id == panel_id).unwrap_or(false)
    }

    /// Cancel drag operation
    pub fn cancel_drag(&mut self) {
        self.dragging_panel = None;
        self.is_dragging = false;
        self.drag_start_pos = None;
        self.hover_zone = None;
    }

    /// Check if currently dragging
    pub fn is_dragging(&self) -> bool {
        self.is_dragging
    }

    /// Get the currently hovered dock zone
    pub fn hovered_zone(&self) -> Option<DockZone> {
        self.hover_zone
    }

    /// Toggle a panel's open state
    pub fn toggle_panel(&mut self, id: &str) {
        if let Some(panel) = self.get_panel_mut(id) {
            panel.open = !panel.open;
        }
    }

    /// Open a panel
    pub fn open_panel(&mut self, id: &str) {
        if let Some(panel) = self.get_panel_mut(id) {
            panel.open = true;
        }
    }

    /// Close a panel
    pub fn close_panel(&mut self, id: &str) {
        if let Some(panel) = self.get_panel_mut(id) {
            panel.open = false;
        }
    }

    /// Render dock zone indicators during drag operations
    pub fn render_dock_zones(&self, ctx: &egui::Context) {
        if !self.is_dragging {
            return;
        }

        let screen_rect = ctx.screen_rect();
        let margin = 80.0;
        let indicator_alpha = 0.3;
        let highlight_alpha = 0.6;

        // Helper to draw a dock zone indicator
        let draw_zone = |ui: &mut egui::Ui, rect: egui::Rect, zone: DockZone| {
            let is_hovered = self.hover_zone == Some(zone);
            let alpha = if is_hovered {
                highlight_alpha
            } else {
                indicator_alpha
            };
            let color = egui::Color32::from_rgba_unmultiplied(100, 150, 255, (alpha * 255.0) as u8);

            ui.painter().rect_filled(rect, 4.0, color);

            if is_hovered {
                ui.painter().rect_stroke(
                    rect,
                    4.0,
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(150, 200, 255)),
                    egui::epaint::StrokeKind::Outside,
                );
            }
        };

        egui::Area::new(egui::Id::new("dock_zone_overlay"))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                // Left zone
                draw_zone(
                    ui,
                    egui::Rect::from_min_size(
                        screen_rect.left_top(),
                        egui::vec2(margin, screen_rect.height()),
                    ),
                    DockZone::Left,
                );

                // Right zone
                draw_zone(
                    ui,
                    egui::Rect::from_min_size(
                        egui::pos2(screen_rect.right() - margin, screen_rect.top()),
                        egui::vec2(margin, screen_rect.height()),
                    ),
                    DockZone::Right,
                );

                // Note: Top/Bottom dock zones are disabled because they don't
                // have rendering implementations. Only Left/Right are supported.
            });
    }
}

/// Panel IDs for the standard panels
pub mod panel_ids {
    pub const CLIP_GRID: &str = "clip_grid";
    pub const EFFECTS_BROWSER: &str = "effects_browser";
    pub const FILES: &str = "files";
    pub const PREVIEW_MONITOR: &str = "preview_monitor";
    pub const PROPERTIES: &str = "properties";
    pub const SOURCES: &str = "sources";
}

