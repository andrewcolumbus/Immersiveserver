//! Slice definition for screen regions
//!
//! A Slice defines a region within a Screen that samples from the composition.

#![allow(dead_code)]

use super::{Mask, Quad, Rect, WarpMode};
use glam::Vec2;
use serde::{Deserialize, Serialize};

/// Input source for a slice
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SliceSource {
    /// Main composition output
    #[default]
    Composition,
    /// Specific layer by index
    Layer(usize),
    /// Layer group by index
    Group(usize),
    /// Preview output
    Preview,
    /// Another screen's output
    Screen(u32),
}

/// A slice is a region within a screen that samples from a source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slice {
    /// Unique identifier within the screen
    pub id: u32,
    /// Display name
    pub name: String,
    /// What part of the source to sample
    pub input_rect: Rect,
    /// Where to render on screen (can be warped)
    pub output_quad: Quad,
    /// Source to sample from
    pub source: SliceSource,
    /// Warping mode
    pub warp_mode: WarpMode,
    /// Optional mask
    pub mask: Option<Mask>,
    /// Opacity (0.0 to 1.0)
    pub opacity: f32,
    /// Whether this slice is enabled
    pub enabled: bool,
    /// Z-order for overlapping slices
    pub z_order: i32,
}

impl Slice {
    /// Create a new slice
    pub fn new(name: String, input_rect: Rect, output_quad: Quad) -> Self {
        Self {
            id: 0,
            name,
            input_rect,
            output_quad,
            source: SliceSource::Composition,
            warp_mode: WarpMode::None,
            mask: None,
            opacity: 1.0,
            enabled: true,
            z_order: 0,
        }
    }

    /// Create a full-screen slice
    pub fn full_screen(width: u32, height: u32) -> Self {
        let rect = Rect::from_size(width as f32, height as f32);
        let quad = Quad::from_rect(Rect::from_size(width as f32, height as f32));
        Self::new("Full Screen".to_string(), rect, quad)
    }

    /// Create a slice with normalized coordinates (0-1 range)
    pub fn normalized(name: String) -> Self {
        let rect = Rect::from_size(1.0, 1.0);
        let quad = Quad::unit();
        Self::new(name, rect, quad)
    }

    /// Set perspective warp with corner offsets
    pub fn set_perspective_warp(&mut self, corners: [Vec2; 4]) {
        self.warp_mode = WarpMode::Perspective(super::warp::PerspectiveWarp { corners });
    }

    /// Set bezier warp with grid
    pub fn set_bezier_warp(&mut self, grid_size: (u32, u32)) {
        self.warp_mode = WarpMode::Bezier(super::warp::BezierWarp::new(grid_size));
    }

    /// Reset warping
    pub fn reset_warp(&mut self) {
        self.warp_mode = WarpMode::None;
        self.output_quad = Quad::from_rect(self.input_rect);
    }

    /// Check if this slice has warping applied
    pub fn is_warped(&self) -> bool {
        !matches!(self.warp_mode, WarpMode::None)
    }

    /// Check if this slice has a mask
    pub fn is_masked(&self) -> bool {
        self.mask.is_some()
    }

    /// Get the effective output vertices considering warping
    pub fn get_output_vertices(&self) -> [Vec2; 4] {
        match &self.warp_mode {
            WarpMode::None => self.output_quad.corners(),
            WarpMode::Perspective(warp) => warp.corners,
            WarpMode::Bezier(warp) => {
                // For bezier, return the corner control points
                [
                    warp.get_point(0, 0),
                    warp.get_point(warp.grid_size.0 as usize - 1, 0),
                    warp.get_point(warp.grid_size.0 as usize - 1, warp.grid_size.1 as usize - 1),
                    warp.get_point(0, warp.grid_size.1 as usize - 1),
                ]
            }
        }
    }
}

impl Default for Slice {
    fn default() -> Self {
        Self::normalized("Slice".to_string())
    }
}



