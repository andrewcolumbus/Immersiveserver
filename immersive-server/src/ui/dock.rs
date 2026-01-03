//! Docking system for detachable/resizable panels
//!
//! Provides a professional docking UI system where panels can be:
//! - Docked to edges (left, right, top, bottom)
//! - Floated as independent windows within the main window
//! - Undocked to separate native OS windows
//! - Combined into tabbed groups
//! - Dragged between dock zones
//! - Snapped to magnetic dock zones
//! - Snapped to other floating/undocked windows

use serde::{Deserialize, Serialize};

/// Distance threshold for window-to-window snapping (in pixels)
const WINDOW_SNAP_THRESHOLD: f32 = 15.0;

// ═══════════════════════════════════════════════════════════════════════════════
// PANEL PLACEMENT — Where a panel lives (extends DockZone concept)
// ═══════════════════════════════════════════════════════════════════════════════

/// Panel placement state - where a panel currently lives
///
/// This extends the concept of DockZone to include undocked (separate OS window)
/// and tabbed (part of a tab group) states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PanelPlacement {
    /// Docked to a zone within the main window
    Docked(DockZone),
    /// Floating as an egui window within the main window
    Floating,
    /// Undocked to a separate native OS window
    Undocked,
    /// Part of a tab group (stores the group ID)
    Tabbed { group_id: u32 },
}

impl Default for PanelPlacement {
    fn default() -> Self {
        PanelPlacement::Docked(DockZone::Right)
    }
}

impl PanelPlacement {
    /// Check if this placement requires a separate OS window
    pub fn is_undocked(&self) -> bool {
        matches!(self, PanelPlacement::Undocked)
    }

    /// Check if this placement is floating (within main window)
    pub fn is_floating(&self) -> bool {
        matches!(self, PanelPlacement::Floating)
    }

    /// Check if this placement is docked to an edge
    pub fn is_docked(&self) -> bool {
        matches!(self, PanelPlacement::Docked(_))
    }

    /// Check if this placement is part of a tab group
    pub fn is_tabbed(&self) -> bool {
        matches!(self, PanelPlacement::Tabbed { .. })
    }

    /// Get the dock zone if docked
    pub fn dock_zone(&self) -> Option<DockZone> {
        match self {
            PanelPlacement::Docked(zone) => Some(*zone),
            _ => None,
        }
    }

    /// Get the tab group ID if tabbed
    pub fn tab_group_id(&self) -> Option<u32> {
        match self {
            PanelPlacement::Tabbed { group_id } => Some(*group_id),
            _ => None,
        }
    }

    /// Convert from legacy DockZone for backward compatibility
    pub fn from_dock_zone(zone: DockZone) -> Self {
        match zone {
            DockZone::Floating => PanelPlacement::Floating,
            other => PanelPlacement::Docked(other),
        }
    }

    /// Convert to legacy DockZone (for existing code compatibility)
    /// Returns DockZone::Floating for Undocked and Tabbed placements
    pub fn to_dock_zone(&self) -> DockZone {
        match self {
            PanelPlacement::Docked(zone) => *zone,
            PanelPlacement::Floating | PanelPlacement::Undocked | PanelPlacement::Tabbed { .. } => {
                DockZone::Floating
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PANEL GEOMETRY — Position and size tracking
// ═══════════════════════════════════════════════════════════════════════════════

/// Geometry (position, size, constraints) for a panel in various states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelGeometry {
    /// Position - screen coords for undocked, window coords for floating
    pub position: (f32, f32),
    /// Size (width, height)
    pub size: (f32, f32),
    /// Minimum size constraints
    #[serde(default = "PanelGeometry::default_min_size")]
    pub min_size: (f32, f32),
    /// Whether the panel/window is maximized (for undocked windows)
    #[serde(default)]
    pub maximized: bool,
    /// Monitor index for undocked windows (for multi-monitor persistence)
    #[serde(default)]
    pub monitor: Option<usize>,
}

impl Default for PanelGeometry {
    fn default() -> Self {
        Self {
            position: (100.0, 100.0),
            size: (300.0, 400.0),
            min_size: Self::default_min_size(),
            maximized: false,
            monitor: None,
        }
    }
}

impl PanelGeometry {
    /// Create geometry at a specific position with default size
    pub fn at_position(x: f32, y: f32) -> Self {
        Self {
            position: (x, y),
            ..Default::default()
        }
    }

    /// Create geometry with specific position and size
    pub fn new(position: (f32, f32), size: (f32, f32)) -> Self {
        Self {
            position,
            size,
            ..Default::default()
        }
    }

    fn default_min_size() -> (f32, f32) {
        (200.0, 150.0)
    }

    /// Update position, clamping to minimum size
    pub fn set_position(&mut self, x: f32, y: f32) {
        self.position = (x, y);
    }

    /// Update size, respecting minimum constraints
    pub fn set_size(&mut self, width: f32, height: f32) {
        self.size = (width.max(self.min_size.0), height.max(self.min_size.1));
    }

    /// Get the bounding rectangle
    pub fn rect(&self) -> (f32, f32, f32, f32) {
        (
            self.position.0,
            self.position.1,
            self.position.0 + self.size.0,
            self.position.1 + self.size.1,
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TAB GROUP — Multiple panels in a tabbed container
// ═══════════════════════════════════════════════════════════════════════════════

/// A group of panels displayed as tabs in a single container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabGroup {
    /// Unique identifier for this tab group
    pub id: u32,
    /// Panel IDs in this group (order = tab order)
    pub panel_ids: Vec<String>,
    /// Currently active tab index
    #[serde(default)]
    pub active_tab: usize,
    /// Where the tab group container lives
    #[serde(default)]
    pub placement: TabGroupPlacement,
    /// Geometry of the tab group container
    #[serde(default)]
    pub geometry: PanelGeometry,
}

/// Where a tab group container lives (subset of PanelPlacement)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TabGroupPlacement {
    /// Floating within the main window
    #[default]
    Floating,
    /// Undocked to a separate native OS window
    Undocked,
}

impl TabGroup {
    /// Create a new tab group with initial panels
    pub fn new(id: u32, panel_ids: Vec<String>) -> Self {
        Self {
            id,
            panel_ids,
            active_tab: 0,
            placement: TabGroupPlacement::Floating,
            geometry: PanelGeometry::default(),
        }
    }

    /// Create a tab group at a specific position
    pub fn new_at(id: u32, panel_ids: Vec<String>, position: (f32, f32), size: (f32, f32)) -> Self {
        Self {
            id,
            panel_ids,
            active_tab: 0,
            placement: TabGroupPlacement::Floating,
            geometry: PanelGeometry::new(position, size),
        }
    }

    /// Get the ID of the currently active panel
    pub fn active_panel_id(&self) -> Option<&String> {
        self.panel_ids.get(self.active_tab)
    }

    /// Set the active tab by panel ID
    pub fn set_active_panel(&mut self, panel_id: &str) -> bool {
        if let Some(idx) = self.panel_ids.iter().position(|id| id == panel_id) {
            self.active_tab = idx;
            true
        } else {
            false
        }
    }

    /// Add a panel to the group
    pub fn add_panel(&mut self, panel_id: String) {
        if !self.panel_ids.contains(&panel_id) {
            self.panel_ids.push(panel_id);
        }
    }

    /// Add a panel at a specific index
    pub fn insert_panel(&mut self, index: usize, panel_id: String) {
        if !self.panel_ids.contains(&panel_id) {
            let idx = index.min(self.panel_ids.len());
            self.panel_ids.insert(idx, panel_id);
            // Adjust active tab if needed
            if self.active_tab >= idx && !self.panel_ids.is_empty() {
                self.active_tab = self.active_tab.saturating_add(1).min(self.panel_ids.len() - 1);
            }
        }
    }

    /// Remove a panel from the group, returns true if the panel was found
    pub fn remove_panel(&mut self, panel_id: &str) -> bool {
        if let Some(idx) = self.panel_ids.iter().position(|id| id == panel_id) {
            self.panel_ids.remove(idx);
            // Adjust active tab
            if self.active_tab >= self.panel_ids.len() && !self.panel_ids.is_empty() {
                self.active_tab = self.panel_ids.len() - 1;
            }
            true
        } else {
            false
        }
    }

    /// Get the number of panels in this group
    pub fn len(&self) -> usize {
        self.panel_ids.len()
    }

    /// Check if the group is empty
    pub fn is_empty(&self) -> bool {
        self.panel_ids.is_empty()
    }

    /// Check if this group should be dissolved (0 or 1 panels)
    pub fn should_dissolve(&self) -> bool {
        self.panel_ids.len() <= 1
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PANEL CATEGORY — For grouping in View menu
// ═══════════════════════════════════════════════════════════════════════════════

/// Panel categories for organization in menus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PanelCategory {
    #[default]
    General,
    Media,
    Properties,
    Monitoring,
    Tools,
}

// ═══════════════════════════════════════════════════════════════════════════════
// DOCK ZONE — Edge zones for panel docking
// ═══════════════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════════════
// DOCKABLE PANEL — Extended panel state
// ═══════════════════════════════════════════════════════════════════════════════

/// State for a dockable panel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockablePanel {
    /// Unique identifier for this panel
    pub id: String,
    /// Display title for the panel
    pub title: String,
    /// Current dock zone (kept for backward compatibility with existing serialization)
    pub zone: DockZone,
    /// Current placement state (new, preferred way to track placement)
    #[serde(default)]
    pub placement: PanelPlacement,
    /// Position when floating (window coordinates) - legacy, use floating_geometry instead
    #[serde(default)]
    pub floating_pos: Option<(f32, f32)>,
    /// Size when floating - legacy, use floating_geometry instead
    #[serde(default)]
    pub floating_size: Option<(f32, f32)>,
    /// Geometry when floating within main window
    #[serde(default)]
    pub floating_geometry: PanelGeometry,
    /// Geometry when undocked to separate native OS window
    #[serde(default)]
    pub undocked_geometry: PanelGeometry,
    /// Whether the panel is open/visible
    pub open: bool,
    /// Default width when docked horizontally
    #[serde(default = "default_dock_width")]
    pub dock_width: f32,
    /// Default height when docked vertically
    #[serde(default = "default_dock_height")]
    pub dock_height: f32,
    /// Last docked zone (for returning from float/undock)
    #[serde(default = "default_last_docked_zone")]
    pub last_docked_zone: DockZone,
    /// Whether this panel can be undocked to a separate window
    #[serde(default = "default_can_undock")]
    pub can_undock: bool,
    /// Whether this panel requires GPU rendering (Environment/Preview monitors)
    #[serde(default)]
    pub requires_gpu_rendering: bool,
    /// Panel category for organization in View menu
    #[serde(default)]
    pub category: PanelCategory,
}

fn default_dock_width() -> f32 {
    300.0
}

fn default_dock_height() -> f32 {
    200.0
}

fn default_last_docked_zone() -> DockZone {
    DockZone::Right
}

fn default_can_undock() -> bool {
    true
}

impl DockablePanel {
    /// Create a new dockable panel
    pub fn new(id: impl Into<String>, title: impl Into<String>, zone: DockZone) -> Self {
        let placement = PanelPlacement::from_dock_zone(zone);
        Self {
            id: id.into(),
            title: title.into(),
            zone,
            placement,
            floating_pos: None,
            floating_size: None,
            floating_geometry: PanelGeometry::default(),
            undocked_geometry: PanelGeometry::default(),
            open: true,
            dock_width: default_dock_width(),
            dock_height: default_dock_height(),
            last_docked_zone: if zone.is_floating() {
                DockZone::Right
            } else {
                zone
            },
            can_undock: true,
            requires_gpu_rendering: false,
            category: PanelCategory::General,
        }
    }

    /// Create a new panel with extended configuration
    pub fn new_extended(
        id: impl Into<String>,
        title: impl Into<String>,
        zone: DockZone,
        can_undock: bool,
        requires_gpu_rendering: bool,
        category: PanelCategory,
    ) -> Self {
        let mut panel = Self::new(id, title, zone);
        panel.can_undock = can_undock;
        panel.requires_gpu_rendering = requires_gpu_rendering;
        panel.category = category;
        panel
    }

    /// Check if this panel is floating (within main window)
    pub fn is_floating(&self) -> bool {
        self.placement.is_floating() || self.zone.is_floating()
    }

    /// Check if this panel is undocked (separate OS window)
    pub fn is_undocked(&self) -> bool {
        self.placement.is_undocked()
    }

    /// Check if this panel is docked to an edge
    pub fn is_docked(&self) -> bool {
        self.placement.is_docked()
    }

    /// Check if this panel is part of a tab group
    pub fn is_tabbed(&self) -> bool {
        self.placement.is_tabbed()
    }

    /// Get the effective placement (syncs zone and placement fields)
    pub fn effective_placement(&self) -> PanelPlacement {
        // If placement is explicitly set to Undocked or Tabbed, use it
        if self.placement.is_undocked() || self.placement.is_tabbed() {
            self.placement
        } else {
            // Otherwise, derive from zone for backward compatibility
            PanelPlacement::from_dock_zone(self.zone)
        }
    }

    /// Toggle between floating and a specific dock zone
    pub fn toggle_float(&mut self, default_dock: DockZone) {
        if self.is_floating() {
            self.dock_to(default_dock);
        } else {
            self.set_floating();
        }
    }

    /// Dock to a specific zone
    pub fn dock_to(&mut self, zone: DockZone) {
        self.zone = zone;
        self.placement = PanelPlacement::Docked(zone);
        if !zone.is_floating() {
            self.last_docked_zone = zone;
        }
    }

    /// Set panel to floating state (within main window)
    pub fn set_floating(&mut self) {
        self.zone = DockZone::Floating;
        self.placement = PanelPlacement::Floating;
    }

    /// Float at a specific position
    pub fn float_at(&mut self, pos: (f32, f32), size: (f32, f32)) {
        self.zone = DockZone::Floating;
        self.placement = PanelPlacement::Floating;
        self.floating_pos = Some(pos);
        self.floating_size = Some(size);
        self.floating_geometry.position = pos;
        self.floating_geometry.size = size;
    }

    /// Undock to a separate native OS window
    pub fn undock(&mut self) {
        if self.can_undock {
            // Save current position to undocked geometry if floating
            if self.is_floating() {
                if let Some(pos) = self.floating_pos {
                    self.undocked_geometry.position = pos;
                }
                if let Some(size) = self.floating_size {
                    self.undocked_geometry.size = size;
                }
            }
            self.placement = PanelPlacement::Undocked;
            // Keep zone as Floating for backward compatibility with code that checks zone
            self.zone = DockZone::Floating;
        }
    }

    /// Undock to a specific position
    pub fn undock_at(&mut self, pos: (f32, f32), size: (f32, f32)) {
        if self.can_undock {
            self.placement = PanelPlacement::Undocked;
            self.zone = DockZone::Floating;
            self.undocked_geometry.position = pos;
            self.undocked_geometry.size = size;
        }
    }

    /// Re-dock the panel (return from undocked or tabbed state)
    pub fn redock(&mut self) {
        self.dock_to(self.last_docked_zone);
    }

    /// Add this panel to a tab group
    pub fn add_to_tab_group(&mut self, group_id: u32) {
        self.placement = PanelPlacement::Tabbed { group_id };
        self.zone = DockZone::Floating;
    }

    /// Remove this panel from its tab group (return to floating)
    pub fn remove_from_tab_group(&mut self) {
        if self.is_tabbed() {
            self.placement = PanelPlacement::Floating;
        }
    }

    /// Get the current geometry based on placement
    pub fn current_geometry(&self) -> &PanelGeometry {
        match self.placement {
            PanelPlacement::Undocked => &self.undocked_geometry,
            _ => &self.floating_geometry,
        }
    }

    /// Get mutable reference to current geometry based on placement
    pub fn current_geometry_mut(&mut self) -> &mut PanelGeometry {
        match self.placement {
            PanelPlacement::Undocked => &mut self.undocked_geometry,
            _ => &mut self.floating_geometry,
        }
    }

    /// Sync legacy floating_pos/floating_size with floating_geometry
    /// Call this after loading from old format files
    pub fn sync_legacy_geometry(&mut self) {
        if let Some(pos) = self.floating_pos {
            self.floating_geometry.position = pos;
        }
        if let Some(size) = self.floating_size {
            self.floating_geometry.size = size;
        }
        // Also sync placement from zone for backward compatibility
        if !self.placement.is_undocked() && !self.placement.is_tabbed() {
            self.placement = PanelPlacement::from_dock_zone(self.zone);
        }
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

// ═══════════════════════════════════════════════════════════════════════════════
// DOCK ACTION — Actions that require handling by main.rs
// ═══════════════════════════════════════════════════════════════════════════════

/// Actions from the dock system that require handling by the application
#[derive(Debug, Clone)]
pub enum DockAction {
    /// Request to undock a panel to a separate OS window
    UndockPanel {
        panel_id: String,
        /// Position for the new window (screen coordinates)
        position: (f32, f32),
        /// Size for the new window
        size: (f32, f32),
    },
    /// Request to re-dock a panel (close its OS window, return to main window)
    RedockPanel {
        panel_id: String,
    },
}

// ═══════════════════════════════════════════════════════════════════════════════
// DOCK MANAGER — Manages all dockable panels and their states
// ═══════════════════════════════════════════════════════════════════════════════

/// Manages all dockable panels, tab groups, and their states
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DockManager {
    /// All registered panels
    panels: Vec<DockablePanel>,
    /// Tab groups (multiple panels in tabbed containers)
    #[serde(default)]
    tab_groups: Vec<TabGroup>,
    /// Next tab group ID to assign
    #[serde(default)]
    next_tab_group_id: u32,
    /// Panel currently being dragged (by index)
    #[serde(skip)]
    dragging_panel: Option<usize>,
    /// Dock zone currently being hovered during drag
    #[serde(skip)]
    hover_zone: Option<DockZone>,
    /// Tab group currently being hovered during drag (for tab-join)
    #[serde(skip)]
    hover_tab_group: Option<u32>,
    /// Whether we're in a drag operation
    #[serde(skip)]
    is_dragging: bool,
    /// Drag start position
    #[serde(skip)]
    drag_start_pos: Option<(f32, f32)>,
    /// Pending snap position to apply on next frame (panel_id -> position)
    #[serde(skip)]
    pending_snap: Option<(String, (f32, f32))>,
    /// Pending dock actions that require handling by main.rs
    #[serde(skip)]
    pending_actions: Vec<DockAction>,
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

    // ═══════════════════════════════════════════════════════════════════════════
    // Tab Group Management
    // ═══════════════════════════════════════════════════════════════════════════

    /// Get all tab groups
    pub fn tab_groups(&self) -> &[TabGroup] {
        &self.tab_groups
    }

    /// Get a tab group by ID
    pub fn get_tab_group(&self, id: u32) -> Option<&TabGroup> {
        self.tab_groups.iter().find(|g| g.id == id)
    }

    /// Get a mutable tab group by ID
    pub fn get_tab_group_mut(&mut self, id: u32) -> Option<&mut TabGroup> {
        self.tab_groups.iter_mut().find(|g| g.id == id)
    }

    /// Create a new tab group from panels
    /// Returns the new group ID, or None if any panel is already in a tab group
    pub fn create_tab_group(&mut self, panel_ids: Vec<String>) -> Option<u32> {
        // Verify all panels exist and are not already tabbed
        for panel_id in &panel_ids {
            let panel = self.get_panel(panel_id)?;
            if panel.is_tabbed() {
                return None;
            }
        }

        let group_id = self.next_tab_group_id;
        self.next_tab_group_id += 1;

        // Get geometry from first panel
        let geometry = self
            .get_panel(&panel_ids[0])
            .map(|p| p.floating_geometry.clone())
            .unwrap_or_default();

        // Update all panels to be tabbed
        for panel_id in &panel_ids {
            if let Some(panel) = self.get_panel_mut(panel_id) {
                panel.add_to_tab_group(group_id);
            }
        }

        self.tab_groups.push(TabGroup {
            id: group_id,
            panel_ids,
            active_tab: 0,
            placement: TabGroupPlacement::Floating,
            geometry,
        });

        Some(group_id)
    }

    /// Create a tab group at a specific position
    pub fn create_tab_group_at(
        &mut self,
        panel_ids: Vec<String>,
        position: (f32, f32),
        size: (f32, f32),
    ) -> Option<u32> {
        let group_id = self.create_tab_group(panel_ids)?;
        if let Some(group) = self.get_tab_group_mut(group_id) {
            group.geometry.position = position;
            group.geometry.size = size;
        }
        Some(group_id)
    }

    /// Add a panel to an existing tab group
    pub fn add_to_tab_group(&mut self, panel_id: &str, group_id: u32) -> bool {
        // Verify panel exists and is not already tabbed
        if let Some(panel) = self.get_panel(panel_id) {
            if panel.is_tabbed() {
                return false;
            }
        } else {
            return false;
        }

        // Add to group
        if let Some(group) = self.get_tab_group_mut(group_id) {
            group.add_panel(panel_id.to_string());
        } else {
            return false;
        }

        // Update panel state
        if let Some(panel) = self.get_panel_mut(panel_id) {
            panel.add_to_tab_group(group_id);
        }

        true
    }

    /// Remove a panel from its tab group
    /// Dissolves the group if only 0-1 panels remain
    pub fn remove_from_tab_group(&mut self, panel_id: &str) -> bool {
        // Find which group this panel is in
        let group_id = self
            .get_panel(panel_id)
            .and_then(|p| p.placement.tab_group_id());

        let group_id = match group_id {
            Some(id) => id,
            None => return false,
        };

        // Remove from group
        let should_dissolve = if let Some(group) = self.get_tab_group_mut(group_id) {
            group.remove_panel(panel_id);
            group.should_dissolve()
        } else {
            return false;
        };

        // Update panel state
        if let Some(panel) = self.get_panel_mut(panel_id) {
            panel.remove_from_tab_group();
        }

        // Dissolve group if needed
        if should_dissolve {
            self.dissolve_tab_group(group_id);
        }

        true
    }

    /// Dissolve a tab group, returning all panels to floating state
    pub fn dissolve_tab_group(&mut self, group_id: u32) {
        if let Some(idx) = self.tab_groups.iter().position(|g| g.id == group_id) {
            let group = self.tab_groups.remove(idx);

            // Return all panels to floating with the group's geometry
            for panel_id in group.panel_ids {
                if let Some(panel) = self.get_panel_mut(&panel_id) {
                    panel.remove_from_tab_group();
                    panel.floating_geometry = group.geometry.clone();
                    panel.floating_pos = Some(group.geometry.position);
                    panel.floating_size = Some(group.geometry.size);
                }
            }
        }
    }

    /// Set the active tab in a group
    pub fn set_active_tab(&mut self, group_id: u32, tab_index: usize) {
        if let Some(group) = self.get_tab_group_mut(group_id) {
            if tab_index < group.panel_ids.len() {
                group.active_tab = tab_index;
            }
        }
    }

    /// Set the active tab by panel ID
    pub fn set_active_tab_by_panel(&mut self, panel_id: &str) -> bool {
        // Find the group containing this panel
        let group_id = self
            .get_panel(panel_id)
            .and_then(|p| p.placement.tab_group_id());

        if let Some(group_id) = group_id {
            if let Some(group) = self.get_tab_group_mut(group_id) {
                return group.set_active_panel(panel_id);
            }
        }
        false
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Undocking Management
    // ═══════════════════════════════════════════════════════════════════════════

    /// Undock a panel to a separate OS window
    pub fn undock_panel(&mut self, panel_id: &str) -> bool {
        // If in a tab group, remove first
        let was_tabbed = if let Some(panel) = self.get_panel(panel_id) {
            panel.is_tabbed()
        } else {
            return false;
        };

        if was_tabbed {
            self.remove_from_tab_group(panel_id);
        }

        // Now undock
        if let Some(panel) = self.get_panel_mut(panel_id) {
            if panel.can_undock {
                panel.undock();
                return true;
            }
        }
        false
    }

    /// Undock a panel to a specific position
    pub fn undock_panel_at(&mut self, panel_id: &str, pos: (f32, f32), size: (f32, f32)) -> bool {
        // If in a tab group, remove first
        let was_tabbed = if let Some(panel) = self.get_panel(panel_id) {
            panel.is_tabbed()
        } else {
            return false;
        };

        if was_tabbed {
            self.remove_from_tab_group(panel_id);
        }

        // Now undock
        if let Some(panel) = self.get_panel_mut(panel_id) {
            if panel.can_undock {
                panel.undock_at(pos, size);
                return true;
            }
        }
        false
    }

    /// Re-dock a panel (return from undocked state to last docked zone)
    pub fn redock_panel(&mut self, panel_id: &str) -> bool {
        if let Some(panel) = self.get_panel_mut(panel_id) {
            if panel.is_undocked() {
                panel.redock();
                return true;
            }
        }
        false
    }

    /// Request to undock a panel - queues a DockAction for main.rs to handle
    ///
    /// This method marks the panel as undocked and queues an action to create
    /// the OS window. The actual window creation is handled by main.rs.
    pub fn request_undock(&mut self, panel_id: &str) {
        if let Some(panel) = self.get_panel(panel_id) {
            if !panel.can_undock || panel.is_undocked() {
                return;
            }
            let geometry = panel.undocked_geometry.clone();
            let position = geometry.position;
            let size = geometry.size;
            let panel_id = panel_id.to_string();

            // Mark as undocked
            if self.undock_panel(&panel_id) {
                // Queue action for main.rs
                self.pending_actions.push(DockAction::UndockPanel {
                    panel_id,
                    position,
                    size,
                });
            }
        }
    }

    /// Request to re-dock a panel - queues a DockAction for main.rs to handle
    ///
    /// This method marks the panel as docked and queues an action to close
    /// the OS window. The actual window destruction is handled by main.rs.
    pub fn request_redock(&mut self, panel_id: &str) {
        if let Some(panel) = self.get_panel(panel_id) {
            if !panel.is_undocked() {
                return;
            }
            let panel_id = panel_id.to_string();

            // Mark as docked
            if self.redock_panel(&panel_id) {
                // Queue action for main.rs
                self.pending_actions.push(DockAction::RedockPanel { panel_id });
            }
        }
    }

    /// Take all pending dock actions (drains the queue)
    pub fn take_pending_actions(&mut self) -> Vec<DockAction> {
        std::mem::take(&mut self.pending_actions)
    }

    /// Check if there are pending dock actions
    pub fn has_pending_actions(&self) -> bool {
        !self.pending_actions.is_empty()
    }

    /// Dock a panel to a specific zone
    pub fn dock_panel_to(&mut self, panel_id: &str, zone: DockZone) {
        // If in a tab group, remove first
        let was_tabbed = if let Some(panel) = self.get_panel(panel_id) {
            panel.is_tabbed()
        } else {
            return;
        };

        if was_tabbed {
            self.remove_from_tab_group(panel_id);
        }

        if let Some(panel) = self.get_panel_mut(panel_id) {
            panel.dock_to(zone);
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Query Methods
    // ═══════════════════════════════════════════════════════════════════════════

    /// Get all panels (for iteration)
    pub fn all_panels(&self) -> impl Iterator<Item = &DockablePanel> {
        self.panels.iter()
    }

    /// Get all open panels
    pub fn open_panels(&self) -> impl Iterator<Item = &DockablePanel> {
        self.panels.iter().filter(|p| p.open)
    }

    /// Get panels that need separate OS windows (undocked)
    pub fn undocked_panels(&self) -> Vec<&DockablePanel> {
        self.panels
            .iter()
            .filter(|p| p.open && p.is_undocked())
            .collect()
    }

    /// Get panels that require GPU rendering
    pub fn gpu_panels(&self) -> Vec<&DockablePanel> {
        self.panels
            .iter()
            .filter(|p| p.requires_gpu_rendering)
            .collect()
    }

    /// Get the hovered tab group during drag
    pub fn hovered_tab_group(&self) -> Option<u32> {
        self.hover_tab_group
    }

    /// Sync all panels' legacy geometry fields
    /// Call this after loading from old format files
    pub fn sync_all_legacy_geometry(&mut self) {
        for panel in &mut self.panels {
            panel.sync_legacy_geometry();
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
    pub const PERFORMANCE: &str = "performance";
    pub const PREVIS: &str = "previs";
    pub const PREVIEW_MONITOR: &str = "preview_monitor";
    pub const PROPERTIES: &str = "properties";
    pub const SOURCES: &str = "sources";
}

