//! Output runtime state for GPU resources
//!
//! This module contains the runtime GPU resources for screens and slices.
//! The Screen/Slice structs in the output module are pure data (configuration),
//! while Runtime structs hold the actual GPU resources needed for rendering.

use std::collections::HashMap;

use super::{Screen, ScreenId, Slice, SliceId, SliceInput, WarpMesh};

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
}

/// Uniform buffer data for slice rendering
///
/// IMPORTANT: This struct must match the WGSL SliceParams layout exactly.
/// WGSL alignment rules: vec2 needs 8-byte alignment, vec4 needs 16-byte alignment.
/// Total size: 224 bytes
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
}                                    // Total: 224 bytes

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

/// Runtime GPU resources for a screen
pub struct ScreenRuntime {
    /// The screen ID this runtime belongs to
    pub screen_id: ScreenId,

    /// Output texture for the screen
    pub output_texture: wgpu::Texture,

    /// Output texture view for binding
    pub output_view: wgpu::TextureView,

    /// Slice runtimes for this screen
    pub slices: HashMap<SliceId, SliceRuntime>,

    /// Screen dimensions
    pub width: u32,
    pub height: u32,

    /// Texture format
    pub format: wgpu::TextureFormat,
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

        Self {
            screen_id,
            output_texture,
            output_view,
            slices: HashMap::new(),
            width,
            height,
            format,
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
}

/// Manages all screen and slice runtimes
pub struct OutputManager {
    /// Screen configurations (owned data)
    screens: HashMap<ScreenId, Screen>,

    /// Screen runtimes (GPU resources)
    runtimes: HashMap<ScreenId, ScreenRuntime>,

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
}

impl OutputManager {
    /// Create a new output manager
    pub fn new(format: wgpu::TextureFormat) -> Self {
        Self {
            screens: HashMap::new(),
            runtimes: HashMap::new(),
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
            bind_group_layouts: &[&slice_bind_group_layout, self.warp_bind_group_layout.as_ref().unwrap()],
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
    pub fn sync_runtime(&mut self, device: &wgpu::Device, screen_id: ScreenId) {
        let Some(screen) = self.screens.get(&screen_id) else {
            return;
        };

        // Get or create runtime
        let runtime = self.runtimes.entry(screen_id).or_insert_with(|| {
            ScreenRuntime::new(device, screen_id, screen.width, screen.height, self.format)
        });

        // Resize if needed
        runtime.resize(device, screen.width, screen.height);

        // Sync slices
        let slice_ids: Vec<_> = screen.slices.iter().map(|s| s.id).collect();

        // Ensure all slices have runtimes
        for slice in &screen.slices {
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

            // Create or get warp bind group
            let warp_bind_group = if let Some(warp_buffer) = &slice_runtime.warp_buffer {
                // Mesh warp is enabled - create bind group if needed
                if slice_runtime.warp_bind_group.is_none() {
                    slice_runtime.warp_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some(&format!("Slice {} Warp Bind Group", slice.id.0)),
                        layout: warp_bind_group_layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: warp_buffer.as_entire_binding(),
                        }],
                    }));
                }
                slice_runtime.warp_bind_group.as_ref().unwrap()
            } else {
                // No mesh warp - use dummy bind group
                dummy_warp_bind_group
            };

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
}
