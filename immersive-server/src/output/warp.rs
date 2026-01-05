//! Warp mesh definitions for projection mapping
//!
//! Supports grid-based surface deformation for mapping onto non-planar surfaces.

use serde::{Deserialize, Serialize};

/// Grid-based warp mesh for surface deformation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarpMesh {
    /// Number of grid columns (minimum 2)
    #[serde(rename = "columns")]
    pub columns: usize,

    /// Number of grid rows (minimum 2)
    #[serde(rename = "rows")]
    pub rows: usize,

    /// Control points stored column-major: index = col * rows + row
    #[serde(rename = "points")]
    pub points: Vec<WarpPoint>,

    /// Interpolation mode between control points
    #[serde(rename = "interpolation", default)]
    pub interpolation: WarpInterpolation,
}

impl Default for WarpMesh {
    fn default() -> Self {
        Self::new(4, 4)
    }
}

impl WarpMesh {
    /// Create a new identity warp mesh (no deformation)
    pub fn new(columns: usize, rows: usize) -> Self {
        let columns = columns.max(2);
        let rows = rows.max(2);
        let mut points = Vec::with_capacity(columns * rows);

        for col in 0..columns {
            for row in 0..rows {
                let u = col as f32 / (columns - 1) as f32;
                let v = row as f32 / (rows - 1) as f32;
                points.push(WarpPoint {
                    uv: [u, v],
                    position: [u, v],
                });
            }
        }

        Self {
            columns,
            rows,
            points,
            interpolation: WarpInterpolation::Linear,
        }
    }

    /// Create a mesh with bezier interpolation
    pub fn new_bezier(columns: usize, rows: usize) -> Self {
        let mut mesh = Self::new(columns, rows);
        mesh.interpolation = WarpInterpolation::Bezier;
        mesh
    }

    /// Get a point by grid coordinates
    pub fn get_point(&self, col: usize, row: usize) -> Option<&WarpPoint> {
        if col < self.columns && row < self.rows {
            Some(&self.points[col * self.rows + row])
        } else {
            None
        }
    }

    /// Get a mutable point by grid coordinates
    pub fn get_point_mut(&mut self, col: usize, row: usize) -> Option<&mut WarpPoint> {
        if col < self.columns && row < self.rows {
            Some(&mut self.points[col * self.rows + row])
        } else {
            None
        }
    }

    /// Set a point's warped position
    pub fn set_point_position(&mut self, col: usize, row: usize, x: f32, y: f32) {
        if let Some(point) = self.get_point_mut(col, row) {
            point.position = [x, y];
        }
    }

    /// Reset mesh to identity (no deformation)
    pub fn reset(&mut self) {
        for col in 0..self.columns {
            for row in 0..self.rows {
                let u = col as f32 / (self.columns - 1) as f32;
                let v = row as f32 / (self.rows - 1) as f32;
                self.points[col * self.rows + row].position = [u, v];
            }
        }
    }

    /// Check if the mesh has any deformation
    pub fn is_identity(&self) -> bool {
        for point in &self.points {
            let diff_u = (point.position[0] - point.uv[0]).abs();
            let diff_v = (point.position[1] - point.uv[1]).abs();
            if diff_u > f32::EPSILON || diff_v > f32::EPSILON {
                return false;
            }
        }
        true
    }

    /// Get the 4 corner points (top-left, top-right, bottom-left, bottom-right)
    pub fn corners(&self) -> [&WarpPoint; 4] {
        [
            self.get_point(0, 0).unwrap(),                             // top-left
            self.get_point(self.columns - 1, 0).unwrap(),              // top-right
            self.get_point(0, self.rows - 1).unwrap(),                 // bottom-left
            self.get_point(self.columns - 1, self.rows - 1).unwrap(),  // bottom-right
        ]
    }

    /// Resize the mesh while preserving corner positions
    pub fn resize(&mut self, new_columns: usize, new_rows: usize) {
        let new_columns = new_columns.max(2);
        let new_rows = new_rows.max(2);

        if new_columns == self.columns && new_rows == self.rows {
            return;
        }

        // Save corner positions
        let corners = [
            self.get_point(0, 0).unwrap().position,
            self.get_point(self.columns - 1, 0).unwrap().position,
            self.get_point(0, self.rows - 1).unwrap().position,
            self.get_point(self.columns - 1, self.rows - 1).unwrap().position,
        ];

        // Create new mesh
        *self = Self::new(new_columns, new_rows);

        // Restore corners
        if let Some(p) = self.get_point_mut(0, 0) {
            p.position = corners[0];
        }
        if let Some(p) = self.get_point_mut(new_columns - 1, 0) {
            p.position = corners[1];
        }
        if let Some(p) = self.get_point_mut(0, new_rows - 1) {
            p.position = corners[2];
        }
        if let Some(p) = self.get_point_mut(new_columns - 1, new_rows - 1) {
            p.position = corners[3];
        }
    }
}

/// A single control point in the warp mesh
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WarpPoint {
    /// Original grid position (normalized 0.0-1.0)
    #[serde(rename = "uv")]
    pub uv: [f32; 2],

    /// Warped position (normalized 0.0-1.0, can exceed bounds)
    #[serde(rename = "position")]
    pub position: [f32; 2],
}

impl Default for WarpPoint {
    fn default() -> Self {
        Self {
            uv: [0.0, 0.0],
            position: [0.0, 0.0],
        }
    }
}

impl WarpPoint {
    /// Create a new warp point at identity (no warp)
    pub fn new_identity(u: f32, v: f32) -> Self {
        Self {
            uv: [u, v],
            position: [u, v],
        }
    }

    /// Get the displacement from original position
    pub fn displacement(&self) -> [f32; 2] {
        [self.position[0] - self.uv[0], self.position[1] - self.uv[1]]
    }

    /// Check if this point is at identity (no displacement)
    pub fn is_identity(&self) -> bool {
        let [dx, dy] = self.displacement();
        dx.abs() < f32::EPSILON && dy.abs() < f32::EPSILON
    }
}

/// Interpolation mode for warp mesh
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WarpInterpolation {
    /// Linear interpolation between control points
    #[default]
    Linear,

    /// Bezier curve interpolation for smooth surfaces
    Bezier,
}

impl WarpInterpolation {
    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            WarpInterpolation::Linear => "Linear",
            WarpInterpolation::Bezier => "Bezier (Smooth)",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_creation() {
        let mesh = WarpMesh::new(4, 4);
        assert_eq!(mesh.columns, 4);
        assert_eq!(mesh.rows, 4);
        assert_eq!(mesh.points.len(), 16);
        assert!(mesh.is_identity());
    }

    #[test]
    fn test_mesh_point_access() {
        let mut mesh = WarpMesh::new(4, 4);

        // Get corner point
        let corner = mesh.get_point(0, 0).unwrap();
        assert_eq!(corner.uv, [0.0, 0.0]);
        assert_eq!(corner.position, [0.0, 0.0]);

        // Modify a point
        mesh.set_point_position(0, 0, 0.1, 0.1);
        assert!(!mesh.is_identity());

        let corner = mesh.get_point(0, 0).unwrap();
        assert_eq!(corner.position, [0.1, 0.1]);
    }

    #[test]
    fn test_mesh_reset() {
        let mut mesh = WarpMesh::new(4, 4);
        mesh.set_point_position(0, 0, 0.5, 0.5);
        assert!(!mesh.is_identity());

        mesh.reset();
        assert!(mesh.is_identity());
    }

    #[test]
    fn test_mesh_resize() {
        let mut mesh = WarpMesh::new(4, 4);
        mesh.set_point_position(0, 0, 0.1, 0.1);
        mesh.set_point_position(3, 3, 0.9, 0.9);

        mesh.resize(8, 8);
        assert_eq!(mesh.columns, 8);
        assert_eq!(mesh.rows, 8);

        // Corners should be preserved
        assert_eq!(mesh.get_point(0, 0).unwrap().position, [0.1, 0.1]);
        assert_eq!(mesh.get_point(7, 7).unwrap().position, [0.9, 0.9]);
    }
}
