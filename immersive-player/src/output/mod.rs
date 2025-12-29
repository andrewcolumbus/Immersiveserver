//! Output module for screen management and projection mapping
//!
//! Handles multi-display output, slices, edge blending, warping, and masking.

#![allow(dead_code)]

mod aqueduct_output;
mod blend;
mod device;
mod mask;
mod receiver_control;
mod screen;
mod slice;
mod warp;
mod window_output;

pub use blend::{BlendConfig, BlendPreset, EdgeBlend};
pub use device::{DeviceType, DisplayInfo, OutputDevice, enumerate_displays};
pub use mask::Mask;
pub use screen::Screen;
pub use slice::Slice;
pub use warp::{BezierWarp, PerspectiveWarp, WarpMode};
pub use window_output::{WindowHandle, WindowManager, WindowOutput, WindowOutputError, render_output_content};

use glam::Vec2;

/// Manages all output screens and displays
pub struct OutputManager {
    /// All configured screens
    pub screens: Vec<Screen>,
    /// Display information cache
    pub displays: Vec<DisplayInfo>,
    /// Window manager for output viewports
    pub window_manager: WindowManager,
    /// Next screen ID
    next_id: u32,
    /// Whether outputs are currently live
    pub is_live: bool,
    /// Show test pattern on all outputs
    pub show_test_pattern: bool,
}


impl Default for OutputManager {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputManager {
    /// Create a new output manager
    pub fn new() -> Self {
        Self {
            screens: Vec::new(),
            displays: Vec::new(),
            window_manager: WindowManager::new(),
            next_id: 1,
            is_live: false,
            show_test_pattern: false,
        }
    }

    /// Enumerate available displays
    pub fn enumerate_displays(&mut self) {
        // Use the platform-specific display enumeration
        self.displays = enumerate_displays();
        log::info!("Enumerated {} displays", self.displays.len());
    }

    /// Add a new screen
    pub fn add_screen(&mut self, mut screen: Screen) -> u32 {
        let id = self.next_id;
        screen.id = id;
        self.screens.push(screen);
        self.next_id += 1;
        log::info!("Added screen with ID {}", id);
        id
    }

    /// Remove a screen by ID
    pub fn remove_screen(&mut self, id: u32) -> Option<Screen> {
        if let Some(pos) = self.screens.iter().position(|s| s.id == id) {
            Some(self.screens.remove(pos))
        } else {
            None
        }
    }

    /// Get a screen by ID
    pub fn get_screen(&self, id: u32) -> Option<&Screen> {
        self.screens.iter().find(|s| s.id == id)
    }

    /// Get a screen by ID mutably
    pub fn get_screen_mut(&mut self, id: u32) -> Option<&mut Screen> {
        self.screens.iter_mut().find(|s| s.id == id)
    }

    /// Get a display by ID
    pub fn get_display(&self, id: u32) -> Option<&DisplayInfo> {
        self.displays.iter().find(|d| d.id == id)
    }

    /// Create a new screen for a display
    pub fn create_screen_for_display(&mut self, display_id: u32) -> Option<u32> {
        let display = self.displays.iter().find(|d| d.id == display_id)?.clone();
        let position = self.auto_position_screen();
        
        let screen = Screen::new_at_position(
            format!("Screen {}", self.next_id),
            display.id,
            display.resolution,
            position,
        );
        
        Some(self.add_screen(screen))
    }

    /// Create a default dual-projector setup
    pub fn create_dual_projector_setup(&mut self) {
        // Left projector - positioned at origin
        let mut left = Screen::new_at_position(
            "Left Projector".to_string(),
            0,
            (1920, 1080),
            (0.0, 0.0),
        );
        left.blend_config.right = Some(EdgeBlend {
            width: 200,
            power: 2.2,
            gamma: 1.0,
            black_level: 0.02,
        });
        left.add_slice(Slice::full_screen(1920, 1080));
        
        // Right projector - positioned to overlap blend region
        let mut right = Screen::new_at_position(
            "Right Projector".to_string(),
            1,
            (1920, 1080),
            (1720.0, 0.0), // 1920 - 200 (blend width) = overlap for blending
        );
        right.blend_config.left = Some(EdgeBlend {
            width: 200,
            power: 2.2,
            gamma: 1.0,
            black_level: 0.02,
        });
        right.add_slice(Slice::full_screen(1920, 1080));
        
        self.add_screen(left);
        self.add_screen(right);
        
        log::info!("Created dual projector setup");
    }

    /// Auto-position a new screen based on existing screens
    pub fn auto_position_screen(&self) -> (f32, f32) {
        if self.screens.is_empty() {
            return (0.0, 0.0);
        }
        
        // Find the rightmost edge of existing screens
        let max_x = self.screens.iter()
            .map(|s| s.position.0 + s.resolution.0 as f32)
            .fold(0.0f32, |a, b| a.max(b));
        
        (max_x + 100.0, 0.0) // 100px gap between screens
    }

    /// Auto-configure blending based on screen overlaps
    /// Returns the number of blend regions configured
    pub fn auto_blend(&mut self) -> usize {
        let mut blend_count = 0;
        
        // First, clear all existing blends
        for screen in &mut self.screens {
            screen.blend_config = BlendConfig::default();
        }
        
        // Get screen info for overlap detection
        let screen_info: Vec<_> = self.screens.iter().enumerate().map(|(i, s)| {
            (i, s.position, s.resolution)
        }).collect();
        
        // Check each pair of screens for overlap
        for i in 0..screen_info.len() {
            for j in (i + 1)..screen_info.len() {
                let (idx_a, pos_a, res_a) = screen_info[i];
                let (idx_b, pos_b, res_b) = screen_info[j];
                
                // Calculate bounding boxes
                let a_left = pos_a.0;
                let a_right = pos_a.0 + res_a.0 as f32;
                let a_top = pos_a.1;
                let a_bottom = pos_a.1 + res_a.1 as f32;
                
                let b_left = pos_b.0;
                let b_right = pos_b.0 + res_b.0 as f32;
                let b_top = pos_b.1;
                let b_bottom = pos_b.1 + res_b.1 as f32;
                
                // Check horizontal overlap (screens side by side)
                let h_overlap = a_right > b_left && a_left < b_right;
                let v_overlap = a_bottom > b_top && a_top < b_bottom;
                
                if h_overlap && v_overlap {
                    // Screens overlap - determine which edges
                    
                    // Check if A is to the left of B
                    if a_right > b_left && a_left < b_left {
                        // A's right edge overlaps B's left edge
                        let overlap_width = (a_right - b_left).max(0.0) as u32;
                        if overlap_width > 0 {
                            // Set right blend on screen A
                            self.screens[idx_a].blend_config.right = Some(EdgeBlend::new(overlap_width));
                            // Set left blend on screen B
                            self.screens[idx_b].blend_config.left = Some(EdgeBlend::new(overlap_width));
                            blend_count += 1;
                        }
                    }
                    
                    // Check if B is to the left of A
                    if b_right > a_left && b_left < a_left {
                        // B's right edge overlaps A's left edge
                        let overlap_width = (b_right - a_left).max(0.0) as u32;
                        if overlap_width > 0 {
                            // Set right blend on screen B
                            self.screens[idx_b].blend_config.right = Some(EdgeBlend::new(overlap_width));
                            // Set left blend on screen A
                            self.screens[idx_a].blend_config.left = Some(EdgeBlend::new(overlap_width));
                            blend_count += 1;
                        }
                    }
                    
                    // Check if A is above B
                    if a_bottom > b_top && a_top < b_top {
                        // A's bottom edge overlaps B's top edge
                        let overlap_height = (a_bottom - b_top).max(0.0) as u32;
                        if overlap_height > 0 {
                            // Set bottom blend on screen A
                            self.screens[idx_a].blend_config.bottom = Some(EdgeBlend::new(overlap_height));
                            // Set top blend on screen B
                            self.screens[idx_b].blend_config.top = Some(EdgeBlend::new(overlap_height));
                            blend_count += 1;
                        }
                    }
                    
                    // Check if B is above A
                    if b_bottom > a_top && b_top < a_top {
                        // B's bottom edge overlaps A's top edge
                        let overlap_height = (b_bottom - a_top).max(0.0) as u32;
                        if overlap_height > 0 {
                            // Set bottom blend on screen B
                            self.screens[idx_b].blend_config.bottom = Some(EdgeBlend::new(overlap_height));
                            // Set top blend on screen A
                            self.screens[idx_a].blend_config.top = Some(EdgeBlend::new(overlap_height));
                            blend_count += 1;
                        }
                    }
                }
            }
        }
        
        log::info!("Auto-blend configured {} blend regions", blend_count);
        blend_count
    }

    /// Go live - start all screen outputs
    pub fn go_live(&mut self) {
        if self.is_live {
            log::warn!("Already live");
            return;
        }

        log::info!("Going live with {} screens", self.screens.len());
        
        for screen in &self.screens {
            if screen.enabled {
                match self.window_manager.start_for_screen(screen.id, screen, &self.displays) {
                    Ok(handle) => {
                        log::info!("Started output for screen '{}' (viewport: {:?})", screen.name, handle.viewport_id);
                    }
                    Err(e) => {
                        log::error!("Failed to start output for screen '{}': {}", screen.name, e);
                    }
                }
            }
        }
        
        self.is_live = true;
    }

    /// Stop all outputs
    pub fn stop_outputs(&mut self) {
        if !self.is_live {
            return;
        }

        log::info!("Stopping all outputs");
        self.window_manager.stop_all();
        self.is_live = false;
    }

    /// Toggle live state
    pub fn toggle_live(&mut self) {
        if self.is_live {
            self.stop_outputs();
        } else {
            self.go_live();
        }
    }

    /// Start output for a specific screen
    pub fn start_screen_output(&mut self, screen_id: u32) -> Result<WindowHandle, WindowOutputError> {
        let screen = self.screens.iter().find(|s| s.id == screen_id)
            .ok_or_else(|| WindowOutputError::CreationFailed(format!("Screen {} not found", screen_id)))?
            .clone();
        
        self.window_manager.start_for_screen(screen_id, &screen, &self.displays)
    }

    /// Stop output for a specific screen
    pub fn stop_screen_output(&mut self, screen_id: u32) {
        self.window_manager.remove(screen_id);
    }
}

/// Rect structure for input regions
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    pub fn from_size(width: f32, height: f32) -> Self {
        Self { x: 0.0, y: 0.0, width, height }
    }

    pub fn center(&self) -> Vec2 {
        Vec2::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }
}

/// Quad structure for output regions (supports non-rectangular shapes)
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Quad {
    /// Top-left corner
    pub tl: Vec2,
    /// Top-right corner
    pub tr: Vec2,
    /// Bottom-right corner
    pub br: Vec2,
    /// Bottom-left corner
    pub bl: Vec2,
}

impl Default for Quad {
    fn default() -> Self {
        Self::from_rect(Rect::from_size(1.0, 1.0))
    }
}

impl Quad {
    /// Create a quad from a rectangle
    pub fn from_rect(rect: Rect) -> Self {
        Self {
            tl: Vec2::new(rect.x, rect.y),
            tr: Vec2::new(rect.x + rect.width, rect.y),
            br: Vec2::new(rect.x + rect.width, rect.y + rect.height),
            bl: Vec2::new(rect.x, rect.y + rect.height),
        }
    }

    /// Create a unit quad (0,0 to 1,1)
    pub fn unit() -> Self {
        Self {
            tl: Vec2::new(0.0, 0.0),
            tr: Vec2::new(1.0, 0.0),
            br: Vec2::new(1.0, 1.0),
            bl: Vec2::new(0.0, 1.0),
        }
    }

    /// Get the corner points as an array
    pub fn corners(&self) -> [Vec2; 4] {
        [self.tl, self.tr, self.br, self.bl]
    }

    /// Get a mutable reference to a corner by index
    pub fn corner_mut(&mut self, index: usize) -> Option<&mut Vec2> {
        match index {
            0 => Some(&mut self.tl),
            1 => Some(&mut self.tr),
            2 => Some(&mut self.br),
            3 => Some(&mut self.bl),
            _ => None,
        }
    }

    /// Get the center of the quad
    pub fn center(&self) -> Vec2 {
        (self.tl + self.tr + self.br + self.bl) / 4.0
    }

    /// Scale the quad around its center
    pub fn scale(&mut self, factor: f32) {
        let center = self.center();
        self.tl = center + (self.tl - center) * factor;
        self.tr = center + (self.tr - center) * factor;
        self.br = center + (self.br - center) * factor;
        self.bl = center + (self.bl - center) * factor;
    }
}

