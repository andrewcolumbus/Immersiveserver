//! Warping for geometric correction
//!
//! Provides perspective and bezier warping for projection surface alignment.

#![allow(dead_code)]

use glam::Vec2;
use serde::{Deserialize, Serialize};

/// Warping mode for a slice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WarpMode {
    /// No warping applied
    None,
    /// 4-corner perspective warp
    Perspective(PerspectiveWarp),
    /// Bezier grid warp
    Bezier(BezierWarp),
}

impl Default for WarpMode {
    fn default() -> Self {
        WarpMode::None
    }
}

/// 4-corner perspective warp
///
/// Simple quad-to-quad transformation for flat surfaces at angles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerspectiveWarp {
    /// Corner positions: [top-left, top-right, bottom-right, bottom-left]
    pub corners: [Vec2; 4],
}

impl Default for PerspectiveWarp {
    fn default() -> Self {
        Self::identity()
    }
}

impl PerspectiveWarp {
    /// Create an identity warp (no transformation)
    pub fn identity() -> Self {
        Self {
            corners: [
                Vec2::new(0.0, 0.0),  // top-left
                Vec2::new(1.0, 0.0),  // top-right
                Vec2::new(1.0, 1.0),  // bottom-right
                Vec2::new(0.0, 1.0),  // bottom-left
            ],
        }
    }

    /// Create from pixel coordinates
    pub fn from_pixels(corners: [Vec2; 4], width: f32, height: f32) -> Self {
        Self {
            corners: corners.map(|c| Vec2::new(c.x / width, c.y / height)),
        }
    }

    /// Get corner by index
    pub fn corner(&self, index: usize) -> Vec2 {
        self.corners[index.min(3)]
    }

    /// Set corner by index
    pub fn set_corner(&mut self, index: usize, position: Vec2) {
        if index < 4 {
            self.corners[index] = position;
        }
    }

    /// Reset to identity
    pub fn reset(&mut self) {
        *self = Self::identity();
    }

    /// Check if this is approximately an identity transform
    pub fn is_identity(&self, epsilon: f32) -> bool {
        let identity = Self::identity();
        for i in 0..4 {
            if (self.corners[i] - identity.corners[i]).length() > epsilon {
                return false;
            }
        }
        true
    }

    /// Calculate the homography matrix for this perspective transform
    pub fn homography_matrix(&self) -> [[f32; 3]; 3] {
        // Source corners (unit square)
        let src = [
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        ];
        
        // Compute homography using DLT algorithm (simplified)
        compute_homography(&src, &self.corners)
    }

    /// Transform a point using this perspective warp
    pub fn transform_point(&self, point: Vec2) -> Vec2 {
        let h = self.homography_matrix();
        let x = h[0][0] * point.x + h[0][1] * point.y + h[0][2];
        let y = h[1][0] * point.x + h[1][1] * point.y + h[1][2];
        let w = h[2][0] * point.x + h[2][1] * point.y + h[2][2];
        Vec2::new(x / w, y / w)
    }
}

/// Compute a 3x3 homography matrix from 4 point correspondences
fn compute_homography(_src: &[Vec2; 4], _dst: &[Vec2; 4]) -> [[f32; 3]; 3] {
    // Simplified projective mapping for quad-to-quad
    // Uses bilinear interpolation approach
    
    // For a proper implementation, we'd solve the 8x8 linear system
    // For now, return an identity-ish transform
    [
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ]
}

/// Bezier grid warp for smooth surface correction
///
/// Uses a grid of control points with bezier interpolation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BezierWarp {
    /// Grid dimensions (width x height in control points)
    pub grid_size: (u32, u32),
    /// Control points (row-major order)
    pub control_points: Vec<Vec2>,
    /// Subdivision level for rendering
    pub subdivision: u32,
}

impl Default for BezierWarp {
    fn default() -> Self {
        Self::new((4, 4))
    }
}

impl BezierWarp {
    /// Create a new bezier warp with the specified grid size
    pub fn new(grid_size: (u32, u32)) -> Self {
        let mut points = Vec::with_capacity((grid_size.0 * grid_size.1) as usize);
        
        for y in 0..grid_size.1 {
            for x in 0..grid_size.0 {
                let u = x as f32 / (grid_size.0 - 1) as f32;
                let v = y as f32 / (grid_size.1 - 1) as f32;
                points.push(Vec2::new(u, v));
            }
        }
        
        Self {
            grid_size,
            control_points: points,
            subdivision: 8,
        }
    }

    /// Get a control point at the specified grid position
    pub fn get_point(&self, x: usize, y: usize) -> Vec2 {
        let idx = y * self.grid_size.0 as usize + x;
        self.control_points.get(idx).copied().unwrap_or(Vec2::ZERO)
    }

    /// Set a control point at the specified grid position
    pub fn set_point(&mut self, x: usize, y: usize, position: Vec2) {
        let idx = y * self.grid_size.0 as usize + x;
        if idx < self.control_points.len() {
            self.control_points[idx] = position;
        }
    }

    /// Move a control point by a delta
    pub fn move_point(&mut self, x: usize, y: usize, delta: Vec2) {
        let idx = y * self.grid_size.0 as usize + x;
        if idx < self.control_points.len() {
            self.control_points[idx] += delta;
        }
    }

    /// Reset all points to their default positions
    pub fn reset(&mut self) {
        *self = Self::new(self.grid_size);
    }

    /// Get the number of control points
    pub fn point_count(&self) -> usize {
        self.control_points.len()
    }

    /// Sample the warped position at a UV coordinate
    pub fn sample(&self, u: f32, v: f32) -> Vec2 {
        // Bicubic bezier interpolation
        let fx = u * (self.grid_size.0 - 1) as f32;
        let fy = v * (self.grid_size.1 - 1) as f32;
        
        let ix = fx.floor() as usize;
        let iy = fy.floor() as usize;
        
        let tx = fx.fract();
        let ty = fy.fract();
        
        // Bilinear interpolation between grid points
        let p00 = self.get_point(ix, iy);
        let p10 = self.get_point((ix + 1).min(self.grid_size.0 as usize - 1), iy);
        let p01 = self.get_point(ix, (iy + 1).min(self.grid_size.1 as usize - 1));
        let p11 = self.get_point(
            (ix + 1).min(self.grid_size.0 as usize - 1),
            (iy + 1).min(self.grid_size.1 as usize - 1),
        );
        
        let top = p00.lerp(p10, tx);
        let bottom = p01.lerp(p11, tx);
        top.lerp(bottom, ty)
    }

    /// Generate mesh vertices for rendering
    pub fn generate_mesh(&self) -> Vec<WarpVertex> {
        let mut vertices = Vec::new();
        let steps = self.subdivision as usize;
        
        for sy in 0..steps {
            for sx in 0..steps {
                let u0 = sx as f32 / steps as f32;
                let v0 = sy as f32 / steps as f32;
                let u1 = (sx + 1) as f32 / steps as f32;
                let v1 = (sy + 1) as f32 / steps as f32;
                
                // Create two triangles for this quad
                let p00 = self.sample(u0, v0);
                let p10 = self.sample(u1, v0);
                let p01 = self.sample(u0, v1);
                let p11 = self.sample(u1, v1);
                
                // Triangle 1
                vertices.push(WarpVertex { position: p00, uv: Vec2::new(u0, v0) });
                vertices.push(WarpVertex { position: p10, uv: Vec2::new(u1, v0) });
                vertices.push(WarpVertex { position: p01, uv: Vec2::new(u0, v1) });
                
                // Triangle 2
                vertices.push(WarpVertex { position: p10, uv: Vec2::new(u1, v0) });
                vertices.push(WarpVertex { position: p11, uv: Vec2::new(u1, v1) });
                vertices.push(WarpVertex { position: p01, uv: Vec2::new(u0, v1) });
            }
        }
        
        vertices
    }
}

/// Vertex for warp mesh rendering
#[derive(Debug, Clone, Copy)]
pub struct WarpVertex {
    pub position: Vec2,
    pub uv: Vec2,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perspective_identity() {
        let warp = PerspectiveWarp::identity();
        assert!(warp.is_identity(0.001));
    }

    #[test]
    fn test_bezier_sample() {
        let warp = BezierWarp::new((4, 4));
        
        // Center should be at 0.5, 0.5
        let center = warp.sample(0.5, 0.5);
        assert!((center.x - 0.5).abs() < 0.01);
        assert!((center.y - 0.5).abs() < 0.01);
        
        // Corners should be at corners
        let tl = warp.sample(0.0, 0.0);
        assert!((tl.x - 0.0).abs() < 0.01);
        assert!((tl.y - 0.0).abs() < 0.01);
    }
}

