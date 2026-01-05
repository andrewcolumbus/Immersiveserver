//! Output masking definitions for complex projection surfaces
//!
//! Supports bezier curves, polygons, rectangles, and ellipses for masking output regions.

use serde::{Deserialize, Serialize};

/// A 2D point for mask vertices
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2D {
    /// X coordinate (normalized 0.0-1.0)
    pub x: f32,
    /// Y coordinate (normalized 0.0-1.0)
    pub y: f32,
}

impl Default for Point2D {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

impl Point2D {
    /// Create a new point
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Distance to another point
    pub fn distance(&self, other: &Point2D) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Linear interpolation to another point
    pub fn lerp(&self, other: &Point2D, t: f32) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
        }
    }
}

impl From<(f32, f32)> for Point2D {
    fn from((x, y): (f32, f32)) -> Self {
        Self { x, y }
    }
}

impl From<[f32; 2]> for Point2D {
    fn from([x, y]: [f32; 2]) -> Self {
        Self { x, y }
    }
}

/// A bezier curve segment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BezierSegment {
    /// Start point
    #[serde(rename = "start")]
    pub start: Point2D,

    /// First control point (affects curve leaving start)
    #[serde(rename = "control1")]
    pub control1: Point2D,

    /// Second control point (affects curve entering end)
    #[serde(rename = "control2")]
    pub control2: Point2D,

    /// End point
    #[serde(rename = "end")]
    pub end: Point2D,
}

impl Default for BezierSegment {
    fn default() -> Self {
        Self {
            start: Point2D::new(0.0, 0.0),
            control1: Point2D::new(0.33, 0.0),
            control2: Point2D::new(0.66, 0.0),
            end: Point2D::new(1.0, 0.0),
        }
    }
}

impl BezierSegment {
    /// Create a new bezier segment
    pub fn new(start: Point2D, control1: Point2D, control2: Point2D, end: Point2D) -> Self {
        Self {
            start,
            control1,
            control2,
            end,
        }
    }

    /// Create a straight line segment (control points on the line)
    pub fn line(start: Point2D, end: Point2D) -> Self {
        let control1 = start.lerp(&end, 0.33);
        let control2 = start.lerp(&end, 0.66);
        Self {
            start,
            control1,
            control2,
            end,
        }
    }

    /// Evaluate the bezier curve at parameter t (0.0-1.0)
    pub fn evaluate(&self, t: f32) -> Point2D {
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;

        Point2D {
            x: mt3 * self.start.x
                + 3.0 * mt2 * t * self.control1.x
                + 3.0 * mt * t2 * self.control2.x
                + t3 * self.end.x,
            y: mt3 * self.start.y
                + 3.0 * mt2 * t * self.control1.y
                + 3.0 * mt * t2 * self.control2.y
                + t3 * self.end.y,
        }
    }
}

/// Mask shape type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MaskShape {
    /// Polygon mask (straight edges)
    #[serde(rename = "Polygon")]
    Polygon {
        /// Vertices in order (clockwise for solid, counter-clockwise for hole)
        #[serde(rename = "points")]
        points: Vec<Point2D>,
    },

    /// Bezier curve mask (smooth edges)
    #[serde(rename = "Bezier")]
    Bezier {
        /// Bezier curve segments forming a closed path
        #[serde(rename = "segments")]
        segments: Vec<BezierSegment>,
    },

    /// Rectangle mask
    #[serde(rename = "Rectangle")]
    Rectangle {
        /// Rectangle bounds (normalized coordinates)
        #[serde(rename = "x")]
        x: f32,
        #[serde(rename = "y")]
        y: f32,
        #[serde(rename = "width")]
        width: f32,
        #[serde(rename = "height")]
        height: f32,
    },

    /// Ellipse mask
    #[serde(rename = "Ellipse")]
    Ellipse {
        /// Center point
        #[serde(rename = "center")]
        center: Point2D,
        /// Horizontal radius (0.0-1.0)
        #[serde(rename = "radiusX")]
        radius_x: f32,
        /// Vertical radius (0.0-1.0)
        #[serde(rename = "radiusY")]
        radius_y: f32,
    },
}

impl Default for MaskShape {
    fn default() -> Self {
        // Default to full rectangle
        Self::Rectangle {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        }
    }
}

impl MaskShape {
    /// Create a default polygon (triangle)
    pub fn default_polygon() -> Self {
        Self::Polygon {
            points: vec![
                Point2D::new(0.5, 0.1),
                Point2D::new(0.9, 0.9),
                Point2D::new(0.1, 0.9),
            ],
        }
    }

    /// Create a centered ellipse
    pub fn centered_ellipse(radius_x: f32, radius_y: f32) -> Self {
        Self::Ellipse {
            center: Point2D::new(0.5, 0.5),
            radius_x,
            radius_y,
        }
    }

    /// Create a centered circle
    pub fn centered_circle(radius: f32) -> Self {
        Self::centered_ellipse(radius, radius)
    }

    /// Get display name for this shape type
    pub fn type_name(&self) -> &'static str {
        match self {
            MaskShape::Polygon { .. } => "Polygon",
            MaskShape::Bezier { .. } => "Bezier",
            MaskShape::Rectangle { .. } => "Rectangle",
            MaskShape::Ellipse { .. } => "Ellipse",
        }
    }
}

/// Slice mask configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SliceMask {
    /// The mask shape
    #[serde(rename = "shape")]
    pub shape: MaskShape,

    /// Edge feather/softness in normalized units (0.0-0.1 typical)
    #[serde(rename = "feather")]
    pub feather: f32,

    /// Whether to invert the mask (show outside instead of inside)
    #[serde(rename = "inverted")]
    pub inverted: bool,

    /// Whether this mask is enabled
    #[serde(rename = "enabled")]
    pub enabled: bool,
}

impl Default for SliceMask {
    fn default() -> Self {
        Self {
            shape: MaskShape::default(),
            feather: 0.0,
            inverted: false,
            enabled: true,
        }
    }
}

impl SliceMask {
    /// Create a new mask with the given shape
    pub fn new(shape: MaskShape) -> Self {
        Self {
            shape,
            feather: 0.0,
            inverted: false,
            enabled: true,
        }
    }

    /// Create a rectangular mask
    pub fn rectangle(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self::new(MaskShape::Rectangle {
            x,
            y,
            width,
            height,
        })
    }

    /// Create an ellipse mask
    pub fn ellipse(center: Point2D, radius_x: f32, radius_y: f32) -> Self {
        Self::new(MaskShape::Ellipse {
            center,
            radius_x,
            radius_y,
        })
    }

    /// Create a polygon mask from points
    pub fn polygon(points: Vec<Point2D>) -> Self {
        Self::new(MaskShape::Polygon { points })
    }

    /// Set the feather amount (clamped to reasonable range)
    pub fn set_feather(&mut self, feather: f32) {
        self.feather = feather.clamp(0.0, 0.5);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point2d() {
        let p1 = Point2D::new(0.0, 0.0);
        let p2 = Point2D::new(1.0, 0.0);
        assert!((p1.distance(&p2) - 1.0).abs() < f32::EPSILON);

        let mid = p1.lerp(&p2, 0.5);
        assert!((mid.x - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_bezier_evaluation() {
        let segment = BezierSegment::line(Point2D::new(0.0, 0.0), Point2D::new(1.0, 1.0));

        let start = segment.evaluate(0.0);
        assert!((start.x - 0.0).abs() < f32::EPSILON);

        let end = segment.evaluate(1.0);
        assert!((end.x - 1.0).abs() < f32::EPSILON);

        let mid = segment.evaluate(0.5);
        assert!((mid.x - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_mask_default() {
        let mask = SliceMask::default();
        assert!(mask.enabled);
        assert!(!mask.inverted);
        assert_eq!(mask.feather, 0.0);
    }
}
