//! Masking for output regions
//!
//! Provides soft and hard masks for hiding/revealing output regions.

#![allow(dead_code)]

use glam::Vec2;
use serde::{Deserialize, Serialize};

/// Mask type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MaskType {
    /// Hard edge mask (binary)
    #[default]
    Hard,
    /// Soft edge mask (gradient falloff)
    Soft,
}

/// A mask defines alpha transparency regions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mask {
    /// Mask type
    pub mask_type: MaskType,
    /// Whether the mask is inverted
    pub inverted: bool,
    /// Feather/blur amount for soft edges (0.0 to 1.0)
    pub feather: f32,
    /// Mask shape
    pub shape: MaskShape,
}

/// Shape of a mask
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MaskShape {
    /// Rectangular mask
    Rectangle(MaskRectangle),
    /// Elliptical mask
    Ellipse(MaskEllipse),
    /// Polygon mask
    Polygon(MaskPolygon),
    /// Gradient mask
    Gradient(MaskGradient),
}

impl Default for Mask {
    fn default() -> Self {
        Self {
            mask_type: MaskType::Hard,
            inverted: false,
            feather: 0.0,
            shape: MaskShape::Rectangle(MaskRectangle::default()),
        }
    }
}

impl Mask {
    /// Create a rectangular mask
    pub fn rectangle(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            shape: MaskShape::Rectangle(MaskRectangle {
                position: Vec2::new(x, y),
                size: Vec2::new(width, height),
                corner_radius: 0.0,
            }),
            ..Default::default()
        }
    }

    /// Create an elliptical mask
    pub fn ellipse(center_x: f32, center_y: f32, radius_x: f32, radius_y: f32) -> Self {
        Self {
            shape: MaskShape::Ellipse(MaskEllipse {
                center: Vec2::new(center_x, center_y),
                radius: Vec2::new(radius_x, radius_y),
            }),
            ..Default::default()
        }
    }

    /// Create a polygon mask
    pub fn polygon(points: Vec<Vec2>) -> Self {
        Self {
            shape: MaskShape::Polygon(MaskPolygon { points }),
            ..Default::default()
        }
    }

    /// Create a gradient mask
    pub fn gradient(start: Vec2, end: Vec2) -> Self {
        Self {
            mask_type: MaskType::Soft,
            shape: MaskShape::Gradient(MaskGradient {
                start,
                end,
                gradient_type: GradientType::Linear,
            }),
            ..Default::default()
        }
    }

    /// Set the feather amount
    pub fn with_feather(mut self, feather: f32) -> Self {
        self.feather = feather.clamp(0.0, 1.0);
        self.mask_type = if feather > 0.0 { MaskType::Soft } else { MaskType::Hard };
        self
    }

    /// Invert the mask
    pub fn inverted(mut self) -> Self {
        self.inverted = true;
        self
    }

    /// Sample the mask alpha at a given point (0.0 = fully masked, 1.0 = fully visible)
    pub fn sample(&self, point: Vec2) -> f32 {
        let base_alpha = match &self.shape {
            MaskShape::Rectangle(rect) => rect.sample(point, self.feather),
            MaskShape::Ellipse(ellipse) => ellipse.sample(point, self.feather),
            MaskShape::Polygon(polygon) => polygon.sample(point, self.feather),
            MaskShape::Gradient(gradient) => gradient.sample(point),
        };

        if self.inverted {
            1.0 - base_alpha
        } else {
            base_alpha
        }
    }
}

/// Rectangular mask shape
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskRectangle {
    /// Top-left position (normalized 0-1)
    pub position: Vec2,
    /// Size (normalized 0-1)
    pub size: Vec2,
    /// Corner radius for rounded rectangles
    pub corner_radius: f32,
}

impl Default for MaskRectangle {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            size: Vec2::ONE,
            corner_radius: 0.0,
        }
    }
}

impl MaskRectangle {
    fn sample(&self, point: Vec2, feather: f32) -> f32 {
        // Check if point is inside rectangle
        let local = point - self.position;
        
        if local.x < 0.0 || local.x > self.size.x || local.y < 0.0 || local.y > self.size.y {
            if feather > 0.0 {
                // Calculate distance to edge for feathering
                let dx = if local.x < 0.0 {
                    -local.x
                } else if local.x > self.size.x {
                    local.x - self.size.x
                } else {
                    0.0
                };
                
                let dy = if local.y < 0.0 {
                    -local.y
                } else if local.y > self.size.y {
                    local.y - self.size.y
                } else {
                    0.0
                };
                
                let dist = (dx * dx + dy * dy).sqrt();
                (1.0 - dist / feather).max(0.0)
            } else {
                0.0
            }
        } else {
            if feather > 0.0 {
                // Inside - check distance to edge for inner feather
                let edge_dist = local.x.min(local.y).min(self.size.x - local.x).min(self.size.y - local.y);
                (edge_dist / feather).min(1.0)
            } else {
                1.0
            }
        }
    }
}

/// Elliptical mask shape
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskEllipse {
    /// Center position (normalized 0-1)
    pub center: Vec2,
    /// Radius in x and y (normalized)
    pub radius: Vec2,
}

impl MaskEllipse {
    fn sample(&self, point: Vec2, feather: f32) -> f32 {
        let local = point - self.center;
        let normalized = Vec2::new(local.x / self.radius.x, local.y / self.radius.y);
        let dist = normalized.length();
        
        if feather > 0.0 {
            (1.0 - (dist - 1.0 + feather) / feather).clamp(0.0, 1.0)
        } else {
            if dist <= 1.0 { 1.0 } else { 0.0 }
        }
    }
}

/// Polygon mask shape
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskPolygon {
    /// Polygon vertices (normalized 0-1)
    pub points: Vec<Vec2>,
}

impl MaskPolygon {
    fn sample(&self, point: Vec2, _feather: f32) -> f32 {
        // Ray casting algorithm for point-in-polygon
        if self.points.len() < 3 {
            return 0.0;
        }

        let mut inside = false;
        let mut j = self.points.len() - 1;
        
        for i in 0..self.points.len() {
            let pi = self.points[i];
            let pj = self.points[j];
            
            if ((pi.y > point.y) != (pj.y > point.y))
                && (point.x < (pj.x - pi.x) * (point.y - pi.y) / (pj.y - pi.y) + pi.x)
            {
                inside = !inside;
            }
            j = i;
        }
        
        if inside { 1.0 } else { 0.0 }
    }
}

/// Gradient mask
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskGradient {
    /// Gradient start point
    pub start: Vec2,
    /// Gradient end point
    pub end: Vec2,
    /// Gradient type
    pub gradient_type: GradientType,
}

/// Gradient type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GradientType {
    #[default]
    Linear,
    Radial,
}

impl MaskGradient {
    fn sample(&self, point: Vec2) -> f32 {
        match self.gradient_type {
            GradientType::Linear => {
                let dir = self.end - self.start;
                let len_sq = dir.length_squared();
                if len_sq < 0.0001 {
                    return 0.5;
                }
                let t = (point - self.start).dot(dir) / len_sq;
                t.clamp(0.0, 1.0)
            }
            GradientType::Radial => {
                let center = (self.start + self.end) / 2.0;
                let radius = (self.end - self.start).length() / 2.0;
                if radius < 0.0001 {
                    return 1.0;
                }
                let dist = (point - center).length();
                (1.0 - dist / radius).clamp(0.0, 1.0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rectangle_mask() {
        let mask = Mask::rectangle(0.25, 0.25, 0.5, 0.5);
        
        // Center should be visible
        assert_eq!(mask.sample(Vec2::new(0.5, 0.5)), 1.0);
        
        // Outside should be hidden
        assert_eq!(mask.sample(Vec2::new(0.0, 0.0)), 0.0);
        assert_eq!(mask.sample(Vec2::new(1.0, 1.0)), 0.0);
    }

    #[test]
    fn test_ellipse_mask() {
        let mask = Mask::ellipse(0.5, 0.5, 0.25, 0.25);
        
        // Center should be visible
        assert_eq!(mask.sample(Vec2::new(0.5, 0.5)), 1.0);
        
        // Far corners should be hidden
        assert_eq!(mask.sample(Vec2::new(0.0, 0.0)), 0.0);
    }
}



