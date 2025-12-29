//! HAP video codec decoder
//!
//! HAP is a GPU-accelerated video codec that stores frames in DXT/S3TC compressed
//! texture formats, allowing direct GPU upload without CPU decompression.
//!
//! HAP Variants:
//! - HAP: DXT1 (BC1) - RGB, no alpha, 4:1 compression
//! - HAP Alpha: DXT5 (BC3) - RGBA, with alpha, 4:1 compression  
//! - HAP Q: BC7 - High quality RGBA, 3:1 compression
//!

#![allow(dead_code)]
//! HAP frames can optionally be compressed with Snappy or LZ4 for smaller file sizes.

use anyhow::{anyhow, Result};
use std::path::Path;

/// HAP texture format variants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HapFormat {
    /// DXT1/BC1 - RGB, no alpha
    Hap,
    /// DXT5/BC3 - RGBA with alpha
    HapAlpha,
    /// BC7 - High quality RGBA
    HapQ,
}

impl HapFormat {
    /// Get the wgpu texture format for this HAP variant
    pub fn texture_format(&self) -> wgpu::TextureFormat {
        match self {
            HapFormat::Hap => wgpu::TextureFormat::Bc1RgbaUnorm,
            HapFormat::HapAlpha => wgpu::TextureFormat::Bc3RgbaUnorm,
            HapFormat::HapQ => wgpu::TextureFormat::Bc7RgbaUnorm,
        }
    }

    /// Get bytes per block (4x4 pixels)
    pub fn bytes_per_block(&self) -> usize {
        match self {
            HapFormat::Hap => 8,      // BC1: 8 bytes per 4x4 block
            HapFormat::HapAlpha => 16, // BC3: 16 bytes per 4x4 block
            HapFormat::HapQ => 16,     // BC7: 16 bytes per 4x4 block
        }
    }

    /// Parse from HAP codec fourcc/type byte
    pub fn from_hap_type(hap_type: u8) -> Option<Self> {
        // HAP uses specific type bytes in the frame header
        match hap_type & 0x0F {
            0x01 => Some(HapFormat::Hap),       // DXT1
            0x02 => Some(HapFormat::HapAlpha),  // DXT5
            0x03 => Some(HapFormat::HapQ),      // BC7
            _ => None,
        }
    }
}

/// Compression type used for HAP frame data
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HapCompression {
    /// No additional compression
    None,
    /// Snappy compression
    Snappy,
    /// LZ4 compression (HAP Q often uses this)
    Lz4,
}

impl HapCompression {
    /// Parse from HAP header byte
    pub fn from_hap_header(byte: u8) -> Self {
        match (byte >> 4) & 0x0F {
            0x00 => HapCompression::None,
            0x01 => HapCompression::Snappy,
            0x02 => HapCompression::Lz4,
            _ => HapCompression::None,
        }
    }
}

/// A decoded HAP frame ready for GPU upload
#[derive(Debug)]
pub struct HapFrame {
    /// Raw DXT/BC texture data
    pub data: Vec<u8>,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// HAP format variant
    pub format: HapFormat,
    /// Frame timestamp in seconds
    pub timestamp: f64,
    /// Frame number
    pub frame_number: u64,
}

impl HapFrame {
    /// Calculate the expected data size for this frame
    pub fn expected_size(&self) -> usize {
        let blocks_wide = (self.width as usize + 3) / 4;
        let blocks_high = (self.height as usize + 3) / 4;
        blocks_wide * blocks_high * self.format.bytes_per_block()
    }

    /// Create a GPU texture from this frame
    pub fn create_texture(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Texture {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HAP Frame Texture"),
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format.texture_format(),
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let blocks_wide = (self.width + 3) / 4;
        let bytes_per_row = blocks_wide * self.format.bytes_per_block() as u32;

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.data,
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

        texture
    }
}

/// HAP video decoder
///
/// Reads HAP-encoded video files and extracts frames as GPU-ready texture data.
#[derive(Debug)]
pub struct HapDecoder {
    /// Video width
    pub width: u32,
    /// Video height
    pub height: u32,
    /// HAP format variant
    pub format: HapFormat,
    /// Frame rate
    pub frame_rate: f64,
    /// Total number of frames
    pub frame_count: u64,
    /// Duration in seconds
    pub duration: f64,
    /// Current frame index
    current_frame: u64,
    /// Frame data storage (simplified - in production would use proper demuxer)
    frames: Vec<HapFrameData>,
}

/// Internal frame data storage
#[derive(Debug)]
struct HapFrameData {
    offset: u64,
    size: u32,
    compressed: HapCompression,
}

impl HapDecoder {
    /// Create a new HAP decoder for the given file
    pub fn new(path: &Path) -> Result<Self> {
        log::info!("Opening HAP file: {:?}", path);
        
        // For now, create a placeholder decoder
        // In a full implementation, this would parse the MOV/AVI container
        // and extract HAP frame locations
        
        // Check file extension
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());
        
        match ext.as_deref() {
            Some("mov") | Some("avi") | Some("mp4") => {}
            _ => return Err(anyhow!("Unsupported file format. Expected .mov, .avi, or .mp4")),
        }

        // Create placeholder with default values
        // Real implementation would parse container metadata
        Ok(Self {
            width: 1920,
            height: 1080,
            format: HapFormat::Hap,
            frame_rate: 30.0,
            frame_count: 0,
            duration: 0.0,
            current_frame: 0,
            frames: Vec::new(),
        })
    }

    /// Create a decoder with test pattern data
    pub fn new_test_pattern(width: u32, height: u32, format: HapFormat) -> Self {
        Self {
            width,
            height,
            format,
            frame_rate: 30.0,
            frame_count: 1,
            duration: 1.0 / 30.0,
            current_frame: 0,
            frames: Vec::new(),
        }
    }

    /// Seek to a specific frame
    pub fn seek_to_frame(&mut self, frame: u64) -> Result<()> {
        if frame >= self.frame_count && self.frame_count > 0 {
            return Err(anyhow!("Frame {} out of range (max: {})", frame, self.frame_count - 1));
        }
        self.current_frame = frame;
        Ok(())
    }

    /// Seek to a specific time
    pub fn seek_to_time(&mut self, time: f64) -> Result<()> {
        let frame = (time * self.frame_rate) as u64;
        self.seek_to_frame(frame)
    }

    /// Decode the next frame
    pub fn decode_next(&mut self) -> Result<Option<HapFrame>> {
        if self.frame_count == 0 {
            // Return test pattern if no real frames loaded
            return Ok(Some(self.generate_test_frame()));
        }

        if self.current_frame >= self.frame_count {
            return Ok(None);
        }

        let frame = self.decode_frame(self.current_frame)?;
        self.current_frame += 1;
        Ok(Some(frame))
    }

    /// Decode a specific frame
    pub fn decode_frame(&self, frame_number: u64) -> Result<HapFrame> {
        // In a real implementation, this would:
        // 1. Read compressed data from the container
        // 2. Decompress with Snappy/LZ4 if needed
        // 3. Return the raw DXT data
        
        // For now, generate test pattern
        Ok(self.generate_test_frame_at(frame_number))
    }

    /// Generate a test pattern frame
    fn generate_test_frame(&self) -> HapFrame {
        self.generate_test_frame_at(self.current_frame)
    }

    /// Generate a test pattern frame at a specific frame number
    fn generate_test_frame_at(&self, frame_number: u64) -> HapFrame {
        let blocks_wide = (self.width as usize + 3) / 4;
        let blocks_high = (self.height as usize + 3) / 4;
        let block_size = self.format.bytes_per_block();
        let data_size = blocks_wide * blocks_high * block_size;
        
        // Generate a simple gradient pattern in DXT format
        let mut data = vec![0u8; data_size];
        
        for by in 0..blocks_high {
            for bx in 0..blocks_wide {
                let idx = (by * blocks_wide + bx) * block_size;
                
                // Create a simple color based on position and frame
                let r = ((bx as f32 / blocks_wide as f32) * 255.0) as u8;
                let g = ((by as f32 / blocks_high as f32) * 255.0) as u8;
                let b = ((frame_number as f32 / 60.0).sin() * 127.0 + 128.0) as u8;
                
                // Write BC1 block (simplified - real BC1 is more complex)
                match self.format {
                    HapFormat::Hap => {
                        // BC1 format: 2 16-bit colors + 4 bytes of indices
                        let color0 = encode_rgb565(r, g, b);
                        let color1 = encode_rgb565(r / 2, g / 2, b / 2);
                        data[idx..idx + 2].copy_from_slice(&color0.to_le_bytes());
                        data[idx + 2..idx + 4].copy_from_slice(&color1.to_le_bytes());
                        // All pixels use color0
                        data[idx + 4..idx + 8].fill(0x00);
                    }
                    HapFormat::HapAlpha | HapFormat::HapQ => {
                        // BC3/BC7 format (simplified)
                        // Alpha block
                        data[idx] = 255;
                        data[idx + 1] = 255;
                        data[idx + 2..idx + 8].fill(0x00);
                        // Color block
                        let color0 = encode_rgb565(r, g, b);
                        let color1 = encode_rgb565(r / 2, g / 2, b / 2);
                        data[idx + 8..idx + 10].copy_from_slice(&color0.to_le_bytes());
                        data[idx + 10..idx + 12].copy_from_slice(&color1.to_le_bytes());
                        data[idx + 12..idx + 16].fill(0x00);
                    }
                }
            }
        }

        HapFrame {
            data,
            width: self.width,
            height: self.height,
            format: self.format,
            timestamp: frame_number as f64 / self.frame_rate,
            frame_number,
        }
    }

    /// Get the current playback position in seconds
    pub fn current_time(&self) -> f64 {
        self.current_frame as f64 / self.frame_rate
    }
}

/// Encode RGB to RGB565 format
fn encode_rgb565(r: u8, g: u8, b: u8) -> u16 {
    let r5 = (r as u16 >> 3) & 0x1F;
    let g6 = (g as u16 >> 2) & 0x3F;
    let b5 = (b as u16 >> 3) & 0x1F;
    (r5 << 11) | (g6 << 5) | b5
}

/// Decompress Snappy-compressed data
pub fn decompress_snappy(compressed: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = snap::raw::Decoder::new();
    decoder
        .decompress_vec(compressed)
        .map_err(|e| anyhow!("Snappy decompression failed: {}", e))
}

/// Decompress LZ4-compressed data
pub fn decompress_lz4(compressed: &[u8], uncompressed_size: usize) -> Result<Vec<u8>> {
    lz4_flex::decompress(compressed, uncompressed_size)
        .map_err(|e| anyhow!("LZ4 decompression failed: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hap_format_bytes_per_block() {
        assert_eq!(HapFormat::Hap.bytes_per_block(), 8);
        assert_eq!(HapFormat::HapAlpha.bytes_per_block(), 16);
        assert_eq!(HapFormat::HapQ.bytes_per_block(), 16);
    }

    #[test]
    fn test_generate_test_frame() {
        let decoder = HapDecoder::new_test_pattern(1920, 1080, HapFormat::Hap);
        let frame = decoder.generate_test_frame();
        assert_eq!(frame.width, 1920);
        assert_eq!(frame.height, 1080);
        assert_eq!(frame.data.len(), frame.expected_size());
    }
}

