//! Output runtime state for GPU resources
//!
//! This module contains the runtime GPU resources for screens and slices.
//! The Screen/Slice structs in the output module are pure data (configuration),
//! while Runtime structs hold the actual GPU resources needed for rendering.

use std::collections::HashMap;

use super::{Screen, ScreenId, Slice, SliceId, SliceInput};

/// Uniform buffer data for slice rendering
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SliceParams {
    /// Input rect (x, y, width, height) - normalized 0.0-1.0
    pub input_rect: [f32; 4],
    /// Output rect (x, y, width, height) - normalized 0.0-1.0
    pub output_rect: [f32; 4],
    /// Rotation in radians
    pub rotation: f32,
    /// Flip flags (x = horizontal, y = vertical)
    pub flip: [f32; 2],
    /// Opacity
    pub opacity: f32,
    /// Color correction: brightness, contrast, gamma, saturation
    pub color_adjust: [f32; 4],
    /// RGB channel multipliers + padding
    pub color_rgb: [f32; 4],
}

impl Default for SliceParams {
    fn default() -> Self {
        Self {
            input_rect: [0.0, 0.0, 1.0, 1.0],
            output_rect: [0.0, 0.0, 1.0, 1.0],
            rotation: 0.0,
            flip: [0.0, 0.0],
            opacity: 1.0,
            color_adjust: [0.0, 1.0, 1.0, 1.0], // brightness, contrast, gamma, saturation
            color_rgb: [1.0, 1.0, 1.0, 0.0],    // R, G, B, padding
        }
    }
}

impl SliceParams {
    /// Create params from a Slice configuration
    pub fn from_slice(slice: &Slice) -> Self {
        let flip_h = if slice.output.flip_h { 1.0 } else { 0.0 };
        let flip_v = if slice.output.flip_v { 1.0 } else { 0.0 };

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
            flip: [flip_h, flip_v],
            opacity: slice.color.opacity,
            color_adjust: [
                slice.color.brightness,
                slice.color.contrast,
                slice.color.gamma,
                1.0, // saturation placeholder
            ],
            color_rgb: [slice.color.red, slice.color.green, slice.color.blue, 0.0],
        }
    }
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
            width,
            height,
        }
    }

    /// Update the params buffer with new slice configuration
    pub fn update_params(&self, queue: &wgpu::Queue, slice: &Slice) {
        let params = SliceParams::from_slice(slice);
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));
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
