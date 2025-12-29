//! GPU texture for video frames
//!
//! Manages a wgpu texture that can receive decoded video frame data
//! and be used for rendering.

use super::DecodedFrame;

/// A GPU texture for displaying video frames
///
/// Handles texture creation and RGBA data upload.
/// Bind groups are created separately by VideoRenderer.
pub struct VideoTexture {
    /// The GPU texture
    texture: wgpu::Texture,
    /// Texture view for binding
    view: wgpu::TextureView,
    /// Texture width in pixels
    width: u32,
    /// Texture height in pixels
    height: u32,
}

impl VideoTexture {
    /// Create a new video texture with the specified dimensions
    ///
    /// # Arguments
    /// * `device` - The wgpu device
    /// * `width` - Texture width in pixels
    /// * `height` - Texture height in pixels
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let (texture, view) = Self::create_texture(device, width, height);

        Self {
            texture,
            view,
            width,
            height,
        }
    }

    /// Create the GPU texture
    fn create_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Video Texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Use Rgba8UnormSrgb for proper gamma-corrected color display
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            // COPY_DST for uploading data, TEXTURE_BINDING for shader sampling
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        (texture, view)
    }

    /// Upload a decoded frame to the GPU texture
    ///
    /// The frame must have the same dimensions as the texture.
    /// If dimensions don't match, call `resize()` first.
    pub fn upload(&self, queue: &wgpu::Queue, frame: &DecodedFrame) {
        assert_eq!(
            frame.width, self.width,
            "Frame width {} doesn't match texture width {}",
            frame.width, self.width
        );
        assert_eq!(
            frame.height, self.height,
            "Frame height {} doesn't match texture height {}",
            frame.height, self.height
        );

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &frame.data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(frame.stride() as u32),
                rows_per_image: Some(self.height),
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Upload raw RGBA data to the GPU texture
    ///
    /// # Arguments
    /// * `queue` - The wgpu queue
    /// * `data` - RGBA pixel data (4 bytes per pixel)
    /// * `width` - Data width in pixels
    /// * `height` - Data height in pixels
    pub fn upload_raw(&self, queue: &wgpu::Queue, data: &[u8], width: u32, height: u32) {
        assert_eq!(
            width, self.width,
            "Data width {} doesn't match texture width {}",
            width, self.width
        );
        assert_eq!(
            height, self.height,
            "Data height {} doesn't match texture height {}",
            height, self.height
        );
        assert_eq!(
            data.len(),
            (width * height * 4) as usize,
            "Data size mismatch"
        );

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Resize the texture to new dimensions
    ///
    /// This recreates the texture. Note that any bind groups referencing
    /// this texture will need to be recreated.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return; // No change needed
        }

        let (texture, view) = Self::create_texture(device, width, height);

        self.texture = texture;
        self.view = view;
        self.width = width;
        self.height = height;

        log::debug!("Resized video texture to {}x{}", width, height);
    }

    /// Get the texture view
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Get the texture width
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get the texture height
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get the texture format
    pub fn format(&self) -> wgpu::TextureFormat {
        wgpu::TextureFormat::Rgba8UnormSrgb
    }
}

#[cfg(test)]
mod tests {
    // Note: GPU tests would require a wgpu device, which is typically done in integration tests
}
