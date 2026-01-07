//! Output runtime state for GPU resources
//!
//! This module contains the runtime GPU resources for screens and slices.
//! The Screen/Slice structs in the output module are pure data (configuration),
//! while Runtime structs hold the actual GPU resources needed for rendering.

use std::collections::HashMap;

use winit::window::WindowId;

use super::{MaskShape, OutputDevice, Screen, ScreenId, Slice, SliceId, SliceInput, SliceMask, WarpMesh};
use crate::network::NdiCapture;

/// Screen-level color correction parameters (matches shader uniform)
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ScreenParams {
    /// Color correction: brightness, contrast, gamma, saturation
    pub color_adjust: [f32; 4],
    /// RGB channel multipliers + padding
    pub color_rgb: [f32; 4],
}

impl Default for ScreenParams {
    fn default() -> Self {
        Self {
            color_adjust: [0.0, 1.0, 1.0, 1.0], // brightness=0, contrast=1, gamma=1, saturation=1
            color_rgb: [1.0, 1.0, 1.0, 0.0],    // R=1, G=1, B=1, padding
        }
    }
}

impl ScreenParams {
    /// Create identity params (no color correction)
    pub fn identity() -> Self {
        Self::default()
    }

    /// Create params from OutputColorCorrection
    pub fn from_color(color: &super::OutputColorCorrection) -> Self {
        Self {
            color_adjust: [color.brightness, color.contrast, color.gamma, color.saturation],
            color_rgb: [color.red, color.green, color.blue, 0.0],
        }
    }

    /// Check if color correction is at identity (no correction needed)
    pub fn is_identity(&self) -> bool {
        (self.color_adjust[0] - 0.0).abs() < f32::EPSILON  // brightness
            && (self.color_adjust[1] - 1.0).abs() < f32::EPSILON  // contrast
            && (self.color_adjust[2] - 1.0).abs() < f32::EPSILON  // gamma
            && (self.color_adjust[3] - 1.0).abs() < f32::EPSILON  // saturation
            && (self.color_rgb[0] - 1.0).abs() < f32::EPSILON     // red
            && (self.color_rgb[1] - 1.0).abs() < f32::EPSILON     // green
            && (self.color_rgb[2] - 1.0).abs() < f32::EPSILON     // blue
    }
}

/// Uniform buffer data for slice rendering
///
/// IMPORTANT: This struct must match the WGSL SliceParams layout exactly.
/// WGSL alignment rules: vec2 needs 8-byte alignment, vec4 needs 16-byte alignment.
/// Total size: 240 bytes
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SliceParams {
    /// Input rect (x, y, width, height) - normalized 0.0-1.0
    pub input_rect: [f32; 4],        // offset 0, size 16
    /// Output rect (x, y, width, height) - normalized 0.0-1.0
    pub output_rect: [f32; 4],       // offset 16, size 16
    /// Rotation in radians
    pub rotation: f32,               // offset 32, size 4
    /// Padding for vec2 alignment
    pub _pad0: f32,                  // offset 36, size 4
    /// Flip flags (x = horizontal, y = vertical)
    pub flip: [f32; 2],              // offset 40, size 8
    /// Opacity
    pub opacity: f32,                // offset 48, size 4
    /// Padding for vec4 alignment (need to reach offset 64)
    pub _pad1: [f32; 3],             // offset 52, size 12
    /// Color correction: brightness, contrast, gamma, saturation
    pub color_adjust: [f32; 4],      // offset 64, size 16
    /// RGB channel multipliers + padding
    pub color_rgb: [f32; 4],         // offset 80, size 16
    // --- Perspective warp fields (offset 96) ---
    /// Perspective top-left corner offset (normalized 0-1, relative to output rect)
    pub perspective_tl: [f32; 2],    // offset 96, size 8
    /// Perspective top-right corner offset
    pub perspective_tr: [f32; 2],    // offset 104, size 8
    /// Perspective bottom-right corner offset
    pub perspective_br: [f32; 2],    // offset 112, size 8
    /// Perspective bottom-left corner offset
    pub perspective_bl: [f32; 2],    // offset 120, size 8
    /// Perspective enabled flag (1.0 = enabled, 0.0 = disabled)
    pub perspective_enabled: f32,    // offset 128, size 4
    /// Padding for alignment
    pub _pad2: [f32; 3],             // offset 132, size 12
    // --- Mesh warp fields (offset 144) ---
    /// Mesh grid columns
    pub mesh_columns: u32,           // offset 144, size 4
    /// Mesh grid rows
    pub mesh_rows: u32,              // offset 148, size 4
    /// Mesh warp enabled flag (1.0 = enabled, 0.0 = disabled)
    pub mesh_enabled: f32,           // offset 152, size 4
    /// Padding for alignment
    pub _pad3: f32,                  // offset 156, size 4
    // --- Edge blend fields (offset 160) ---
    /// Edge blend left: [enabled, width, gamma, black_level]
    pub edge_left: [f32; 4],         // offset 160, size 16
    /// Edge blend right: [enabled, width, gamma, black_level]
    pub edge_right: [f32; 4],        // offset 176, size 16
    /// Edge blend top: [enabled, width, gamma, black_level]
    pub edge_top: [f32; 4],          // offset 192, size 16
    /// Edge blend bottom: [enabled, width, gamma, black_level]
    pub edge_bottom: [f32; 4],       // offset 208, size 16
    // --- Mask fields (offset 224) ---
    /// Mask enabled flag (1.0 = enabled, 0.0 = disabled)
    pub mask_enabled: f32,           // offset 224, size 4
    /// Mask inverted flag (1.0 = show outside, 0.0 = show inside)
    pub mask_inverted: f32,          // offset 228, size 4
    /// Mask feather amount (0.0-0.5)
    pub mask_feather: f32,           // offset 232, size 4
    /// Padding for alignment
    pub _pad4: f32,                  // offset 236, size 4
}                                    // Total: 240 bytes

impl Default for SliceParams {
    fn default() -> Self {
        Self {
            input_rect: [0.0, 0.0, 1.0, 1.0],
            output_rect: [0.0, 0.0, 1.0, 1.0],
            rotation: 0.0,
            _pad0: 0.0,
            flip: [0.0, 0.0],
            opacity: 1.0,
            _pad1: [0.0; 3],
            color_adjust: [0.0, 1.0, 1.0, 1.0], // brightness, contrast, gamma, saturation
            color_rgb: [1.0, 1.0, 1.0, 0.0],    // R, G, B, padding
            // Perspective defaults to identity corners (no warp)
            perspective_tl: [0.0, 0.0],
            perspective_tr: [1.0, 0.0],
            perspective_br: [1.0, 1.0],
            perspective_bl: [0.0, 1.0],
            perspective_enabled: 0.0,
            _pad2: [0.0; 3],
            // Mesh warp defaults
            mesh_columns: 0,
            mesh_rows: 0,
            mesh_enabled: 0.0,
            _pad3: 0.0,
            // Edge blend defaults (all disabled)
            edge_left: [0.0, 0.15, 2.2, 0.0],   // enabled, width, gamma, black_level
            edge_right: [0.0, 0.15, 2.2, 0.0],
            edge_top: [0.0, 0.15, 2.2, 0.0],
            edge_bottom: [0.0, 0.15, 2.2, 0.0],
            // Mask defaults (disabled)
            mask_enabled: 0.0,
            mask_inverted: 0.0,
            mask_feather: 0.0,
            _pad4: 0.0,
        }
    }
}

impl SliceParams {
    /// Create params from a Slice configuration
    pub fn from_slice(slice: &Slice) -> Self {
        let flip_h = if slice.output.flip_h { 1.0 } else { 0.0 };
        let flip_v = if slice.output.flip_v { 1.0 } else { 0.0 };

        // Extract perspective corners (default to identity if not set)
        let (perspective_tl, perspective_tr, perspective_br, perspective_bl, perspective_enabled) =
            if let Some(corners) = &slice.output.perspective {
                (
                    [corners[0].x, corners[0].y], // TL
                    [corners[1].x, corners[1].y], // TR
                    [corners[2].x, corners[2].y], // BR
                    [corners[3].x, corners[3].y], // BL
                    1.0,
                )
            } else {
                // Identity corners (no warp)
                ([0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], 0.0)
            };

        // Extract mesh warp data
        let (mesh_columns, mesh_rows, mesh_enabled) = if let Some(mesh) = &slice.output.mesh {
            (mesh.columns as u32, mesh.rows as u32, 1.0)
        } else {
            (0, 0, 0.0)
        };

        // Extract edge blend config
        let edge = &slice.output.edge_blend;
        let edge_left = [
            if edge.left.enabled { 1.0 } else { 0.0 },
            edge.left.width,
            edge.left.gamma,
            edge.left.black_level,
        ];
        let edge_right = [
            if edge.right.enabled { 1.0 } else { 0.0 },
            edge.right.width,
            edge.right.gamma,
            edge.right.black_level,
        ];
        let edge_top = [
            if edge.top.enabled { 1.0 } else { 0.0 },
            edge.top.width,
            edge.top.gamma,
            edge.top.black_level,
        ];
        let edge_bottom = [
            if edge.bottom.enabled { 1.0 } else { 0.0 },
            edge.bottom.width,
            edge.bottom.gamma,
            edge.bottom.black_level,
        ];

        Self {
            input_rect: [
                slice.input_rect.x,
                slice.input_rect.y,
                slice.input_rect.width,
                slice.input_rect.height,
            ],
            output_rect: [
                slice.output.rect.x,
                slice.output.rect.y,
                slice.output.rect.width,
                slice.output.rect.height,
            ],
            rotation: slice.output.rotation.to_radians(),
            _pad0: 0.0,
            flip: [flip_h, flip_v],
            opacity: slice.color.opacity,
            _pad1: [0.0; 3],
            color_adjust: [
                slice.color.brightness,
                slice.color.contrast,
                slice.color.gamma,
                1.0, // saturation placeholder
            ],
            color_rgb: [slice.color.red, slice.color.green, slice.color.blue, 0.0],
            perspective_tl,
            perspective_tr,
            perspective_br,
            perspective_bl,
            perspective_enabled,
            _pad2: [0.0; 3],
            mesh_columns,
            mesh_rows,
            mesh_enabled,
            _pad3: 0.0,
            edge_left,
            edge_right,
            edge_top,
            edge_bottom,
            // Extract mask config
            mask_enabled: if slice.mask.as_ref().is_some_and(|m| m.enabled) { 1.0 } else { 0.0 },
            mask_inverted: if slice.mask.as_ref().is_some_and(|m| m.inverted) { 1.0 } else { 0.0 },
            mask_feather: slice.mask.as_ref().map(|m| m.feather).unwrap_or(0.0),
            _pad4: 0.0,
        }
    }
}

/// GPU data layout for a single warp point in the storage buffer
/// Each point stores: [uv.x, uv.y, position.x, position.y]
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct WarpPointGpu {
    /// Original grid UV coordinates
    pub uv: [f32; 2],
    /// Warped position coordinates
    pub position: [f32; 2],
}

/// Runtime GPU resources for a slice
pub struct SliceRuntime {
    /// The slice ID this runtime belongs to
    pub slice_id: SliceId,

    /// Texture for slice output (rendered content)
    pub texture: wgpu::Texture,

    /// Texture view for binding
    pub texture_view: wgpu::TextureView,

    /// Bind group for rendering this slice
    pub bind_group: Option<wgpu::BindGroup>,

    /// Uniform buffer for slice parameters
    pub params_buffer: wgpu::Buffer,

    /// Storage buffer for mesh warp points (optional)
    pub warp_buffer: Option<wgpu::Buffer>,

    /// Bind group for mesh warp data (optional)
    pub warp_bind_group: Option<wgpu::BindGroup>,

    /// Mask texture (optional, CPU-rasterized)
    pub mask_texture: Option<wgpu::Texture>,

    /// Mask texture view for binding
    pub mask_texture_view: Option<wgpu::TextureView>,

    /// Bind group for mask data (optional)
    pub mask_bind_group: Option<wgpu::BindGroup>,

    /// Flag to track if mask needs re-rasterization
    pub mask_dirty: bool,

    /// Cached slice dimensions
    pub width: u32,
    pub height: u32,
}

impl SliceRuntime {
    /// Create a new slice runtime with GPU resources
    pub fn new(
        device: &wgpu::Device,
        slice_id: SliceId,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("Slice {} Texture", slice_id.0)),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("Slice {} Params", slice_id.0)),
            size: std::mem::size_of::<SliceParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            slice_id,
            texture,
            texture_view,
            bind_group: None,
            params_buffer,
            warp_buffer: None,
            warp_bind_group: None,
            mask_texture: None,
            mask_texture_view: None,
            mask_bind_group: None,
            mask_dirty: false,
            width,
            height,
        }
    }

    /// Update the params buffer with new slice configuration
    pub fn update_params(&self, queue: &wgpu::Queue, slice: &Slice) {
        let params = SliceParams::from_slice(slice);
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));
    }

    /// Update the warp buffer with mesh data
    ///
    /// Creates the buffer if it doesn't exist or if the size changed.
    /// Clears the buffer if mesh is None.
    pub fn update_warp_buffer(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: Option<&WarpMesh>,
    ) {
        match mesh {
            Some(mesh) => {
                let point_count = mesh.points.len();
                let buffer_size = (point_count * std::mem::size_of::<WarpPointGpu>()) as u64;

                // Create or recreate buffer if size changed
                let needs_new_buffer = match &self.warp_buffer {
                    Some(buf) => buf.size() != buffer_size,
                    None => true,
                };

                if needs_new_buffer {
                    self.warp_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some(&format!("Slice {} Warp Buffer", self.slice_id.0)),
                        size: buffer_size.max(16), // Minimum 16 bytes
                        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }));
                    // Bind group needs recreation
                    self.warp_bind_group = None;
                }

                // Convert mesh points to GPU format and upload
                let gpu_points: Vec<WarpPointGpu> = mesh
                    .points
                    .iter()
                    .map(|p| WarpPointGpu {
                        uv: p.uv,
                        position: p.position,
                    })
                    .collect();

                if let Some(buffer) = &self.warp_buffer {
                    queue.write_buffer(buffer, 0, bytemuck::cast_slice(&gpu_points));
                }
            }
            None => {
                // Clear warp buffer when mesh is disabled
                self.warp_buffer = None;
                self.warp_bind_group = None;
            }
        }
    }

    /// Check if mesh warp is enabled (has a warp buffer)
    pub fn has_mesh_warp(&self) -> bool {
        self.warp_buffer.is_some()
    }

    /// Check if masking is enabled (has a mask texture)
    pub fn has_mask(&self) -> bool {
        self.mask_texture.is_some()
    }

    /// Update the mask texture with rasterized mask data
    ///
    /// Creates the texture if it doesn't exist.
    /// Clears the texture if mask is None.
    pub fn update_mask_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mask: Option<&super::SliceMask>,
    ) {
        match mask {
            Some(mask) if mask.enabled => {
                // Rasterize mask to 256x256 texture
                const MASK_SIZE: u32 = 256;
                let pixels = rasterize_mask(mask, MASK_SIZE);

                // Create texture if it doesn't exist
                if self.mask_texture.is_none() {
                    let texture = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some(&format!("Slice {} Mask Texture", self.slice_id.0)),
                        size: wgpu::Extent3d {
                            width: MASK_SIZE,
                            height: MASK_SIZE,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                    self.mask_texture = Some(texture);
                    self.mask_texture_view = Some(texture_view);
                    self.mask_bind_group = None; // Needs recreation
                }

                // Upload pixel data
                if let Some(texture) = &self.mask_texture {
                    queue.write_texture(
                        wgpu::ImageCopyTexture {
                            texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        &pixels,
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(MASK_SIZE * 4),
                            rows_per_image: Some(MASK_SIZE),
                        },
                        wgpu::Extent3d {
                            width: MASK_SIZE,
                            height: MASK_SIZE,
                            depth_or_array_layers: 1,
                        },
                    );
                }

                self.mask_dirty = false;
            }
            _ => {
                // Clear mask texture when disabled
                self.mask_texture = None;
                self.mask_texture_view = None;
                self.mask_bind_group = None;
                self.mask_dirty = false;
            }
        }
    }

    /// Resize the slice texture if needed
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32, format: wgpu::TextureFormat) {
        if self.width == width && self.height == height {
            return;
        }

        self.texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("Slice {} Texture", self.slice_id.0)),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        self.texture_view = self.texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.width = width;
        self.height = height;
        self.bind_group = None; // Needs recreation
    }
}

/// Rasterize a mask shape to an RGBA pixel buffer
///
/// Returns a Vec<u8> with size * size * 4 bytes (RGBA format).
/// The alpha channel contains the mask value (255 = inside, 0 = outside).
fn rasterize_mask(mask: &SliceMask, size: u32) -> Vec<u8> {
    let mut pixels = vec![0u8; (size * size * 4) as usize];

    match &mask.shape {
        MaskShape::Rectangle { x, y, width, height } => {
            rasterize_rectangle(&mut pixels, size, *x, *y, *width, *height, mask.feather);
        }
        MaskShape::Ellipse { center, radius_x, radius_y } => {
            rasterize_ellipse(&mut pixels, size, center.x, center.y, *radius_x, *radius_y, mask.feather);
        }
        MaskShape::Polygon { points } => {
            rasterize_polygon(&mut pixels, size, points, mask.feather);
        }
        MaskShape::Bezier { segments } => {
            // Tesselate bezier to polygon and rasterize
            let mut points = Vec::new();
            for segment in segments {
                // Sample each bezier segment at multiple points
                for i in 0..16 {
                    let t = i as f32 / 16.0;
                    let pt = segment.evaluate(t);
                    points.push(super::Point2D::new(pt.x, pt.y));
                }
            }
            if !points.is_empty() {
                rasterize_polygon(&mut pixels, size, &points, mask.feather);
            }
        }
    }

    pixels
}

/// Rasterize a rectangle mask
fn rasterize_rectangle(pixels: &mut [u8], size: u32, x: f32, y: f32, width: f32, height: f32, feather: f32) {
    let size_f = size as f32;

    for py in 0..size {
        for px in 0..size {
            let u = px as f32 / size_f;
            let v = py as f32 / size_f;

            // Calculate signed distance to rectangle edges
            let dx = if u < x {
                x - u
            } else if u > x + width {
                u - (x + width)
            } else {
                0.0
            };
            let dy = if v < y {
                y - v
            } else if v > y + height {
                v - (y + height)
            } else {
                0.0
            };

            // Distance to nearest edge (positive = outside, negative = inside)
            let dist = if dx == 0.0 && dy == 0.0 {
                // Inside rectangle - calculate distance to nearest edge
                let left_dist = u - x;
                let right_dist = (x + width) - u;
                let top_dist = v - y;
                let bottom_dist = (y + height) - v;
                -left_dist.min(right_dist).min(top_dist).min(bottom_dist)
            } else {
                (dx * dx + dy * dy).sqrt()
            };

            // Apply feathering
            let alpha = if feather > 0.0 {
                1.0 - (dist / feather).clamp(0.0, 1.0)
            } else {
                if dist <= 0.0 { 1.0 } else { 0.0 }
            };

            let idx = ((py * size + px) * 4) as usize;
            pixels[idx] = 255;     // R
            pixels[idx + 1] = 255; // G
            pixels[idx + 2] = 255; // B
            pixels[idx + 3] = (alpha * 255.0) as u8; // A
        }
    }
}

/// Rasterize an ellipse mask
fn rasterize_ellipse(pixels: &mut [u8], size: u32, cx: f32, cy: f32, rx: f32, ry: f32, feather: f32) {
    let size_f = size as f32;

    for py in 0..size {
        for px in 0..size {
            let u = px as f32 / size_f;
            let v = py as f32 / size_f;

            // Normalized distance from center (1.0 = on ellipse edge)
            let dx = (u - cx) / rx.max(0.0001);
            let dy = (v - cy) / ry.max(0.0001);
            let normalized_dist = (dx * dx + dy * dy).sqrt();

            // Convert to actual distance for feathering
            // This is an approximation - actual ellipse distance is complex
            let avg_radius = (rx + ry) / 2.0;
            let dist = (normalized_dist - 1.0) * avg_radius;

            // Apply feathering
            let alpha = if feather > 0.0 {
                1.0 - (dist / feather).clamp(0.0, 1.0)
            } else {
                if normalized_dist <= 1.0 { 1.0 } else { 0.0 }
            };

            let idx = ((py * size + px) * 4) as usize;
            pixels[idx] = 255;     // R
            pixels[idx + 1] = 255; // G
            pixels[idx + 2] = 255; // B
            pixels[idx + 3] = (alpha * 255.0) as u8; // A
        }
    }
}

/// Rasterize a polygon mask using point-in-polygon test
fn rasterize_polygon(pixels: &mut [u8], size: u32, points: &[super::Point2D], feather: f32) {
    if points.len() < 3 {
        return;
    }

    let size_f = size as f32;

    for py in 0..size {
        for px in 0..size {
            let u = px as f32 / size_f;
            let v = py as f32 / size_f;

            // Point-in-polygon test using ray casting
            let inside = point_in_polygon(u, v, points);

            // Calculate distance to nearest edge for feathering
            let dist = if feather > 0.0 {
                distance_to_polygon_edge(u, v, points)
            } else {
                0.0
            };

            // Apply feathering
            let alpha = if feather > 0.0 {
                if inside {
                    1.0 - ((-dist) / feather).clamp(0.0, 1.0).max(0.0)
                } else {
                    1.0 - (dist / feather).clamp(0.0, 1.0)
                }
            } else {
                if inside { 1.0 } else { 0.0 }
            };

            let idx = ((py * size + px) * 4) as usize;
            pixels[idx] = 255;     // R
            pixels[idx + 1] = 255; // G
            pixels[idx + 2] = 255; // B
            pixels[idx + 3] = (alpha * 255.0) as u8; // A
        }
    }
}

/// Point-in-polygon test using ray casting algorithm
fn point_in_polygon(x: f32, y: f32, points: &[super::Point2D]) -> bool {
    let n = points.len();
    let mut inside = false;

    let mut j = n - 1;
    for i in 0..n {
        let xi = points[i].x;
        let yi = points[i].y;
        let xj = points[j].x;
        let yj = points[j].y;

        if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }

    inside
}

/// Calculate minimum distance from a point to polygon edges
fn distance_to_polygon_edge(x: f32, y: f32, points: &[super::Point2D]) -> f32 {
    let n = points.len();
    let mut min_dist = f32::MAX;

    for i in 0..n {
        let j = (i + 1) % n;
        let dist = point_to_segment_distance(x, y, points[i].x, points[i].y, points[j].x, points[j].y);
        min_dist = min_dist.min(dist);
    }

    min_dist
}

/// Distance from point (px, py) to line segment from (x1, y1) to (x2, y2)
fn point_to_segment_distance(px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len_sq = dx * dx + dy * dy;

    if len_sq < 0.0001 {
        // Degenerate segment (point)
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }

    // Project point onto line, clamping to segment
    let t = (((px - x1) * dx + (py - y1) * dy) / len_sq).clamp(0.0, 1.0);
    let proj_x = x1 + t * dx;
    let proj_y = y1 + t * dy;

    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}

// ═══════════════════════════════════════════════════════════════════════════════
// FRAME DELAY BUFFER — Ring buffer for projector sync timing
// ═══════════════════════════════════════════════════════════════════════════════

/// Ring buffer for frame delay (projector sync)
///
/// Stores a configurable number of frames to introduce latency for projector timing sync.
/// When delay_frames is 0, no buffering occurs (passthrough mode).
pub struct FrameDelayBuffer {
    /// Ring buffer of frame textures
    frames: Vec<wgpu::Texture>,
    /// Ring buffer of texture views
    views: Vec<wgpu::TextureView>,
    /// Current write index in the ring buffer
    write_index: usize,
    /// Number of frames of delay (0 = passthrough)
    delay_frames: usize,
    /// Texture width
    width: u32,
    /// Texture height
    height: u32,
    /// Texture format
    format: wgpu::TextureFormat,
}

impl FrameDelayBuffer {
    /// Create a new frame delay buffer
    ///
    /// Initially empty - call `set_delay_frames` to allocate textures.
    pub fn new(width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        Self {
            frames: Vec::new(),
            views: Vec::new(),
            write_index: 0,
            delay_frames: 0,
            width,
            height,
            format,
        }
    }

    /// Get the current delay in frames
    pub fn delay_frames(&self) -> usize {
        self.delay_frames
    }

    /// Check if delay is active (non-zero)
    pub fn is_active(&self) -> bool {
        self.delay_frames > 0 && !self.frames.is_empty()
    }

    /// Set the delay in frames, allocating/deallocating textures as needed
    ///
    /// When delay changes, textures are reallocated and initialized to black.
    pub fn set_delay_frames(&mut self, device: &wgpu::Device, delay_frames: usize) {
        if delay_frames == self.delay_frames {
            return;
        }

        self.delay_frames = delay_frames;
        self.write_index = 0;

        // Deallocate if no delay
        if delay_frames == 0 {
            self.frames.clear();
            self.views.clear();
            return;
        }

        // Allocate ring buffer textures
        // We need delay_frames + 1 slots: one being written, delay_frames being read
        let buffer_size = delay_frames + 1;
        self.frames.clear();
        self.views.clear();

        for i in 0..buffer_size {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("Frame Delay Buffer Slot {}", i)),
                size: wgpu::Extent3d {
                    width: self.width.max(1),
                    height: self.height.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.frames.push(texture);
            self.views.push(view);
        }

        tracing::debug!(
            "Frame delay buffer: {} frames ({} buffer slots) @ {}x{}",
            delay_frames,
            buffer_size,
            self.width,
            self.height
        );
    }

    /// Resize the buffer textures
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == self.width && height == self.height {
            return;
        }

        self.width = width;
        self.height = height;

        // Re-allocate if active
        if self.delay_frames > 0 {
            let delay = self.delay_frames;
            self.delay_frames = 0; // Force reallocation
            self.set_delay_frames(device, delay);
        }
    }

    /// Push current frame and get delayed frame
    ///
    /// Copies `input_view` to the write slot and returns the view of the delayed frame.
    /// Returns None if delay is not active (caller should use input directly).
    pub fn push_and_get<'a>(
        &'a mut self,
        encoder: &mut wgpu::CommandEncoder,
        input_texture: &wgpu::Texture,
    ) -> Option<&'a wgpu::TextureView> {
        if !self.is_active() {
            return None;
        }

        let buffer_size = self.frames.len();

        // Copy input to current write slot
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: input_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &self.frames[self.write_index],
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        // Calculate read index (delay_frames behind write)
        let read_index = (self.write_index + buffer_size - self.delay_frames) % buffer_size;

        // Advance write index
        self.write_index = (self.write_index + 1) % buffer_size;

        Some(&self.views[read_index])
    }

    /// Get the delayed frame view without pushing a new frame
    ///
    /// Useful for reading the current delayed state without modifying the buffer.
    pub fn current_delayed_view(&self) -> Option<&wgpu::TextureView> {
        if !self.is_active() {
            return None;
        }

        let buffer_size = self.frames.len();
        // Read index is delay_frames behind the current write position
        // But since we haven't written yet, we use write_index - 1 as the "last written"
        let last_written = (self.write_index + buffer_size - 1) % buffer_size;
        let read_index = (last_written + buffer_size - self.delay_frames + 1) % buffer_size;

        Some(&self.views[read_index])
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SCREEN RUNTIME — GPU resources for a screen
// ═══════════════════════════════════════════════════════════════════════════════

/// Runtime GPU resources for a screen
pub struct ScreenRuntime {
    /// The screen ID this runtime belongs to
    pub screen_id: ScreenId,

    /// Output texture for the screen
    pub output_texture: wgpu::Texture,

    /// Output texture view for binding
    pub output_view: wgpu::TextureView,

    /// Secondary texture for color correction ping-pong
    pub color_temp_texture: wgpu::Texture,

    /// Secondary texture view for color correction
    pub color_temp_view: wgpu::TextureView,

    /// Bind group for color correction (samples from output_texture)
    pub color_bind_group: Option<wgpu::BindGroup>,

    /// Frame delay buffer for projector sync timing
    pub delay_buffer: FrameDelayBuffer,

    /// Slice runtimes for this screen
    pub slices: HashMap<SliceId, SliceRuntime>,

    /// Screen dimensions
    pub width: u32,
    pub height: u32,

    /// Texture format
    pub format: wgpu::TextureFormat,

    /// NDI capture for screens with NDI output device
    pub ndi_capture: Option<NdiCapture>,

    /// OMT capture for screens with OMT output device
    pub omt_capture: Option<crate::network::OmtCapture>,
}

impl ScreenRuntime {
    /// Create a new screen runtime with GPU resources
    pub fn new(
        device: &wgpu::Device,
        screen_id: ScreenId,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("Screen {} Output", screen_id.0)),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create secondary texture for color correction ping-pong
        let color_temp_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("Screen {} Color Temp", screen_id.0)),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let color_temp_view = color_temp_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create delay buffer (initially empty - will be allocated when delay_ms > 0)
        let delay_buffer = FrameDelayBuffer::new(width, height, format);

        Self {
            screen_id,
            output_texture,
            output_view,
            color_temp_texture,
            color_temp_view,
            color_bind_group: None,
            delay_buffer,
            slices: HashMap::new(),
            width,
            height,
            format,
            ndi_capture: None,
            omt_capture: None,
        }
    }

    /// Resize the screen output if needed
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }

        self.output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("Screen {} Output", self.screen_id.0)),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        self.output_view = self.output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Also resize the secondary texture for color correction
        self.color_temp_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("Screen {} Color Temp", self.screen_id.0)),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        self.color_temp_view = self.color_temp_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Invalidate color bind group (will be recreated when needed)
        self.color_bind_group = None;

        // Resize delay buffer if active
        self.delay_buffer.resize(device, width, height);

        self.width = width;
        self.height = height;
    }

    /// Ensure slice runtime exists for a slice
    pub fn ensure_slice(&mut self, device: &wgpu::Device, slice: &Slice) {
        if !self.slices.contains_key(&slice.id) {
            let runtime = SliceRuntime::new(device, slice.id, self.width, self.height, self.format);
            self.slices.insert(slice.id, runtime);
        }
    }

    /// Remove slice runtime
    pub fn remove_slice(&mut self, slice_id: SliceId) {
        self.slices.remove(&slice_id);
    }

    /// Get the output texture view
    pub fn output_view(&self) -> &wgpu::TextureView {
        &self.output_view
    }

    /// Get the output texture
    pub fn output_texture(&self) -> &wgpu::Texture {
        &self.output_texture
    }

    /// Update delay settings based on delay_ms and target_fps
    ///
    /// Calculates the number of frames needed for the given delay.
    pub fn update_delay(&mut self, device: &wgpu::Device, delay_ms: u32, target_fps: f32) {
        let delay_frames = if delay_ms == 0 || target_fps <= 0.0 {
            0
        } else {
            // delay_frames = delay_ms / (1000 / fps) = delay_ms * fps / 1000
            ((delay_ms as f32 * target_fps) / 1000.0).round() as usize
        };

        self.delay_buffer.set_delay_frames(device, delay_frames);
    }

    /// Get the delayed output view for presentation
    ///
    /// If delay is active, pushes the current frame and returns the delayed frame.
    /// If delay is not active, returns the direct output view.
    pub fn get_delayed_output<'a>(
        &'a mut self,
        encoder: &mut wgpu::CommandEncoder,
    ) -> &'a wgpu::TextureView {
        // Try to get delayed frame; if delay not active, return direct output
        if let Some(delayed_view) = self.delay_buffer.push_and_get(encoder, &self.output_texture) {
            delayed_view
        } else {
            &self.output_view
        }
    }

    /// Check if delay is currently active
    pub fn delay_active(&self) -> bool {
        self.delay_buffer.is_active()
    }

    /// Get the current delay in frames
    pub fn delay_frames(&self) -> usize {
        self.delay_buffer.delay_frames()
    }

    /// Update NDI output based on device type
    ///
    /// Creates or destroys NDI capture based on whether the screen is configured
    /// as an NDI output device.
    pub fn update_ndi_output(&mut self, device: &wgpu::Device, screen: &Screen, target_fps: u32) {
        match &screen.device {
            OutputDevice::Ndi { name } if screen.enabled => {
                // Create NDI capture if not already created or dimensions changed
                let needs_create = self.ndi_capture.as_ref().map_or(true, |capture| {
                    // Check if dimensions match
                    !capture.dimensions_match(self.width, self.height)
                });

                if needs_create {
                    tracing::info!(
                        "Creating NDI output for screen '{}' ({}x{}) as '{}'",
                        screen.name,
                        self.width,
                        self.height,
                        name
                    );

                    // Create NDI sender
                    match crate::network::NdiSender::new(name, target_fps) {
                        Ok(sender) => {
                            let mut capture = NdiCapture::new(device, self.width, self.height);
                            capture.start_sender_thread(sender);
                            self.ndi_capture = Some(capture);
                            tracing::info!("NDI output started for screen '{}'", screen.name);
                        }
                        Err(e) => {
                            tracing::error!("Failed to start NDI sender for '{}': {}", name, e);
                        }
                    }
                }
            }
            _ => {
                // Not an NDI device or disabled - remove capture if exists
                if self.ndi_capture.is_some() {
                    tracing::info!("Stopping NDI output for screen '{}'", screen.name);
                    self.ndi_capture = None;
                }
            }
        }
    }

    /// Capture frame to NDI if enabled
    ///
    /// Call this after rendering the screen to send the output to NDI.
    pub fn capture_ndi_frame(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(capture) = &mut self.ndi_capture {
            capture.capture_frame(encoder, &self.output_texture);
        }
    }

    /// Check if NDI output is active
    pub fn is_ndi_active(&self) -> bool {
        self.ndi_capture.is_some()
    }

    /// Update OMT output based on device type
    ///
    /// Creates or destroys OMT capture based on whether the screen is configured
    /// as an OMT output device.
    pub fn update_omt_output(
        &mut self,
        device: &wgpu::Device,
        screen: &Screen,
        target_fps: u32,
        tokio_handle: &tokio::runtime::Handle,
    ) {
        match &screen.device {
            OutputDevice::Omt { name, port } if screen.enabled => {
                // Create OMT capture if not already created or dimensions changed
                let needs_create = self.omt_capture.as_ref().map_or(true, |capture| {
                    !capture.dimensions_match(self.width, self.height)
                });

                if needs_create {
                    tracing::info!(
                        "Creating OMT output for screen '{}' ({}x{}) as '{}'",
                        screen.name,
                        self.width,
                        self.height,
                        name
                    );

                    let sender = crate::network::OmtSender::new(name.clone(), *port);
                    let mut capture = crate::network::OmtCapture::new(device, self.width, self.height);
                    capture.set_target_fps(target_fps);
                    capture.start_sender_thread(sender, tokio_handle.clone());
                    self.omt_capture = Some(capture);
                    tracing::info!("OMT output started for screen '{}'", screen.name);
                }
            }
            _ => {
                // Not an OMT device or disabled - remove capture if exists
                if self.omt_capture.is_some() {
                    tracing::info!("Stopping OMT output for screen '{}'", screen.name);
                    self.omt_capture = None;
                }
            }
        }
    }

    /// Capture frame to OMT if enabled
    ///
    /// Call this after rendering the screen to send the output to OMT.
    pub fn capture_omt_frame(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(capture) = &mut self.omt_capture {
            capture.capture_frame(encoder, &self.output_texture);
        }
    }

    /// Check if OMT output is active
    pub fn is_omt_active(&self) -> bool {
        self.omt_capture.is_some()
    }
}

/// Manages all screen and slice runtimes
pub struct OutputManager {
    /// Screen configurations (owned data)
    screens: HashMap<ScreenId, Screen>,

    /// Screen runtimes (GPU resources)
    runtimes: HashMap<ScreenId, ScreenRuntime>,

    /// Window IDs for screens with Display output devices
    screen_windows: HashMap<ScreenId, WindowId>,

    /// Next screen ID
    next_screen_id: u32,

    /// Next slice ID (global counter)
    next_slice_id: u32,

    /// Texture format for output
    format: wgpu::TextureFormat,

    // =========================================
    // Render Pipeline Infrastructure
    // =========================================
    /// Render pipeline for slice rendering (samples input, applies transforms)
    slice_render_pipeline: Option<wgpu::RenderPipeline>,

    /// Render pipeline for screen compositing (composites slices to screen output)
    screen_composite_pipeline: Option<wgpu::RenderPipeline>,

    /// Bind group layout for slice rendering (texture + sampler + SliceParams)
    slice_bind_group_layout: Option<wgpu::BindGroupLayout>,

    /// Bind group layout for mesh warp storage buffer
    warp_bind_group_layout: Option<wgpu::BindGroupLayout>,

    /// Bind group layout for screen compositing (texture + sampler + ScreenParams)
    screen_bind_group_layout: Option<wgpu::BindGroupLayout>,

    /// Shared sampler for texture filtering
    sampler: Option<wgpu::Sampler>,

    /// Uniform buffer for screen params
    screen_params_buffer: Option<wgpu::Buffer>,

    /// Dummy warp buffer for slices without mesh warp
    dummy_warp_buffer: Option<wgpu::Buffer>,

    /// Dummy warp bind group for slices without mesh warp
    dummy_warp_bind_group: Option<wgpu::BindGroup>,

    /// Bind group layout for mask texture (texture + sampler)
    mask_bind_group_layout: Option<wgpu::BindGroupLayout>,

    /// Dummy mask texture for slices without masking (1x1 white)
    dummy_mask_texture: Option<wgpu::Texture>,

    /// Dummy mask texture view for slices without masking
    dummy_mask_texture_view: Option<wgpu::TextureView>,

    /// Dummy mask bind group for slices without masking
    dummy_mask_bind_group: Option<wgpu::BindGroup>,

    // =========================================
    // Blit Pipeline (for presenting to surfaces)
    // =========================================
    /// Render pipeline for blitting textures to surfaces
    blit_pipeline: Option<wgpu::RenderPipeline>,

    /// Bind group layout for blit (texture + sampler only)
    blit_bind_group_layout: Option<wgpu::BindGroupLayout>,
}

impl OutputManager {
    /// Create a new output manager
    pub fn new(format: wgpu::TextureFormat) -> Self {
        Self {
            screens: HashMap::new(),
            runtimes: HashMap::new(),
            screen_windows: HashMap::new(),
            next_screen_id: 1,
            next_slice_id: 1,
            format,
            // Render infrastructure (initialized lazily)
            slice_render_pipeline: None,
            screen_composite_pipeline: None,
            slice_bind_group_layout: None,
            warp_bind_group_layout: None,
            screen_bind_group_layout: None,
            sampler: None,
            screen_params_buffer: None,
            dummy_warp_buffer: None,
            dummy_warp_bind_group: None,
            mask_bind_group_layout: None,
            dummy_mask_texture: None,
            dummy_mask_texture_view: None,
            dummy_mask_bind_group: None,
            blit_pipeline: None,
            blit_bind_group_layout: None,
        }
    }

    /// Create from existing screens (e.g., loaded from settings)
    pub fn from_screens(screens: Vec<Screen>, format: wgpu::TextureFormat) -> Self {
        let mut manager = Self::new(format);

        // Find max IDs
        for screen in &screens {
            if screen.id.0 >= manager.next_screen_id {
                manager.next_screen_id = screen.id.0 + 1;
            }
            for slice in &screen.slices {
                if slice.id.0 >= manager.next_slice_id {
                    manager.next_slice_id = slice.id.0 + 1;
                }
            }
        }

        // Add screens
        for screen in screens {
            manager.screens.insert(screen.id, screen);
        }

        manager
    }

    /// Initialize GPU runtimes for all screens
    pub fn init_runtimes(&mut self, device: &wgpu::Device) {
        for screen in self.screens.values() {
            if !self.runtimes.contains_key(&screen.id) {
                let mut runtime = ScreenRuntime::new(
                    device,
                    screen.id,
                    screen.width,
                    screen.height,
                    self.format,
                );

                // Create slice runtimes
                for slice in &screen.slices {
                    runtime.ensure_slice(device, slice);
                }

                self.runtimes.insert(screen.id, runtime);
            }
        }
    }

    /// Create render pipelines for slice rendering and screen compositing
    ///
    /// This must be called before render_screen() can be used.
    pub fn create_pipelines(&mut self, device: &wgpu::Device) {
        // Create shared sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Output Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        self.sampler = Some(sampler);

        // Create screen params buffer
        let screen_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Screen Params Buffer"),
            size: std::mem::size_of::<ScreenParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.screen_params_buffer = Some(screen_params_buffer);

        // Create slice bind group layout
        let slice_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Slice Bind Group Layout"),
            entries: &[
                // Input texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // SliceParams uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create warp bind group layout (for mesh warp storage buffer)
        let warp_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Warp Bind Group Layout"),
            entries: &[
                // Warp points storage buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create dummy warp buffer (single point, used when mesh warp is disabled)
        let dummy_warp_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dummy Warp Buffer"),
            size: std::mem::size_of::<WarpPointGpu>() as u64,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        // Create dummy warp bind group
        let dummy_warp_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Dummy Warp Bind Group"),
            layout: &warp_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: dummy_warp_buffer.as_entire_binding(),
            }],
        });

        self.warp_bind_group_layout = Some(warp_bind_group_layout);
        self.dummy_warp_buffer = Some(dummy_warp_buffer);
        self.dummy_warp_bind_group = Some(dummy_warp_bind_group);

        // Create mask bind group layout (for mask texture + sampler)
        let mask_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Mask Bind Group Layout"),
            entries: &[
                // Mask texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Mask sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create dummy mask texture (1x1 white with full alpha - passes through everything)
        let dummy_mask_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Dummy Mask Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let dummy_mask_texture_view = dummy_mask_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create dummy mask bind group
        let dummy_mask_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Dummy Mask Bind Group"),
            layout: &mask_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&dummy_mask_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(self.sampler.as_ref().unwrap()),
                },
            ],
        });

        self.mask_bind_group_layout = Some(mask_bind_group_layout);
        self.dummy_mask_texture = Some(dummy_mask_texture);
        self.dummy_mask_texture_view = Some(dummy_mask_texture_view);
        self.dummy_mask_bind_group = Some(dummy_mask_bind_group);

        // Create screen bind group layout
        let screen_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Screen Bind Group Layout"),
            entries: &[
                // Slice texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // ScreenParams uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create slice render pipeline
        let slice_shader_source = crate::shaders::load_slice_render_shader();
        let slice_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Slice Render Shader"),
            source: wgpu::ShaderSource::Wgsl(slice_shader_source.into()),
        });

        let slice_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Slice Pipeline Layout"),
            bind_group_layouts: &[
                &slice_bind_group_layout,
                self.warp_bind_group_layout.as_ref().unwrap(),
                self.mask_bind_group_layout.as_ref().unwrap(),
            ],
            push_constant_ranges: &[],
        });

        let slice_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Slice Render Pipeline"),
            layout: Some(&slice_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &slice_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &slice_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create screen composite pipeline
        let screen_shader_source = crate::shaders::load_screen_composite_shader();
        let screen_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Screen Composite Shader"),
            source: wgpu::ShaderSource::Wgsl(screen_shader_source.into()),
        });

        let screen_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Screen Pipeline Layout"),
            bind_group_layouts: &[&screen_bind_group_layout],
            push_constant_ranges: &[],
        });

        let screen_composite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Screen Composite Pipeline"),
            layout: Some(&screen_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &screen_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &screen_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        self.slice_bind_group_layout = Some(slice_bind_group_layout);
        self.screen_bind_group_layout = Some(screen_bind_group_layout);
        self.slice_render_pipeline = Some(slice_render_pipeline);
        self.screen_composite_pipeline = Some(screen_composite_pipeline);

        // Create blit pipeline for presenting to surfaces
        let blit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Blit Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/output/blit.wgsl").into()),
        });

        let blit_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Blit Bind Group Layout"),
                entries: &[
                    // Source texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Blit Pipeline Layout"),
            bind_group_layouts: &[&blit_bind_group_layout],
            push_constant_ranges: &[],
        });

        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Blit Pipeline"),
            layout: Some(&blit_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &blit_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &blit_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.format,
                    blend: None, // No blending - direct copy
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        self.blit_bind_group_layout = Some(blit_bind_group_layout);
        self.blit_pipeline = Some(blit_pipeline);

        tracing::info!("Output render pipelines created");
    }

    /// Check if render pipelines are initialized
    pub fn has_pipelines(&self) -> bool {
        self.slice_render_pipeline.is_some()
    }

    /// Add a new screen with default slice
    pub fn add_screen(&mut self, device: &wgpu::Device, name: impl Into<String>) -> ScreenId {
        let screen_id = ScreenId(self.next_screen_id);
        self.next_screen_id += 1;

        let slice_id = SliceId(self.next_slice_id);
        self.next_slice_id += 1;

        let screen = Screen::new_with_default_slice(screen_id, name, slice_id);
        let width = screen.width;
        let height = screen.height;

        // Create runtime
        let mut runtime = ScreenRuntime::new(device, screen_id, width, height, self.format);
        for slice in &screen.slices {
            runtime.ensure_slice(device, slice);
        }

        self.runtimes.insert(screen_id, runtime);
        self.screens.insert(screen_id, screen);

        screen_id
    }

    /// Add a screen from existing Screen data (for loading presets)
    pub fn add_screen_from_data(&mut self, device: &wgpu::Device, mut screen: Screen) -> ScreenId {
        // Assign new IDs to avoid conflicts
        let screen_id = ScreenId(self.next_screen_id);
        self.next_screen_id += 1;
        screen.id = screen_id;

        // Reassign slice IDs
        for slice in &mut screen.slices {
            slice.id = SliceId(self.next_slice_id);
            self.next_slice_id += 1;
        }

        let width = screen.width;
        let height = screen.height;

        // Create runtime
        let mut runtime = ScreenRuntime::new(device, screen_id, width, height, self.format);
        for slice in &screen.slices {
            runtime.ensure_slice(device, slice);
        }

        self.runtimes.insert(screen_id, runtime);
        self.screens.insert(screen_id, screen);

        screen_id
    }

    /// Remove a screen
    pub fn remove_screen(&mut self, screen_id: ScreenId) {
        self.screens.remove(&screen_id);
        self.runtimes.remove(&screen_id);
    }

    /// Get a screen by ID
    pub fn get_screen(&self, screen_id: ScreenId) -> Option<&Screen> {
        self.screens.get(&screen_id)
    }

    /// Get a mutable screen by ID
    pub fn get_screen_mut(&mut self, screen_id: ScreenId) -> Option<&mut Screen> {
        self.screens.get_mut(&screen_id)
    }

    /// Get screen runtime by ID
    pub fn get_runtime(&self, screen_id: ScreenId) -> Option<&ScreenRuntime> {
        self.runtimes.get(&screen_id)
    }

    /// Get mutable screen runtime by ID
    pub fn get_runtime_mut(&mut self, screen_id: ScreenId) -> Option<&mut ScreenRuntime> {
        self.runtimes.get_mut(&screen_id)
    }

    /// Get all screens
    pub fn screens(&self) -> impl Iterator<Item = &Screen> {
        self.screens.values()
    }

    /// Get all enabled screens
    pub fn enabled_screens(&self) -> impl Iterator<Item = &Screen> {
        self.screens.values().filter(|s| s.enabled)
    }

    /// Get screen count
    pub fn screen_count(&self) -> usize {
        self.screens.len()
    }

    // =========================================
    // Window Management (for Display output devices)
    // =========================================

    /// Get screens that need windows created for their Display output devices
    ///
    /// Returns (screen_id, display_id) pairs for screens that:
    /// - Are enabled
    /// - Have OutputDevice::Display
    /// - Don't already have a window assigned
    pub fn pending_display_windows(&self) -> Vec<(ScreenId, u32)> {
        self.screens
            .values()
            .filter(|screen| screen.enabled)
            .filter_map(|screen| match &screen.device {
                OutputDevice::Display { display_id } => {
                    if !self.screen_windows.contains_key(&screen.id) {
                        Some((screen.id, *display_id))
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect()
    }

    /// Associate a window with a screen
    ///
    /// Call this after creating a window for a screen's Display output.
    pub fn set_window_for_screen(&mut self, screen_id: ScreenId, window_id: WindowId) {
        self.screen_windows.insert(screen_id, window_id);
    }

    /// Remove window association for a screen
    ///
    /// Call this when a display window is closed.
    pub fn remove_window_for_screen(&mut self, screen_id: ScreenId) -> Option<WindowId> {
        self.screen_windows.remove(&screen_id)
    }

    /// Get the window ID for a screen (if any)
    pub fn get_window_for_screen(&self, screen_id: ScreenId) -> Option<WindowId> {
        self.screen_windows.get(&screen_id).copied()
    }

    /// Find the screen ID for a given window ID
    pub fn get_screen_for_window(&self, window_id: WindowId) -> Option<ScreenId> {
        self.screen_windows
            .iter()
            .find(|(_, &wid)| wid == window_id)
            .map(|(&sid, _)| sid)
    }

    /// Check if a screen has an associated window
    pub fn screen_has_window(&self, screen_id: ScreenId) -> bool {
        self.screen_windows.contains_key(&screen_id)
    }

    /// Get all screens with windows
    pub fn screens_with_windows(&self) -> impl Iterator<Item = (ScreenId, WindowId)> + '_ {
        self.screen_windows.iter().map(|(&s, &w)| (s, w))
    }

    /// Get screens that have windows but should no longer (device changed or disabled)
    ///
    /// Returns screen IDs that should have their windows closed.
    pub fn stale_display_windows(&self) -> Vec<ScreenId> {
        self.screen_windows
            .keys()
            .filter(|screen_id| {
                self.screens.get(screen_id).map_or(true, |screen| {
                    !screen.enabled || !matches!(screen.device, OutputDevice::Display { .. })
                })
            })
            .copied()
            .collect()
    }

    // =========================================
    // Surface Presentation (for monitor windows)
    // =========================================

    /// Present a screen's output texture to a surface
    ///
    /// This blits the screen's rendered output to the provided surface view.
    /// Used for displaying screen content on monitor windows.
    /// If the screen has delay configured, the delayed frame is presented.
    pub fn present_to_surface(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        screen_id: ScreenId,
        surface_view: &wgpu::TextureView,
        surface_format: wgpu::TextureFormat,
    ) -> bool {
        // Get the blit pipeline and resources (borrow these first)
        let Some(blit_pipeline) = &self.blit_pipeline else {
            return false;
        };
        let Some(blit_bind_group_layout) = &self.blit_bind_group_layout else {
            return false;
        };
        let Some(sampler) = &self.sampler else {
            return false;
        };

        // Clone references we need to keep across the mutable borrow
        let blit_pipeline = blit_pipeline.clone();
        let blit_bind_group_layout = blit_bind_group_layout.clone();
        let sampler = sampler.clone();

        // Get the screen runtime mutably to access delay buffer
        let Some(runtime) = self.runtimes.get_mut(&screen_id) else {
            return false;
        };

        // Get the output view (delayed if delay buffer is active)
        let source_view = runtime.get_delayed_output(encoder);

        // Create bind group for this blit operation
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blit Bind Group"),
            layout: &blit_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Check if we need to handle format mismatch
        // For now, we assume formats match. In the future, we could add format conversion.
        let _ = surface_format;

        // Create render pass to blit to surface
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Blit to Surface"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&blit_pipeline);
            render_pass.set_bind_group(0, &bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        true
    }

    /// Add a slice to a screen
    pub fn add_slice(
        &mut self,
        device: &wgpu::Device,
        screen_id: ScreenId,
        name: impl Into<String>,
    ) -> Option<SliceId> {
        let screen = self.screens.get_mut(&screen_id)?;

        let slice_id = SliceId(self.next_slice_id);
        self.next_slice_id += 1;

        let slice = Slice::new_full_composition(slice_id, name);
        screen.slices.push(slice.clone());

        // Create slice runtime
        if let Some(runtime) = self.runtimes.get_mut(&screen_id) {
            runtime.ensure_slice(device, &slice);
        }

        Some(slice_id)
    }

    /// Remove a slice from a screen
    pub fn remove_slice(&mut self, screen_id: ScreenId, slice_id: SliceId) -> bool {
        let Some(screen) = self.screens.get_mut(&screen_id) else {
            return false;
        };

        if screen.remove_slice(slice_id).is_some() {
            if let Some(runtime) = self.runtimes.get_mut(&screen_id) {
                runtime.remove_slice(slice_id);
            }
            true
        } else {
            false
        }
    }

    /// Sync screen data to runtime (after screen properties change)
    ///
    /// `target_fps` is used to calculate the number of delay frames from delay_ms.
    /// `tokio_handle` is needed for OMT output devices.
    pub fn sync_runtime(
        &mut self,
        device: &wgpu::Device,
        screen_id: ScreenId,
        target_fps: f32,
        tokio_handle: Option<&tokio::runtime::Handle>,
    ) {
        let Some(screen) = self.screens.get(&screen_id) else {
            return;
        };

        // Get screen settings we need
        let width = screen.width;
        let height = screen.height;
        let delay_ms = screen.delay_ms;
        let slices: Vec<_> = screen.slices.clone();

        // Get or create runtime
        let runtime = self.runtimes.entry(screen_id).or_insert_with(|| {
            ScreenRuntime::new(device, screen_id, width, height, self.format)
        });

        // Resize if needed
        runtime.resize(device, width, height);

        // Update delay settings
        runtime.update_delay(device, delay_ms, target_fps);

        // Sync slices
        let slice_ids: Vec<_> = slices.iter().map(|s| s.id).collect();

        // Ensure all slices have runtimes
        for slice in &slices {
            runtime.ensure_slice(device, slice);
        }

        // Remove orphaned slice runtimes
        let orphaned: Vec<_> = runtime.slices.keys()
            .filter(|id| !slice_ids.contains(id))
            .copied()
            .collect();
        for id in orphaned {
            runtime.remove_slice(id);
        }

        // Update NDI output based on device type
        // Re-borrow screen since we consumed it earlier
        if let Some(screen) = self.screens.get(&screen_id) {
            if let Some(runtime) = self.runtimes.get_mut(&screen_id) {
                runtime.update_ndi_output(device, screen, target_fps as u32);
            }
        }

        // Update OMT output based on device type
        if let Some(handle) = tokio_handle {
            if let Some(screen) = self.screens.get(&screen_id) {
                if let Some(runtime) = self.runtimes.get_mut(&screen_id) {
                    runtime.update_omt_output(device, screen, target_fps as u32, handle);
                }
            }
        }
    }

    /// Capture NDI frame for a screen (if NDI output is enabled)
    pub fn capture_ndi_frame(&mut self, encoder: &mut wgpu::CommandEncoder, screen_id: ScreenId) {
        if let Some(runtime) = self.runtimes.get_mut(&screen_id) {
            runtime.capture_ndi_frame(encoder);
        }
    }

    /// Process NDI capture pipelines for all screens.
    ///
    /// Call this after queue.submit() to poll GPU and send captured frames.
    pub fn process_ndi_captures(&mut self, device: &wgpu::Device) {
        for runtime in self.runtimes.values_mut() {
            if let Some(capture) = &mut runtime.ndi_capture {
                capture.process(device);
            }
        }
    }

    /// Capture OMT frame for a screen (if OMT output is enabled)
    pub fn capture_omt_frame(&mut self, encoder: &mut wgpu::CommandEncoder, screen_id: ScreenId) {
        if let Some(runtime) = self.runtimes.get_mut(&screen_id) {
            runtime.capture_omt_frame(encoder);
        }
    }

    /// Process OMT capture pipelines for all screens.
    ///
    /// Call this after queue.submit() to poll GPU and send captured frames.
    pub fn process_omt_captures(&mut self, device: &wgpu::Device) {
        for runtime in self.runtimes.values_mut() {
            if let Some(capture) = &mut runtime.omt_capture {
                capture.process(device);
            }
        }
    }

    /// Export screens for serialization
    pub fn export_screens(&self) -> Vec<Screen> {
        self.screens.values().cloned().collect()
    }

    /// Check if a slice uses layer input
    pub fn slice_uses_layer(&self, screen_id: ScreenId, slice_id: SliceId, layer_id: u32) -> bool {
        self.screens.get(&screen_id)
            .and_then(|s| s.find_slice(slice_id))
            .map(|slice| matches!(slice.input, SliceInput::Layer { layer_id: id } if id == layer_id))
            .unwrap_or(false)
    }

    /// Get IDs of all enabled screens
    pub fn enabled_screen_ids(&self) -> Vec<ScreenId> {
        self.screens.values().filter(|s| s.enabled).map(|s| s.id).collect()
    }

    /// Render all slices for a screen to the screen's output texture
    ///
    /// This method renders each slice from its input source (composition or layer)
    /// to the screen's output texture.
    ///
    /// # Arguments
    /// * `device` - The wgpu device
    /// * `queue` - The wgpu queue for buffer uploads
    /// * `encoder` - The command encoder
    /// * `screen_id` - The screen to render
    /// * `environment_view` - Texture view for the composition input
    /// * `layer_textures` - Map of layer_id to texture view for layer inputs
    pub fn render_screen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        screen_id: ScreenId,
        environment_view: &wgpu::TextureView,
        layer_textures: &HashMap<u32, &wgpu::TextureView>,
    ) {
        // Get required render infrastructure
        let Some(slice_pipeline) = &self.slice_render_pipeline else {
            tracing::warn!("Slice render pipeline not initialized");
            return;
        };
        let Some(slice_bind_group_layout) = &self.slice_bind_group_layout else {
            return;
        };
        let Some(warp_bind_group_layout) = &self.warp_bind_group_layout else {
            return;
        };
        let Some(sampler) = &self.sampler else {
            return;
        };
        let Some(dummy_warp_bind_group) = &self.dummy_warp_bind_group else {
            return;
        };
        let Some(mask_bind_group_layout) = &self.mask_bind_group_layout else {
            return;
        };
        let Some(dummy_mask_bind_group) = &self.dummy_mask_bind_group else {
            return;
        };

        // Get screen config and runtime
        let Some(screen) = self.screens.get(&screen_id) else {
            return;
        };
        let Some(runtime) = self.runtimes.get_mut(&screen_id) else {
            return;
        };

        if !screen.enabled {
            return;
        }

        // Clone slices to avoid borrow issues
        let slices: Vec<Slice> = screen.slices.clone();

        // First, clear the screen output to black
        {
            // Clear pass - begins and ends immediately (enclosed in block)
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Screen Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &runtime.output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            // Render pass automatically ends when dropped
        }

        // Render each slice
        for slice in &slices {
            // Determine input texture based on slice input type
            let input_view = match &slice.input {
                SliceInput::Composition => environment_view,
                SliceInput::Layer { layer_id } => {
                    if let Some(view) = layer_textures.get(layer_id) {
                        *view
                    } else {
                        // Layer not found, skip this slice
                        continue;
                    }
                }
            };

            // Get or skip slice runtime
            let Some(slice_runtime) = runtime.slices.get_mut(&slice.id) else {
                continue;
            };

            // Update slice params buffer
            slice_runtime.update_params(queue, slice);

            // Update warp buffer if mesh warp is enabled
            slice_runtime.update_warp_buffer(device, queue, slice.output.mesh.as_ref());

            // Update mask texture if mask is enabled (before borrowing bind groups)
            slice_runtime.update_mask_texture(device, queue, slice.mask.as_ref());

            // Create warp bind group if needed (must be done before borrowing)
            if slice_runtime.warp_buffer.is_some() && slice_runtime.warp_bind_group.is_none() {
                slice_runtime.warp_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("Slice {} Warp Bind Group", slice.id.0)),
                    layout: warp_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: slice_runtime.warp_buffer.as_ref().unwrap().as_entire_binding(),
                    }],
                }));
            }

            // Create mask bind group if needed (must be done before borrowing)
            if slice_runtime.mask_texture_view.is_some() && slice_runtime.mask_bind_group.is_none() {
                slice_runtime.mask_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("Slice {} Mask Bind Group", slice.id.0)),
                    layout: mask_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(slice_runtime.mask_texture_view.as_ref().unwrap()),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(sampler),
                        },
                    ],
                }));
            }

            // Get bind groups (immutable borrows after mutable work is done)
            let warp_bind_group = slice_runtime.warp_bind_group.as_ref().unwrap_or(dummy_warp_bind_group);
            let mask_bind_group = slice_runtime.mask_bind_group.as_ref().unwrap_or(dummy_mask_bind_group);

            // Create bind group for this slice
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Slice {} Bind Group", slice.id.0)),
                layout: slice_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(input_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: slice_runtime.params_buffer.as_entire_binding(),
                    },
                ],
            });

            // Render the slice directly to the screen output
            // (rendering each slice to its own texture then compositing would be more flexible
            // but for now we render directly with alpha blending)
            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some(&format!("Slice {} Render Pass", slice.id.0)),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &runtime.output_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                render_pass.set_pipeline(slice_pipeline);
                render_pass.set_bind_group(0, &bind_group, &[]);
                render_pass.set_bind_group(1, warp_bind_group, &[]);
                render_pass.set_bind_group(2, mask_bind_group, &[]);
                render_pass.draw(0..3, 0..1); // Fullscreen triangle
            }
        }
    }

    /// Get the slice bind group layout for creating external bind groups
    pub fn slice_bind_group_layout(&self) -> Option<&wgpu::BindGroupLayout> {
        self.slice_bind_group_layout.as_ref()
    }

    /// Get the shared sampler
    pub fn sampler(&self) -> Option<&wgpu::Sampler> {
        self.sampler.as_ref()
    }

    /// Apply screen-level color correction
    ///
    /// This should be called after render_screen() to apply per-screen color correction.
    /// Uses a ping-pong approach: renders from output_texture to color_temp_texture with
    /// color correction, then copies back to output_texture.
    ///
    /// If the screen's color correction is at identity, this is a no-op.
    pub fn apply_screen_color(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        screen_id: ScreenId,
    ) {
        // Get screen config
        let Some(screen) = self.screens.get(&screen_id) else {
            return;
        };

        // Skip if color correction is at identity
        if screen.color.is_identity() {
            return;
        }

        // Get required pipeline infrastructure
        let Some(screen_pipeline) = &self.screen_composite_pipeline else {
            tracing::warn!("Screen composite pipeline not initialized");
            return;
        };
        let Some(screen_bind_group_layout) = &self.screen_bind_group_layout else {
            return;
        };
        let Some(sampler) = &self.sampler else {
            return;
        };
        let Some(screen_params_buffer) = &self.screen_params_buffer else {
            return;
        };

        // Get screen runtime
        let Some(runtime) = self.runtimes.get_mut(&screen_id) else {
            return;
        };

        // Update screen params buffer with color correction values
        let screen_params = ScreenParams::from_color(&screen.color);
        queue.write_buffer(screen_params_buffer, 0, bytemuck::bytes_of(&screen_params));

        // Create or get color bind group (samples from output_texture)
        if runtime.color_bind_group.is_none() {
            runtime.color_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Screen {} Color Bind Group", screen_id.0)),
                layout: screen_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&runtime.output_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: screen_params_buffer.as_entire_binding(),
                    },
                ],
            }));
        }

        let color_bind_group = runtime.color_bind_group.as_ref().unwrap();

        // Pass 1: Render from output_texture to color_temp_texture with color correction
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Screen Color Correction Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &runtime.color_temp_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(screen_pipeline);
            render_pass.set_bind_group(0, color_bind_group, &[]);
            render_pass.draw(0..3, 0..1); // Fullscreen triangle
        }

        // Pass 2: Copy from color_temp_texture back to output_texture
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture {
                texture: &runtime.color_temp_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyTexture {
                texture: &runtime.output_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: runtime.width,
                height: runtime.height,
                depth_or_array_layers: 1,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slice_params_default() {
        let params = SliceParams::default();
        assert_eq!(params.input_rect, [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(params.opacity, 1.0);
    }

    #[test]
    fn test_slice_params_from_slice() {
        let mut slice = Slice::default();
        slice.output.flip_h = true;
        slice.color.opacity = 0.5;

        let params = SliceParams::from_slice(&slice);
        assert_eq!(params.flip[0], 1.0);
        assert_eq!(params.opacity, 0.5);
    }

    #[test]
    fn test_frame_delay_buffer_new() {
        let buffer = FrameDelayBuffer::new(1920, 1080, wgpu::TextureFormat::Rgba8Unorm);
        assert!(!buffer.is_active());
        assert_eq!(buffer.delay_frames(), 0);
    }

    #[test]
    fn test_frame_delay_calculation() {
        // Test delay frame calculation
        // At 60fps: 100ms delay = 6 frames
        let frames_100ms_60fps = ((100.0_f32 * 60.0) / 1000.0).round() as usize;
        assert_eq!(frames_100ms_60fps, 6);

        // At 30fps: 100ms delay = 3 frames
        let frames_100ms_30fps = ((100.0_f32 * 30.0) / 1000.0).round() as usize;
        assert_eq!(frames_100ms_30fps, 3);

        // Zero delay = 0 frames
        let frames_0ms = ((0.0_f32 * 60.0) / 1000.0).round() as usize;
        assert_eq!(frames_0ms, 0);
    }
}
