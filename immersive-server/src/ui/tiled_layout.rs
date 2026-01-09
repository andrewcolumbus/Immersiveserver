//! Tiled window layout system using a binary split tree.
//!
//! This module provides a tmux/vim-style tiled layout where panels always fill
//! 100% of available space. The layout is represented as a binary tree where
//! each node is either a split (with two children) or a leaf (containing panels).
//!
//! # Architecture
//!
//! ```text
//! TiledLayout
//!   └── root: TileNode
//!         ├── Split { direction, ratio, first, second }
//!         │     ├── first: Box<TileNode>
//!         │     └── second: Box<TileNode>
//!         └── Leaf(TabbedCell)
//!               └── panel_ids: Vec<String>
//! ```
//!
//! # Example Layout
//!
//! ```text
//! +------------------+----------+
//! |                  | Props    |
//! |   Environment    +----------+
//! |   (viewport)     | Effects  |
//! +--------+---------+----------+
//! | Clips  | Sources | Files    |
//! +--------+---------+----------+
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::dock::panel_ids;

// ============================================================================
// Panel Restrictions
// ============================================================================

/// Panels that cannot be undocked due to drag-and-drop functionality.
///
/// egui's DragAndDrop API uses context-local payloads that don't work across
/// separate windows. These panels rely on drag-and-drop for their primary
/// functionality, so they must remain in the tiled layout.
pub const NON_UNDOCKABLE_PANELS: &[&str] = &[
    panel_ids::CLIP_GRID,      // Drop target for sources/files
    panel_ids::PROPERTIES,     // Drop target for effects + drag source for reordering
    panel_ids::SOURCES,        // Drag source for OMT/NDI/File streams
    panel_ids::EFFECTS_BROWSER, // Drag source for effects
    panel_ids::FILES,          // Drag source for video/image files
];

/// Returns true if the given panel can be undocked from the tiled layout.
///
/// Panels that participate in drag-and-drop operations cannot be undocked
/// because egui's DragAndDrop API uses context-local payloads that don't
/// transfer between separate windows.
pub fn can_panel_undock(panel_id: &str) -> bool {
    !NON_UNDOCKABLE_PANELS.contains(&panel_id)
}

/// Panels that are always floating windows, never in tiled layout.
///
/// These panels are utility windows that don't need a permanent place in the
/// main editing grid. They can be toggled via View menu and appear as floating
/// egui::Windows.
pub const FLOATING_ONLY_PANELS: &[&str] = &[
    panel_ids::PERFORMANCE,
    panel_ids::PREVIS,
];

/// Returns true if the panel should always be a floating window.
pub fn is_floating_only(panel_id: &str) -> bool {
    FLOATING_ONLY_PANELS.contains(&panel_id)
}

// ============================================================================
// Core Types
// ============================================================================

/// Direction of a split divider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitDirection {
    /// Horizontal split: left | right (divider is vertical line)
    Horizontal,
    /// Vertical split: top / bottom (divider is horizontal line)
    Vertical,
}

impl SplitDirection {
    /// Returns the perpendicular direction.
    pub fn perpendicular(&self) -> Self {
        match self {
            SplitDirection::Horizontal => SplitDirection::Vertical,
            SplitDirection::Vertical => SplitDirection::Horizontal,
        }
    }
}

/// A node in the binary split tree.
///
/// The tree is always balanced in structure (binary), but the visual balance
/// depends on the split ratios.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TileNode {
    /// A split node dividing space between two children.
    Split {
        /// Direction of the split.
        direction: SplitDirection,
        /// Position of the divider as a ratio (0.0-1.0).
        /// For horizontal: ratio of width given to first child.
        /// For vertical: ratio of height given to first child.
        ratio: f32,
        /// First child (left for horizontal, top for vertical).
        first: Box<TileNode>,
        /// Second child (right for horizontal, bottom for vertical).
        second: Box<TileNode>,
    },
    /// A leaf node containing one or more tabbed panels.
    Leaf(TabbedCell),
}

impl TileNode {
    /// Creates a new leaf node with a single panel.
    pub fn leaf(cell_id: u32, panel_id: impl Into<String>) -> Self {
        TileNode::Leaf(TabbedCell::new(cell_id, panel_id.into()))
    }

    /// Creates a new split node with two children.
    pub fn split(direction: SplitDirection, ratio: f32, first: TileNode, second: TileNode) -> Self {
        TileNode::Split {
            direction,
            ratio: ratio.clamp(0.1, 0.9),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Returns true if this is a leaf node.
    pub fn is_leaf(&self) -> bool {
        matches!(self, TileNode::Leaf(_))
    }

    /// Returns the cell if this is a leaf node.
    pub fn as_leaf(&self) -> Option<&TabbedCell> {
        match self {
            TileNode::Leaf(cell) => Some(cell),
            TileNode::Split { .. } => None,
        }
    }

    /// Returns the cell mutably if this is a leaf node.
    pub fn as_leaf_mut(&mut self) -> Option<&mut TabbedCell> {
        match self {
            TileNode::Leaf(cell) => Some(cell),
            TileNode::Split { .. } => None,
        }
    }
}

/// A cell that can contain multiple tabbed panels.
///
/// When a cell has multiple panels, they are displayed as tabs with the
/// active tab's content visible.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabbedCell {
    /// Unique identifier for this cell.
    pub id: u32,
    /// Panel IDs in this cell (order determines tab order).
    pub panel_ids: Vec<String>,
    /// Index of the currently active/visible tab.
    pub active_tab: usize,
}

impl TabbedCell {
    /// Creates a new cell with a single panel.
    pub fn new(id: u32, panel_id: String) -> Self {
        Self {
            id,
            panel_ids: vec![panel_id],
            active_tab: 0,
        }
    }

    /// Creates a new cell with multiple panels.
    pub fn with_panels(id: u32, panel_ids: Vec<String>) -> Self {
        Self {
            id,
            panel_ids,
            active_tab: 0,
        }
    }

    /// Returns the active panel ID, if any.
    pub fn active_panel(&self) -> Option<&String> {
        self.panel_ids.get(self.active_tab)
    }

    /// Returns true if this cell contains the given panel.
    pub fn contains_panel(&self, panel_id: &str) -> bool {
        self.panel_ids.iter().any(|id| id == panel_id)
    }

    /// Adds a panel as a new tab and makes it active.
    pub fn add_panel(&mut self, panel_id: String) {
        if !self.contains_panel(&panel_id) {
            self.panel_ids.push(panel_id);
            self.active_tab = self.panel_ids.len() - 1;
        }
    }

    /// Removes a panel from this cell. Returns true if removed.
    pub fn remove_panel(&mut self, panel_id: &str) -> bool {
        if let Some(idx) = self.panel_ids.iter().position(|id| id == panel_id) {
            self.panel_ids.remove(idx);
            // Adjust active tab if needed
            if self.active_tab >= self.panel_ids.len() && !self.panel_ids.is_empty() {
                self.active_tab = self.panel_ids.len() - 1;
            }
            true
        } else {
            false
        }
    }

    /// Returns true if this cell has no panels.
    pub fn is_empty(&self) -> bool {
        self.panel_ids.is_empty()
    }

    /// Returns the number of tabs in this cell.
    pub fn tab_count(&self) -> usize {
        self.panel_ids.len()
    }
}

// ============================================================================
// Computed Layout (output of layout algorithm)
// ============================================================================

/// Result of computing layout rectangles from the tree.
#[derive(Debug, Clone, Default)]
pub struct ComputedLayout {
    /// Map from cell ID to its computed rectangle.
    pub cell_rects: HashMap<u32, egui::Rect>,
    /// Map from panel ID to (cell_id, tab_index).
    pub panel_locations: HashMap<String, (u32, usize)>,
    /// List of dividers with their rectangles and metadata.
    pub dividers: Vec<DividerInfo>,
}

/// Information about a divider for hit testing and rendering.
#[derive(Debug, Clone)]
pub struct DividerInfo {
    /// Hit/render rectangle for the divider.
    pub rect: egui::Rect,
    /// Direction of the split this divider belongs to.
    pub direction: SplitDirection,
    /// Path through the tree to reach this divider's split node.
    /// Each element is 0 (first child) or 1 (second child).
    pub tree_path: Vec<usize>,
    /// Current ratio of the split.
    pub ratio: f32,
    /// The parent rectangle (coordinate space) for this divider's ratio calculations.
    /// This is the split node's full area, NOT the entire window.
    pub parent_rect: egui::Rect,
}

// ============================================================================
// Divider Drag State
// ============================================================================

/// State for tracking divider drag operations.
#[derive(Debug, Clone, Default)]
pub struct DividerDragState {
    /// Path to the split node being dragged (None if not dragging).
    pub dragging_path: Option<Vec<usize>>,
    /// Direction of the split being dragged.
    pub drag_direction: Option<SplitDirection>,
    /// Mouse position when drag started.
    pub drag_start_pos: Option<egui::Pos2>,
    /// Split ratio when drag started.
    pub start_ratio: f32,
    /// Rectangle of the split's parent area (for ratio calculation).
    pub parent_rect: Option<egui::Rect>,
}

impl DividerDragState {
    /// Returns true if currently dragging a divider.
    pub fn is_dragging(&self) -> bool {
        self.dragging_path.is_some()
    }

    /// Clears the drag state.
    pub fn clear(&mut self) {
        self.dragging_path = None;
        self.drag_direction = None;
        self.drag_start_pos = None;
        self.start_ratio = 0.0;
        self.parent_rect = None;
    }
}

// ============================================================================
// Panel Drag State (for moving panels between cells)
// ============================================================================

/// State for tracking panel drag operations (moving panels between cells).
#[derive(Debug, Clone, Default)]
pub struct PanelDragState {
    /// ID of the panel being dragged (None if not dragging).
    pub dragged_panel: Option<String>,
    /// Source cell ID where the panel came from.
    pub source_cell_id: Option<u32>,
    /// Current mouse position during drag.
    pub current_pos: Option<egui::Pos2>,
}

impl PanelDragState {
    /// Returns true if currently dragging a panel.
    pub fn is_dragging(&self) -> bool {
        self.dragged_panel.is_some()
    }

    /// Starts dragging a panel.
    pub fn start(&mut self, panel_id: String, cell_id: u32, pos: egui::Pos2) {
        self.dragged_panel = Some(panel_id);
        self.source_cell_id = Some(cell_id);
        self.current_pos = Some(pos);
    }

    /// Clears the drag state.
    pub fn clear(&mut self) {
        self.dragged_panel = None;
        self.source_cell_id = None;
        self.current_pos = None;
    }
}

/// Target location for dropping a panel.
#[derive(Debug, Clone, PartialEq)]
pub enum DropTarget {
    /// Drop as a new tab in an existing cell.
    Tab { cell_id: u32 },
    /// Drop to create a new split.
    Split {
        cell_id: u32,
        direction: SplitDirection,
        /// True if the new panel should be first (left/top).
        new_first: bool,
    },
}

// ============================================================================
// TiledLayout - Main Manager
// ============================================================================

/// Manages the tiled layout tree and provides operations for manipulating it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TiledLayout {
    /// Root of the split tree.
    root: TileNode,
    /// Next cell ID to assign.
    next_cell_id: u32,
    /// Panel IDs that are undocked (in separate OS windows, not in tree).
    #[serde(default)]
    pub undocked_panels: Vec<String>,
    /// Cell ID containing the Environment viewport (protected from closing).
    environment_cell_id: u32,
    /// Currently focused cell ID.
    #[serde(skip)]
    focused_cell_id: Option<u32>,
    /// Minimum panel size in pixels.
    #[serde(default = "default_min_panel_size")]
    pub min_panel_size: f32,
    /// Divider thickness in pixels.
    #[serde(default = "default_divider_thickness")]
    pub divider_thickness: f32,
}

fn default_min_panel_size() -> f32 {
    100.0
}

fn default_divider_thickness() -> f32 {
    4.0
}

impl Default for TiledLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl TiledLayout {
    /// Creates a new layout with just the Environment viewport.
    pub fn new() -> Self {
        let env_cell = TabbedCell::new(0, panel_ids::ENVIRONMENT.to_string());
        Self {
            root: TileNode::Leaf(env_cell),
            next_cell_id: 1,
            undocked_panels: Vec::new(),
            environment_cell_id: 0,
            focused_cell_id: Some(0),
            min_panel_size: default_min_panel_size(),
            divider_thickness: default_divider_thickness(),
        }
    }

    /// Creates a default layout with common panels arranged sensibly.
    pub fn default_layout() -> Self {
        let mut layout = Self {
            root: TileNode::Leaf(TabbedCell::new(0, "placeholder".to_string())),
            next_cell_id: 0,
            undocked_panels: Vec::new(),
            environment_cell_id: 0,
            focused_cell_id: None,
            min_panel_size: default_min_panel_size(),
            divider_thickness: default_divider_thickness(),
        };

        // Build the default layout tree:
        // Split(V, 0.7)
        // ├── Split(H, 0.7)
        // │   ├── Leaf(environment)          [cell 0]
        // │   └── Split(V, 0.5)
        // │       ├── Leaf(properties)       [cell 1]
        // │       └── Leaf(effects_browser)  [cell 2]
        // └── Split(H, 0.33)
        //     ├── Leaf(clip_grid)            [cell 3]
        //     └── Split(H, 0.5)
        //         ├── Leaf(sources)          [cell 4]
        //         └── Leaf(files)            [cell 5]

        let environment = TileNode::leaf(0, panel_ids::ENVIRONMENT);
        let properties = TileNode::leaf(1, panel_ids::PROPERTIES);
        let effects = TileNode::leaf(2, panel_ids::EFFECTS_BROWSER);
        let clips = TileNode::leaf(3, panel_ids::CLIP_GRID);
        let sources = TileNode::leaf(4, panel_ids::SOURCES);
        let files = TileNode::leaf(5, panel_ids::FILES);

        // Right column: properties / effects
        let right_column = TileNode::split(SplitDirection::Vertical, 0.5, properties, effects);

        // Top row: environment | right_column
        let top_row = TileNode::split(SplitDirection::Horizontal, 0.7, environment, right_column);

        // Bottom right: sources | files
        let bottom_right = TileNode::split(SplitDirection::Horizontal, 0.5, sources, files);

        // Bottom row: clips | bottom_right (50% for clip grid)
        let bottom_row = TileNode::split(SplitDirection::Horizontal, 0.5, clips, bottom_right);

        // Root: top_row / bottom_row
        layout.root = TileNode::split(SplitDirection::Vertical, 0.7, top_row, bottom_row);
        layout.next_cell_id = 6;
        layout.environment_cell_id = 0;
        layout.focused_cell_id = Some(0);

        layout
    }

    /// Returns the root node of the tree.
    pub fn root(&self) -> &TileNode {
        &self.root
    }

    /// Returns the currently focused cell ID.
    pub fn focused_cell_id(&self) -> Option<u32> {
        self.focused_cell_id
    }

    /// Sets the focused cell.
    pub fn set_focused_cell(&mut self, cell_id: u32) {
        self.focused_cell_id = Some(cell_id);
    }

    /// Returns the environment cell ID (always exists, cannot be closed).
    pub fn get_environment_cell_id(&self) -> u32 {
        self.environment_cell_id
    }

    /// Returns the list of undocked panel IDs.
    pub fn undocked_panels(&self) -> &[String] {
        &self.undocked_panels
    }

    /// Allocates a new unique cell ID.
    fn alloc_cell_id(&mut self) -> u32 {
        let id = self.next_cell_id;
        self.next_cell_id += 1;
        id
    }

    // ========================================================================
    // Layout Computation
    // ========================================================================

    /// Computes pixel rectangles for all cells and dividers.
    pub fn compute_layout(&self, available_rect: egui::Rect) -> ComputedLayout {
        let mut result = ComputedLayout::default();

        self.compute_node_layout(
            &self.root,
            available_rect,
            &mut result,
            Vec::new(),
        );

        result
    }

    fn compute_node_layout(
        &self,
        node: &TileNode,
        rect: egui::Rect,
        result: &mut ComputedLayout,
        path: Vec<usize>,
    ) {
        match node {
            TileNode::Leaf(cell) => {
                result.cell_rects.insert(cell.id, rect);
                for (idx, panel_id) in cell.panel_ids.iter().enumerate() {
                    result.panel_locations.insert(panel_id.clone(), (cell.id, idx));
                }
            }
            TileNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let divider_half = self.divider_thickness / 2.0;

                let (first_rect, divider_rect, second_rect) = match direction {
                    SplitDirection::Horizontal => {
                        let split_x = rect.left() + rect.width() * ratio;
                        (
                            egui::Rect::from_min_max(
                                rect.min,
                                egui::pos2(split_x - divider_half, rect.bottom()),
                            ),
                            egui::Rect::from_min_max(
                                egui::pos2(split_x - divider_half, rect.top()),
                                egui::pos2(split_x + divider_half, rect.bottom()),
                            ),
                            egui::Rect::from_min_max(
                                egui::pos2(split_x + divider_half, rect.top()),
                                rect.max,
                            ),
                        )
                    }
                    SplitDirection::Vertical => {
                        let split_y = rect.top() + rect.height() * ratio;
                        (
                            egui::Rect::from_min_max(
                                rect.min,
                                egui::pos2(rect.right(), split_y - divider_half),
                            ),
                            egui::Rect::from_min_max(
                                egui::pos2(rect.left(), split_y - divider_half),
                                egui::pos2(rect.right(), split_y + divider_half),
                            ),
                            egui::Rect::from_min_max(
                                egui::pos2(rect.left(), split_y + divider_half),
                                rect.max,
                            ),
                        )
                    }
                };

                result.dividers.push(DividerInfo {
                    rect: divider_rect,
                    direction: *direction,
                    tree_path: path.clone(),
                    ratio: *ratio,
                    parent_rect: rect, // The split node's full area for ratio calculations
                });

                let mut first_path = path.clone();
                first_path.push(0);
                self.compute_node_layout(first, first_rect, result, first_path);

                let mut second_path = path;
                second_path.push(1);
                self.compute_node_layout(second, second_rect, result, second_path);
            }
        }
    }

    // ========================================================================
    // Tree Queries
    // ========================================================================

    /// Finds the cell containing a panel.
    pub fn find_panel_location(&self, panel_id: &str) -> Option<(u32, usize)> {
        self.find_panel_in_node(&self.root, panel_id)
    }

    fn find_panel_in_node(&self, node: &TileNode, panel_id: &str) -> Option<(u32, usize)> {
        match node {
            TileNode::Leaf(cell) => {
                cell.panel_ids
                    .iter()
                    .position(|id| id == panel_id)
                    .map(|idx| (cell.id, idx))
            }
            TileNode::Split { first, second, .. } => {
                self.find_panel_in_node(first, panel_id)
                    .or_else(|| self.find_panel_in_node(second, panel_id))
            }
        }
    }

    /// Finds a cell by ID.
    pub fn find_cell(&self, cell_id: u32) -> Option<&TabbedCell> {
        self.find_cell_in_node(&self.root, cell_id)
    }

    fn find_cell_in_node<'a>(&self, node: &'a TileNode, cell_id: u32) -> Option<&'a TabbedCell> {
        match node {
            TileNode::Leaf(cell) if cell.id == cell_id => Some(cell),
            TileNode::Leaf(_) => None,
            TileNode::Split { first, second, .. } => {
                self.find_cell_in_node(first, cell_id)
                    .or_else(|| self.find_cell_in_node(second, cell_id))
            }
        }
    }

    /// Finds a cell by ID (mutable).
    pub fn find_cell_mut(&mut self, cell_id: u32) -> Option<&mut TabbedCell> {
        Self::find_cell_in_node_mut(&mut self.root, cell_id)
    }

    fn find_cell_in_node_mut(node: &mut TileNode, cell_id: u32) -> Option<&mut TabbedCell> {
        match node {
            TileNode::Leaf(cell) if cell.id == cell_id => Some(cell),
            TileNode::Leaf(_) => None,
            TileNode::Split { first, second, .. } => {
                Self::find_cell_in_node_mut(first, cell_id)
                    .or_else(|| Self::find_cell_in_node_mut(second, cell_id))
            }
        }
    }

    /// Returns all panel IDs currently in the tiled layout.
    pub fn all_panel_ids(&self) -> Vec<String> {
        let mut result = Vec::new();
        self.collect_panels(&self.root, &mut result);
        result
    }

    fn collect_panels(&self, node: &TileNode, result: &mut Vec<String>) {
        match node {
            TileNode::Leaf(cell) => {
                result.extend(cell.panel_ids.clone());
            }
            TileNode::Split { first, second, .. } => {
                self.collect_panels(first, result);
                self.collect_panels(second, result);
            }
        }
    }

    /// Checks if a panel is in the tiled layout (not undocked).
    pub fn is_panel_in_layout(&self, panel_id: &str) -> bool {
        self.find_panel_location(panel_id).is_some()
    }

    // ========================================================================
    // Divider Operations
    // ========================================================================

    /// Updates a split ratio at the given tree path.
    pub fn set_ratio_at_path(&mut self, path: &[usize], new_ratio: f32) {
        Self::set_ratio_recursive(&mut self.root, path, new_ratio.clamp(0.1, 0.9));
    }

    fn set_ratio_recursive(node: &mut TileNode, path: &[usize], ratio: f32) {
        if path.is_empty() {
            if let TileNode::Split { ratio: r, .. } = node {
                *r = ratio;
            }
        } else if let TileNode::Split { first, second, .. } = node {
            match path[0] {
                0 => Self::set_ratio_recursive(first, &path[1..], ratio),
                1 => Self::set_ratio_recursive(second, &path[1..], ratio),
                _ => {}
            }
        }
    }

    /// Calculates a new ratio based on drag position.
    pub fn calculate_drag_ratio(
        &self,
        drag_state: &DividerDragState,
        current_pos: egui::Pos2,
    ) -> Option<f32> {
        let direction = drag_state.drag_direction?;
        let parent_rect = drag_state.parent_rect?;

        let new_ratio = match direction {
            SplitDirection::Horizontal => {
                (current_pos.x - parent_rect.left()) / parent_rect.width()
            }
            SplitDirection::Vertical => {
                (current_pos.y - parent_rect.top()) / parent_rect.height()
            }
        };

        // Clamp based on minimum panel size
        let total_size = match direction {
            SplitDirection::Horizontal => parent_rect.width(),
            SplitDirection::Vertical => parent_rect.height(),
        };
        let min_ratio = self.min_panel_size / total_size;
        let max_ratio = 1.0 - min_ratio;

        Some(new_ratio.clamp(min_ratio.max(0.1), max_ratio.min(0.9)))
    }

    // ========================================================================
    // Panel Operations
    // ========================================================================

    /// Splits a cell and adds a new panel to the new cell.
    ///
    /// Returns the new cell ID, or None if the panel wasn't found.
    pub fn split_cell(
        &mut self,
        cell_id: u32,
        direction: SplitDirection,
        new_panel_id: String,
        new_first: bool,
    ) -> Option<u32> {
        let new_cell_id = self.alloc_cell_id();
        let new_cell = TabbedCell::new(new_cell_id, new_panel_id);

        if Self::replace_leaf_with_split(
            &mut self.root,
            cell_id,
            direction,
            new_cell,
            new_first,
        ) {
            Some(new_cell_id)
        } else {
            // Rollback cell ID allocation
            self.next_cell_id -= 1;
            None
        }
    }

    fn replace_leaf_with_split(
        node: &mut TileNode,
        target_cell_id: u32,
        direction: SplitDirection,
        new_cell: TabbedCell,
        new_first: bool,
    ) -> bool {
        match node {
            TileNode::Leaf(cell) if cell.id == target_cell_id => {
                let old_leaf = std::mem::replace(
                    node,
                    TileNode::Leaf(TabbedCell::new(0, String::new())),
                );
                let new_leaf = TileNode::Leaf(new_cell);

                let (first, second) = if new_first {
                    (Box::new(new_leaf), Box::new(old_leaf))
                } else {
                    (Box::new(old_leaf), Box::new(new_leaf))
                };

                *node = TileNode::Split {
                    direction,
                    ratio: 0.5,
                    first,
                    second,
                };
                true
            }
            TileNode::Leaf(_) => false,
            TileNode::Split { first, second, .. } => {
                Self::replace_leaf_with_split(first, target_cell_id, direction, new_cell.clone(), new_first)
                    || Self::replace_leaf_with_split(second, target_cell_id, direction, new_cell, new_first)
            }
        }
    }

    /// Returns true if the panel is currently in the tiled layout (not undocked, not hidden).
    pub fn contains_panel(&self, panel_id: &str) -> bool {
        self.find_panel_location(panel_id).is_some()
    }

    /// Toggle a panel's visibility. If in layout, remove it. If not, add it.
    ///
    /// Returns true if the toggle was successful.
    pub fn toggle_panel(&mut self, panel_id: &str) -> bool {
        use crate::ui::dock::panel_ids;

        if self.contains_panel(panel_id) {
            // Panel is visible in layout - close it (unless it's environment)
            if panel_id != panel_ids::ENVIRONMENT {
                self.close_panel(panel_id)
            } else {
                false // Cannot toggle environment
            }
        } else if self.undocked_panels.contains(&panel_id.to_string()) {
            // Panel is undocked (floating) - remove from undocked list (hide it)
            self.undocked_panels.retain(|p| p != panel_id);
            true
        } else {
            // Panel is hidden - add it to the focused cell (or environment cell) as a tab
            let target_cell = self.focused_cell_id.unwrap_or(self.environment_cell_id);
            self.add_tab(target_cell, panel_id.to_string());
            true
        }
    }

    /// Closes a panel, removing it from its cell.
    ///
    /// If the cell becomes empty, collapses the tree so the sibling expands.
    /// Returns false if trying to close the Environment viewport (protected).
    pub fn close_panel(&mut self, panel_id: &str) -> bool {
        // Cannot close environment viewport
        if panel_id == panel_ids::ENVIRONMENT {
            return false;
        }

        if let Some((cell_id, _)) = self.find_panel_location(panel_id) {
            // Check if cell has multiple tabs
            let cell_has_multiple = self
                .find_cell(cell_id)
                .map(|c| c.panel_ids.len() > 1)
                .unwrap_or(false);

            if cell_has_multiple {
                // Just remove the tab
                if let Some(cell) = self.find_cell_mut(cell_id) {
                    cell.remove_panel(panel_id);
                }
            } else {
                // Cell will be empty - collapse it (sibling expands)
                // But don't collapse if it's the environment cell
                if cell_id != self.environment_cell_id {
                    self.collapse_cell(cell_id);
                }
            }
            true
        } else {
            false
        }
    }

    /// Collapses an empty cell, replacing the parent split with the sibling.
    fn collapse_cell(&mut self, cell_id: u32) {
        if let Some(replacement) = Self::collapse_cell_recursive(&mut self.root, cell_id) {
            self.root = replacement;
        }
    }

    fn collapse_cell_recursive(node: &mut TileNode, cell_id: u32) -> Option<TileNode> {
        match node {
            TileNode::Split { first, second, .. } => {
                // Check if first child is the target leaf
                if let TileNode::Leaf(cell) = first.as_ref() {
                    if cell.id == cell_id {
                        return Some(second.as_ref().clone());
                    }
                }
                // Check if second child is the target leaf
                if let TileNode::Leaf(cell) = second.as_ref() {
                    if cell.id == cell_id {
                        return Some(first.as_ref().clone());
                    }
                }
                // Recurse into children
                if let Some(replacement) = Self::collapse_cell_recursive(first, cell_id) {
                    **first = replacement;
                }
                if let Some(replacement) = Self::collapse_cell_recursive(second, cell_id) {
                    **second = replacement;
                }
                None
            }
            TileNode::Leaf(_) => None,
        }
    }

    /// Moves a panel to a different cell as a new tab.
    pub fn move_panel_to_cell(&mut self, panel_id: &str, target_cell_id: u32) -> bool {
        // Find source location
        let source_info = self.find_panel_location(panel_id);
        if source_info.is_none() {
            return false;
        }
        let (source_cell_id, _) = source_info.unwrap();

        // Don't move to same cell
        if source_cell_id == target_cell_id {
            return false;
        }

        // Check if source cell will be empty after removal
        let source_will_be_empty = self
            .find_cell(source_cell_id)
            .map(|c| c.panel_ids.len() == 1)
            .unwrap_or(false);

        // Update environment tracking if moving environment
        if panel_id == panel_ids::ENVIRONMENT {
            self.environment_cell_id = target_cell_id;
        }

        // Remove from source
        if let Some(cell) = self.find_cell_mut(source_cell_id) {
            cell.remove_panel(panel_id);
        }

        // Add to target
        if let Some(cell) = self.find_cell_mut(target_cell_id) {
            cell.add_panel(panel_id.to_string());
        }

        // Collapse source if it's now empty (and not the environment cell)
        if source_will_be_empty && source_cell_id != self.environment_cell_id {
            self.collapse_cell(source_cell_id);
        }

        true
    }

    /// Adds a panel as a new tab in an existing cell.
    pub fn add_tab(&mut self, cell_id: u32, panel_id: String) -> bool {
        if let Some(cell) = self.find_cell_mut(cell_id) {
            if !cell.contains_panel(&panel_id) {
                cell.add_panel(panel_id);
                return true;
            }
        }
        false
    }

    /// Undocks a panel from the tiled layout to a separate OS window.
    pub fn undock_panel(&mut self, panel_id: &str) -> bool {
        // Cannot undock environment through this method
        if panel_id == panel_ids::ENVIRONMENT {
            return false;
        }

        // Close the panel (removes from tree)
        if self.close_panel(panel_id) {
            // Add to undocked list
            if !self.undocked_panels.contains(&panel_id.to_string()) {
                self.undocked_panels.push(panel_id.to_string());
            }
            true
        } else {
            false
        }
    }

    /// Redocks a panel back into the tiled layout.
    pub fn redock_panel(&mut self, panel_id: &str, target_cell_id: Option<u32>) -> bool {
        // Remove from undocked list
        self.undocked_panels.retain(|id| id != panel_id);

        if let Some(cell_id) = target_cell_id {
            // Add to specified cell as tab
            self.add_tab(cell_id, panel_id.to_string())
        } else {
            // Add as new split at root (right side)
            self.split_at_root(SplitDirection::Horizontal, panel_id.to_string(), false);
            true
        }
    }

    /// Splits at the root level to add a new panel.
    fn split_at_root(&mut self, direction: SplitDirection, panel_id: String, new_first: bool) {
        let new_cell_id = self.alloc_cell_id();
        let new_cell = TabbedCell::new(new_cell_id, panel_id);
        let new_leaf = TileNode::Leaf(new_cell);

        let old_root = std::mem::replace(
            &mut self.root,
            TileNode::Leaf(TabbedCell::new(0, String::new())),
        );

        let (first, second) = if new_first {
            (Box::new(new_leaf), Box::new(old_root))
        } else {
            (Box::new(old_root), Box::new(new_leaf))
        };

        self.root = TileNode::Split {
            direction,
            ratio: 0.5,
            first,
            second,
        };
    }

    /// Handles a drop operation, either adding as tab or creating a split.
    pub fn handle_drop(&mut self, panel_id: &str, target: DropTarget) -> bool {
        match target {
            DropTarget::Tab { cell_id } => {
                // First remove from current location if in layout
                if let Some((source_cell_id, _)) = self.find_panel_location(panel_id) {
                    if source_cell_id != cell_id {
                        return self.move_panel_to_cell(panel_id, cell_id);
                    }
                    return false; // Already in target cell
                }
                // Panel was undocked, redock as tab
                self.redock_panel(panel_id, Some(cell_id))
            }
            DropTarget::Split {
                cell_id,
                direction,
                new_first,
            } => {
                // Remove from current location first
                let was_in_layout = if let Some((source_cell_id, _)) = self.find_panel_location(panel_id) {
                    let source_will_be_empty = self
                        .find_cell(source_cell_id)
                        .map(|c| c.panel_ids.len() == 1)
                        .unwrap_or(false);

                    if let Some(cell) = self.find_cell_mut(source_cell_id) {
                        cell.remove_panel(panel_id);
                    }

                    if source_will_be_empty && source_cell_id != self.environment_cell_id {
                        self.collapse_cell(source_cell_id);
                    }
                    true
                } else {
                    // Remove from undocked
                    self.undocked_panels.retain(|id| id != panel_id);
                    false
                };

                // Now create the split
                self.split_cell(cell_id, direction, panel_id.to_string(), new_first)
                    .is_some()
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_layout() {
        let layout = TiledLayout::new();
        assert!(layout.is_panel_in_layout(panel_ids::ENVIRONMENT));
        assert_eq!(layout.all_panel_ids().len(), 1);
    }

    #[test]
    fn test_default_layout() {
        let layout = TiledLayout::default_layout();
        assert!(layout.is_panel_in_layout(panel_ids::ENVIRONMENT));
        assert!(layout.is_panel_in_layout(panel_ids::PROPERTIES));
        assert!(layout.is_panel_in_layout(panel_ids::CLIP_GRID));
        assert_eq!(layout.all_panel_ids().len(), 6);
    }

    #[test]
    fn test_split_cell() {
        let mut layout = TiledLayout::new();
        let new_cell = layout.split_cell(0, SplitDirection::Horizontal, "properties".to_string(), false);
        assert!(new_cell.is_some());
        assert!(layout.is_panel_in_layout("properties"));
        assert!(layout.is_panel_in_layout(panel_ids::ENVIRONMENT));
    }

    #[test]
    fn test_close_panel() {
        let mut layout = TiledLayout::default_layout();
        assert!(layout.close_panel(panel_ids::PROPERTIES));
        assert!(!layout.is_panel_in_layout(panel_ids::PROPERTIES));
    }

    #[test]
    fn test_cannot_close_environment() {
        let mut layout = TiledLayout::new();
        assert!(!layout.close_panel(panel_ids::ENVIRONMENT));
        assert!(layout.is_panel_in_layout(panel_ids::ENVIRONMENT));
    }

    #[test]
    fn test_add_tab() {
        let mut layout = TiledLayout::new();
        assert!(layout.add_tab(0, "properties".to_string()));
        let cell = layout.find_cell(0).unwrap();
        assert_eq!(cell.panel_ids.len(), 2);
    }

    #[test]
    fn test_compute_layout() {
        let layout = TiledLayout::default_layout();
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 800.0));
        let computed = layout.compute_layout(rect);

        // Should have 6 cells
        assert_eq!(computed.cell_rects.len(), 6);
        // Should have 5 dividers (one less than cells in binary tree)
        assert_eq!(computed.dividers.len(), 5);
        // All panels should be located
        assert_eq!(computed.panel_locations.len(), 6);
    }
}
