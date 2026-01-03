//! Mesh generation for previs surfaces
//!
//! Generates vertex and index data for circle, walls, and dome surfaces.

use bytemuck::{Pod, Zeroable};
use std::f32::consts::{PI, TAU};

use super::types::WallSettings;

/// Vertex for 3D previs mesh
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct PrevisVertex {
    /// Position in world space
    pub position: [f32; 3],
    /// Texture coordinates
    pub uv: [f32; 2],
    /// Normal vector (for lighting)
    pub normal: [f32; 3],
    /// Texture index (0 = walls/environment, 1 = floor/layer)
    pub tex_index: u32,
}

impl PrevisVertex {
    /// Size of vertex in bytes
    pub const SIZE: u64 = std::mem::size_of::<Self>() as u64;

    /// Vertex buffer layout for wgpu
    pub fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: Self::SIZE,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // uv
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // normal
                wgpu::VertexAttribute {
                    offset: 20,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // tex_index
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}

/// Generated mesh data
pub struct PrevisMesh {
    pub vertices: Vec<PrevisVertex>,
    pub indices: Vec<u32>,
}

impl PrevisMesh {
    /// Generate flat circle on XZ plane (floor projection)
    ///
    /// Camera views from above looking down.
    /// UV maps the environment texture directly onto the circle.
    pub fn circle(radius: f32, segments: u32) -> Self {
        let segments = segments.max(8);
        let mut vertices = Vec::with_capacity(segments as usize + 1);
        let mut indices = Vec::with_capacity(segments as usize * 3);

        // Center vertex
        vertices.push(PrevisVertex {
            position: [0.0, 0.0, 0.0],
            uv: [0.5, 0.5],
            normal: [0.0, 1.0, 0.0], // Pointing up
        });

        // Edge vertices
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * TAU;
            let x = angle.cos() * radius;
            let z = angle.sin() * radius;

            // Planar UV mapping from above
            let u = (angle.cos() + 1.0) * 0.5;
            let v = (angle.sin() + 1.0) * 0.5;

            vertices.push(PrevisVertex {
                position: [x, 0.0, z],
                uv: [u, v],
                normal: [0.0, 1.0, 0.0],
            });
        }

        // Triangle fan indices (CCW winding, viewed from above)
        for i in 0..segments {
            indices.push(0);
            indices.push(i + 1);
            indices.push(((i + 1) % segments) + 1);
        }

        Self { vertices, indices }
    }

    /// Generate 4 individual axis-aligned walls (inside view - camera inside looking out)
    ///
    /// Creates a room with 4 walls (front, back, left, right) where the camera is inside.
    /// Walls are connected at the corners forming a proper room.
    /// - Front/back wall width determines the room's X dimension
    /// - Left/right wall width determines the room's Z dimension
    /// - Each wall has independent height
    /// Environment texture is divided into quadrants for each wall.
    pub fn walls_individual(
        front: &WallSettings,
        back: &WallSettings,
        left: &WallSettings,
        right: &WallSettings,
    ) -> Self {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // Room dimensions based on wall widths
        // Front/back walls span the X axis, so their width = room width
        // Left/right walls span the Z axis, so their width = room depth
        let room_half_width = (front.width.max(back.width)) / 2.0;  // X extent
        let room_half_depth = (left.width.max(right.width)) / 2.0;  // Z extent

        // Helper to add a wall quad
        let mut add_wall = |corners: [[f32; 3]; 4],
                           normal: [f32; 3],
                           uv_start: [f32; 2],
                           uv_end: [f32; 2]| {
            let base_idx = vertices.len() as u32;

            // Bottom-left
            vertices.push(PrevisVertex {
                position: corners[0],
                uv: [uv_start[0], uv_end[1]], // Bottom of this quadrant
                normal,
            });
            // Bottom-right
            vertices.push(PrevisVertex {
                position: corners[1],
                uv: [uv_end[0], uv_end[1]],
                normal,
            });
            // Top-right
            vertices.push(PrevisVertex {
                position: corners[2],
                uv: [uv_end[0], uv_start[1]], // Top of this quadrant
                normal,
            });
            // Top-left
            vertices.push(PrevisVertex {
                position: corners[3],
                uv: [uv_start[0], uv_start[1]],
                normal,
            });

            // Two triangles (CCW winding for inward-facing surfaces)
            indices.extend_from_slice(&[base_idx, base_idx + 2, base_idx + 1]);
            indices.extend_from_slice(&[base_idx, base_idx + 3, base_idx + 2]);
        };

        // Front wall (at +Z, facing -Z toward camera)
        // Spans from -room_half_width to +room_half_width on X axis
        // UV: left quarter of texture (0.0 - 0.25)
        if front.enabled {
            let h = front.height;
            add_wall(
                [
                    [-room_half_width, 0.0, room_half_depth],  // bottom-left corner
                    [room_half_width, 0.0, room_half_depth],   // bottom-right corner
                    [room_half_width, h, room_half_depth],     // top-right corner
                    [-room_half_width, h, room_half_depth],    // top-left corner
                ],
                [0.0, 0.0, -1.0], // facing inward (-Z)
                [0.0, 0.0],       // UV start
                [0.25, 1.0],      // UV end
            );
        }

        // Right wall (at +X, facing -X toward camera)
        // Spans from +room_half_depth to -room_half_depth on Z axis (connects front to back)
        // UV: second quarter (0.25 - 0.5)
        if right.enabled {
            let h = right.height;
            add_wall(
                [
                    [room_half_width, 0.0, room_half_depth],   // bottom-left (front corner)
                    [room_half_width, 0.0, -room_half_depth],  // bottom-right (back corner)
                    [room_half_width, h, -room_half_depth],    // top-right (back corner)
                    [room_half_width, h, room_half_depth],     // top-left (front corner)
                ],
                [-1.0, 0.0, 0.0], // facing inward (-X)
                [0.25, 0.0],
                [0.5, 1.0],
            );
        }

        // Back wall (at -Z, facing +Z toward camera)
        // Spans from +room_half_width to -room_half_width on X axis (connects right to left)
        // UV: third quarter (0.5 - 0.75)
        if back.enabled {
            let h = back.height;
            add_wall(
                [
                    [room_half_width, 0.0, -room_half_depth],  // bottom-left (right corner)
                    [-room_half_width, 0.0, -room_half_depth], // bottom-right (left corner)
                    [-room_half_width, h, -room_half_depth],   // top-right (left corner)
                    [room_half_width, h, -room_half_depth],    // top-left (right corner)
                ],
                [0.0, 0.0, 1.0], // facing inward (+Z)
                [0.5, 0.0],
                [0.75, 1.0],
            );
        }

        // Left wall (at -X, facing +X toward camera)
        // Spans from -room_half_depth to +room_half_depth on Z axis (connects back to front)
        // UV: fourth quarter (0.75 - 1.0)
        if left.enabled {
            let h = left.height;
            add_wall(
                [
                    [-room_half_width, 0.0, -room_half_depth], // bottom-left (back corner)
                    [-room_half_width, 0.0, room_half_depth],  // bottom-right (front corner)
                    [-room_half_width, h, room_half_depth],    // top-right (front corner)
                    [-room_half_width, h, -room_half_depth],   // top-left (back corner)
                ],
                [1.0, 0.0, 0.0], // facing inward (+X)
                [0.75, 0.0],
                [1.0, 1.0],
            );
        }

        Self { vertices, indices }
    }

    /// Generate hemisphere dome (inside view - camera at center looking up)
    ///
    /// Creates a dome where the camera is inside looking up at the surface.
    /// Uses equirectangular UV mapping for fisheye/360 content.
    pub fn dome(radius: f32, horiz_segments: u32, vert_segments: u32) -> Self {
        let horiz_segments = horiz_segments.clamp(8, 64);
        let vert_segments = vert_segments.clamp(4, 32);
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // Generate vertices using spherical coordinates
        // Hemisphere from phi=0 (horizon) to phi=PI/2 (zenith)
        for v in 0..=vert_segments {
            let phi = (v as f32 / vert_segments as f32) * (PI / 2.0);
            let y = phi.sin() * radius;
            let ring_radius = phi.cos() * radius;

            for h in 0..=horiz_segments {
                let theta = (h as f32 / horiz_segments as f32) * TAU;
                let x = theta.cos() * ring_radius;
                let z = theta.sin() * ring_radius;

                // Equirectangular UV mapping (for inside dome)
                let u = h as f32 / horiz_segments as f32;
                let v_coord = 1.0 - (v as f32 / vert_segments as f32); // Flip V for proper orientation

                // Inward-pointing normal (for inside view)
                let len = (x * x + y * y + z * z).sqrt();
                let nx = -x / len;
                let ny = -y / len;
                let nz = -z / len;

                vertices.push(PrevisVertex {
                    position: [x, y, z],
                    uv: [u, v_coord],
                    normal: [nx, ny, nz],
                });
            }
        }

        // Generate indices (reversed winding for inside view)
        for v in 0..vert_segments {
            for h in 0..horiz_segments {
                let top_left = v * (horiz_segments + 1) + h;
                let top_right = top_left + 1;
                let bottom_left = top_left + horiz_segments + 1;
                let bottom_right = bottom_left + 1;

                // CCW winding for inside view
                indices.extend_from_slice(&[top_left, top_right, bottom_left]);
                indices.extend_from_slice(&[bottom_left, top_right, bottom_right]);
            }
        }

        Self { vertices, indices }
    }

    /// Get vertex count
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Get index count
    pub fn index_count(&self) -> usize {
        self.indices.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circle_mesh() {
        let mesh = PrevisMesh::circle(5.0, 16);
        assert_eq!(mesh.vertices.len(), 17); // Center + 16 edge vertices
        assert_eq!(mesh.indices.len(), 48); // 16 triangles * 3
    }

    #[test]
    fn test_walls_mesh() {
        let wall = WallSettings::default();
        let mesh = PrevisMesh::walls_individual(&wall, &wall, &wall, &wall);
        assert_eq!(mesh.vertices.len(), 16); // 4 walls * 4 vertices
        assert_eq!(mesh.indices.len(), 24); // 4 walls * 2 triangles * 3
    }

    #[test]
    fn test_walls_individual_disabled() {
        let enabled_wall = WallSettings::default();
        let mut disabled_wall = WallSettings::default();
        disabled_wall.enabled = false;

        // Only 2 walls enabled
        let mesh = PrevisMesh::walls_individual(&enabled_wall, &disabled_wall, &enabled_wall, &disabled_wall);
        assert_eq!(mesh.vertices.len(), 8); // 2 walls * 4 vertices
        assert_eq!(mesh.indices.len(), 12); // 2 walls * 2 triangles * 3
    }

    #[test]
    fn test_dome_mesh() {
        let mesh = PrevisMesh::dome(5.0, 8, 4);
        // (vert_segments + 1) * (horiz_segments + 1) vertices
        assert_eq!(mesh.vertices.len(), 45); // 5 * 9
        // vert_segments * horiz_segments * 2 triangles * 3 indices
        assert_eq!(mesh.indices.len(), 192); // 4 * 8 * 2 * 3
    }
}
