//! GPU texture pool for efficient frame management
//!
//! Manages a pool of GPU textures for double/triple buffering during video playback.

#![allow(dead_code)]

use wgpu;

/// Configuration for the texture pool
#[derive(Debug, Clone)]
pub struct TexturePoolConfig {
    /// Number of textures to keep in the pool
    pub pool_size: usize,
    /// Texture width
    pub width: u32,
    /// Texture height
    pub height: u32,
    /// Texture format
    pub format: wgpu::TextureFormat,
}

impl Default for TexturePoolConfig {
    fn default() -> Self {
        Self {
            pool_size: 3,
            width: 1920,
            height: 1080,
            format: wgpu::TextureFormat::Bc1RgbaUnorm,
        }
    }
}

/// A pooled texture with its view
pub struct PooledTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    /// Whether this texture is currently in use
    pub in_use: bool,
}

impl PooledTexture {
    /// Create a new pooled texture
    pub fn new(device: &wgpu::Device, config: &TexturePoolConfig, label: &str) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: config.format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture,
            view,
            width: config.width,
            height: config.height,
            format: config.format,
            in_use: false,
        }
    }

    /// Upload data to this texture
    pub fn upload(&self, queue: &wgpu::Queue, data: &[u8], bytes_per_block: u32) {
        let blocks_wide = (self.width + 3) / 4;
        let bytes_per_row = blocks_wide * bytes_per_block;

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }
}

/// Pool of GPU textures for video playback
pub struct TexturePool {
    textures: Vec<PooledTexture>,
    config: TexturePoolConfig,
    current_index: usize,
}

impl TexturePool {
    /// Create a new texture pool
    pub fn new(device: &wgpu::Device, config: TexturePoolConfig) -> Self {
        let textures = (0..config.pool_size)
            .map(|i| PooledTexture::new(device, &config, &format!("Pool Texture {}", i)))
            .collect();

        Self {
            textures,
            config,
            current_index: 0,
        }
    }

    /// Get the next available texture from the pool
    pub fn acquire(&mut self) -> Option<&mut PooledTexture> {
        // Find a texture that's not in use
        for i in 0..self.textures.len() {
            let idx = (self.current_index + i) % self.textures.len();
            if !self.textures[idx].in_use {
                self.textures[idx].in_use = true;
                self.current_index = (idx + 1) % self.textures.len();
                return Some(&mut self.textures[idx]);
            }
        }
        None
    }

    /// Release a texture back to the pool
    pub fn release(&mut self, index: usize) {
        if index < self.textures.len() {
            self.textures[index].in_use = false;
        }
    }

    /// Release all textures
    pub fn release_all(&mut self) {
        for texture in &mut self.textures {
            texture.in_use = false;
        }
    }

    /// Get a texture by index
    pub fn get(&self, index: usize) -> Option<&PooledTexture> {
        self.textures.get(index)
    }

    /// Get the current configuration
    pub fn config(&self) -> &TexturePoolConfig {
        &self.config
    }

    /// Resize the pool for new video dimensions
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32, format: wgpu::TextureFormat) {
        self.config.width = width;
        self.config.height = height;
        self.config.format = format;
        
        // Recreate all textures
        self.textures = (0..self.config.pool_size)
            .map(|i| PooledTexture::new(device, &self.config, &format!("Pool Texture {}", i)))
            .collect();
        
        self.current_index = 0;
    }

    /// Get the number of textures in the pool
    pub fn len(&self) -> usize {
        self.textures.len()
    }

    /// Check if the pool is empty
    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }
}

impl Default for TexturePool {
    fn default() -> Self {
        // Create a minimal pool without actual GPU textures
        Self {
            textures: Vec::new(),
            config: TexturePoolConfig::default(),
            current_index: 0,
        }
    }
}

