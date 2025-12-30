//! Docking system for detachable/resizable panels
//!
//! Provides a professional docking UI system where panels can be:
//! - Docked to edges (left, right, top, bottom)
//! - Floated as independent windows
//! - Dragged between dock zones
//! - Snapped to magnetic dock zones

use serde::{Deserialize, Serialize};

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
        let margin = 80.0; // Size of magnetic zone at edges

        self.hover_zone = if pos.0 < margin {
            Some(DockZone::Left)
        } else if pos.0 > window_size.0 - margin {
            Some(DockZone::Right)
        } else if pos.1 < margin + 30.0 {
            // Account for menu bar
            Some(DockZone::Top)
        } else if pos.1 > window_size.1 - margin {
            Some(DockZone::Bottom)
        } else {
            None // Floating zone
        };
    }

    /// End drag operation - dock or float the panel
    pub fn end_drag(&mut self, pos: (f32, f32)) {
        if let Some(idx) = self.dragging_panel.take() {
            if let Some(panel) = self.panels.get_mut(idx) {
                if let Some(zone) = self.hover_zone.take() {
                    // Dock to the hovered zone
                    panel.zone = zone;
                } else {
                    // Float at the current position
                    let size = panel.floating_size.unwrap_or((300.0, 400.0));
                    panel.float_at(pos, size);
                }
            }
        }
        self.is_dragging = false;
        self.drag_start_pos = None;
        self.hover_zone = None;
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

                // Top zone
                draw_zone(
                    ui,
                    egui::Rect::from_min_size(
                        egui::pos2(screen_rect.left() + margin, screen_rect.top()),
                        egui::vec2(screen_rect.width() - margin * 2.0, margin),
                    ),
                    DockZone::Top,
                );

                // Bottom zone
                draw_zone(
                    ui,
                    egui::Rect::from_min_size(
                        egui::pos2(screen_rect.left() + margin, screen_rect.bottom() - margin),
                        egui::vec2(screen_rect.width() - margin * 2.0, margin),
                    ),
                    DockZone::Bottom,
                );
            });
    }
}

/// Panel IDs for the standard panels
pub mod panel_ids {
    pub const CLIP_GRID: &str = "clip_grid";
    pub const PROPERTIES: &str = "properties";
}

