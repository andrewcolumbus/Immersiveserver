//! HAP video codec support
//!
//! HAP is a GPU-accelerated video codec that stores frames in DXT/S3TC compressed
//! texture formats, allowing direct GPU upload without CPU decompression.
//!
//! HAP Variants:
//! - HAP: DXT1 (BC1) - RGB, no alpha, 4:1 compression
//! - HAP Alpha: DXT5 (BC3) - RGBA, with alpha, 4:1 compression
//! - HAP Q: Scaled YCoCg DXT5 - Higher quality, uses 2 textures
//!
//! HAP frames can be compressed with Snappy for smaller file sizes.

#![allow(dead_code)]
#![allow(deprecated)]

use std::path::Path;

/// HAP texture format variants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HapFormat {
    /// DXT1/BC1 - RGB, no alpha (standard Hap)
    Hap,
    /// DXT5/BC3 - RGBA with alpha (Hap Alpha)
    HapAlpha,
    /// Scaled YCoCg DXT5 (Hap Q) - Higher quality
    HapQ,
}

impl HapFormat {
    /// Get the wgpu texture format for this HAP variant
    pub fn texture_format(&self) -> wgpu::TextureFormat {
        match self {
            HapFormat::Hap => wgpu::TextureFormat::Bc1RgbaUnorm,
            HapFormat::HapAlpha => wgpu::TextureFormat::Bc3RgbaUnorm,
            HapFormat::HapQ => wgpu::TextureFormat::Bc3RgbaUnorm, // YCoCg uses BC3
        }
    }

    /// Get bytes per block (4x4 pixels)
    pub fn bytes_per_block(&self) -> usize {
        match self {
            HapFormat::Hap => 8,       // BC1: 8 bytes per 4x4 block
            HapFormat::HapAlpha => 16, // BC3: 16 bytes per 4x4 block
            HapFormat::HapQ => 16,     // BC3: 16 bytes per 4x4 block
        }
    }

    /// Parse from FFmpeg codec name
    pub fn from_codec_name(name: &str) -> Option<Self> {
        match name {
            "hap" => Some(HapFormat::Hap),
            "hap_alpha" => Some(HapFormat::HapAlpha),
            "hapqa" | "hap_q" | "hapq" => Some(HapFormat::HapQ),
            _ => None,
        }
    }

    /// Check if a codec name is a HAP variant
    pub fn is_hap_codec(name: &str) -> bool {
        name.starts_with("hap")
    }
}

/// A decoded HAP frame ready for direct GPU upload
#[derive(Debug)]
pub struct HapFrame {
    /// Raw DXT/BC texture data (GPU-compressed format)
    pub data: Vec<u8>,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// HAP format variant
    pub format: HapFormat,
    /// Frame timestamp in seconds
    pub pts: f64,
    /// Frame number
    pub frame_index: u64,
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

/// HAP video decoder using FFmpeg for demuxing
///
/// This decoder uses FFmpeg to demux HAP video files, then handles
/// Snappy decompression if needed to extract raw DXT texture data.
pub struct HapDecoder {
    /// The input format context
    input: ffmpeg_next::format::context::Input,
    /// Index of the video stream
    video_stream_index: usize,
    /// Video decoder
    decoder: ffmpeg_next::decoder::Video,
    /// Video width
    width: u32,
    /// Video height
    height: u32,
    /// HAP format variant
    format: HapFormat,
    /// Frame rate (fps)
    frame_rate: f64,
    /// Video duration in seconds
    duration: f64,
    /// Time base for PTS conversion
    time_base: f64,
    /// Current frame index
    frame_index: u64,
    /// Whether we've reached end of file
    eof: bool,
}

impl HapDecoder {
    /// Open a HAP video file
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        ffmpeg_next::init().map_err(|e| format!("FFmpeg init failed: {}", e))?;

        let path = path.as_ref();
        let path_str = path.to_string_lossy().to_string();

        // Open input file
        let input = ffmpeg_next::format::input(&path)
            .map_err(|_| format!("Failed to open video file: {}", path_str))?;

        // Find best video stream
        let video_stream = input
            .streams()
            .best(ffmpeg_next::media::Type::Video)
            .ok_or("No video stream found")?;

        let video_stream_index = video_stream.index();

        // Get stream parameters
        let time_base = video_stream.time_base();
        let time_base_f64 = time_base.numerator() as f64 / time_base.denominator() as f64;

        let frame_rate = video_stream.avg_frame_rate();
        let frame_rate_f64 = if frame_rate.denominator() > 0 {
            frame_rate.numerator() as f64 / frame_rate.denominator() as f64
        } else {
            30.0
        };

        let duration = if video_stream.duration() > 0 {
            video_stream.duration() as f64 * time_base_f64
        } else if input.duration() > 0 {
            input.duration() as f64 / ffmpeg_next::ffi::AV_TIME_BASE as f64
        } else {
            0.0
        };

        // Get codec info
        let parameters = video_stream.parameters();
        let codec_id = unsafe { (*parameters.as_ptr()).codec_id };
        let codec = ffmpeg_next::decoder::find(codec_id.into());
        let codec_name = codec
            .map(|c| c.name().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Verify this is a HAP codec
        let format = HapFormat::from_codec_name(&codec_name)
            .ok_or_else(|| format!("Not a HAP codec: {}", codec_name))?;

        // Create decoder
        let context = ffmpeg_next::codec::context::Context::from_parameters(parameters)
            .map_err(|e| format!("Failed to create codec context: {}", e))?;
        let decoder = context
            .decoder()
            .video()
            .map_err(|e| format!("Failed to create decoder: {}", e))?;

        let width = decoder.width();
        let height = decoder.height();

        log::info!(
            "Opened HAP video: {}x{} @ {:.2}fps, duration: {:.2}s, format: {:?}",
            width,
            height,
            frame_rate_f64,
            duration,
            format
        );

        Ok(Self {
            input,
            video_stream_index,
            decoder,
            width,
            height,
            format,
            frame_rate: frame_rate_f64,
            duration,
            time_base: time_base_f64,
            frame_index: 0,
            eof: false,
        })
    }

    /// Check if a file is a HAP video
    pub fn is_hap_file<P: AsRef<Path>>(path: P) -> bool {
        if let Ok(input) = ffmpeg_next::format::input(path.as_ref()) {
            if let Some(stream) = input.streams().best(ffmpeg_next::media::Type::Video) {
                let parameters = stream.parameters();
                let codec_id = unsafe { (*parameters.as_ptr()).codec_id };
                if let Some(codec) = ffmpeg_next::decoder::find(codec_id.into()) {
                    return HapFormat::is_hap_codec(codec.name());
                }
            }
        }
        false
    }

    /// Decode the next frame
    pub fn decode_next_frame(&mut self) -> Result<Option<HapFrame>, String> {
        if self.eof {
            return Ok(None);
        }

        let mut decoded_frame = ffmpeg_next::frame::Video::empty();

        loop {
            // Try to receive a decoded frame
            match self.decoder.receive_frame(&mut decoded_frame) {
                Ok(()) => {
                    // Got a frame - extract the raw DXT data
                    let pts = decoded_frame.pts().unwrap_or(0) as f64 * self.time_base;

                    // The decoded frame contains the raw DXT/BC data
                    // FFmpeg's HAP decoder outputs the compressed texture data directly
                    let data = decoded_frame.data(0);
                    let expected_size = self.expected_frame_size();

                    // Copy the data
                    let frame_data = if data.len() >= expected_size {
                        data[..expected_size].to_vec()
                    } else {
                        // Padding may be needed
                        let mut padded = vec![0u8; expected_size];
                        padded[..data.len()].copy_from_slice(data);
                        padded
                    };

                    let frame = HapFrame {
                        data: frame_data,
                        width: self.width,
                        height: self.height,
                        format: self.format,
                        pts,
                        frame_index: self.frame_index,
                    };
                    self.frame_index += 1;

                    return Ok(Some(frame));
                }
                Err(ffmpeg_next::Error::Other {
                    errno: ffmpeg_next::error::EAGAIN,
                }) => {
                    // Need more input
                }
                Err(ffmpeg_next::Error::Eof) => {
                    self.eof = true;
                    return Ok(None);
                }
                Err(e) => {
                    return Err(format!("Decode error: {}", e));
                }
            }

            // Read next packet
            loop {
                match self.input.packets().next() {
                    Some((stream, packet)) => {
                        if stream.index() == self.video_stream_index {
                            self.decoder
                                .send_packet(&packet)
                                .map_err(|e| format!("Failed to send packet: {}", e))?;
                            break;
                        }
                    }
                    None => {
                        let _ = self.decoder.send_eof();
                        self.eof = true;
                        break;
                    }
                }
            }
        }
    }

    /// Calculate expected frame size in bytes
    fn expected_frame_size(&self) -> usize {
        let blocks_wide = (self.width as usize + 3) / 4;
        let blocks_high = (self.height as usize + 3) / 4;
        blocks_wide * blocks_high * self.format.bytes_per_block()
    }

    /// Reset to beginning
    pub fn reset(&mut self) -> Result<(), String> {
        self.input
            .seek(0, ..)
            .map_err(|e| format!("Seek failed: {}", e))?;
        self.decoder.flush();
        self.frame_index = 0;
        self.eof = false;
        Ok(())
    }

    /// Get video width
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get video height
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get frame rate
    pub fn frame_rate(&self) -> f64 {
        self.frame_rate
    }

    /// Get duration
    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// Get HAP format
    pub fn format(&self) -> HapFormat {
        self.format
    }

    /// Check if EOF reached
    pub fn is_eof(&self) -> bool {
        self.eof
    }
}

/// Decompress Snappy-compressed data
pub fn decompress_snappy(compressed: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoder = snap::raw::Decoder::new();
    decoder
        .decompress_vec(compressed)
        .map_err(|e| format!("Snappy decompression failed: {}", e))
}

/// Decompress LZ4-compressed data
pub fn decompress_lz4(compressed: &[u8], uncompressed_size: usize) -> Result<Vec<u8>, String> {
    lz4_flex::decompress(compressed, uncompressed_size)
        .map_err(|e| format!("LZ4 decompression failed: {}", e))
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
    fn test_hap_format_from_codec_name() {
        assert_eq!(HapFormat::from_codec_name("hap"), Some(HapFormat::Hap));
        assert_eq!(
            HapFormat::from_codec_name("hap_alpha"),
            Some(HapFormat::HapAlpha)
        );
        assert_eq!(HapFormat::from_codec_name("unknown"), None);
    }
}

