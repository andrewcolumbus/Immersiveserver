//! Environment (composition canvas)
//!
//! The Environment is a fixed-resolution render target that represents the
//! composition canvas. It is independent of the window size; the window is
//! simply a viewport that displays the environment (typically scaled to fit).

/// Fixed-resolution composition canvas backed by a GPU texture.
pub struct Environment {
    /// Width of the composition canvas in pixels
    width: u32,
    /// Height of the composition canvas in pixels
    height: u32,
    /// Texture format for the environment render target
    format: wgpu::TextureFormat,
    /// GPU texture for the environment (render target)
    texture: wgpu::Texture,
    /// View of the environment texture
    texture_view: wgpu::TextureView,
}

impl Environment {
    /// Create a new Environment render target.
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        let width = width.max(1);
        let height = height.max(1);

        let (texture, texture_view) = Self::create_texture(device, width, height, format);

        Self {
            width,
            height,
            format,
            texture,
            texture_view,
        }
    }

    /// Resize the environment canvas.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);

        if width == self.width && height == self.height {
            return;
        }

        let (texture, texture_view) = Self::create_texture(device, width, height, self.format);

        self.width = width;
        self.height = height;
        self.texture = texture;
        self.texture_view = texture_view;
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn texture_view(&self) -> &wgpu::TextureView {
        &self.texture_view
    }

    fn create_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Environment Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }
}


