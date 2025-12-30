//! Decoded video frame representation
//!
//! Contains the raw pixel data and metadata for a decoded video frame.
//! Supports both RGBA (software decoded) and DXT/BC (GPU-native) formats.

/// A decoded video frame with pixel data
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    /// Raw pixel data - either RGBA (4 bytes/pixel) or DXT/BC compressed
    pub data: Vec<u8>,
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Presentation timestamp in seconds
    pub pts: f64,
    /// Frame index (0-based)
    pub frame_index: u64,
    /// Whether this is a GPU-native frame (DXT/BC compressed)
    pub is_gpu_native: bool,
    /// For GPU-native frames: true = BC3/DXT5, false = BC1/DXT1
    pub is_bc3: bool,
}

impl DecodedFrame {
    /// Create a new RGBA decoded frame
    pub fn new(data: Vec<u8>, width: u32, height: u32, pts: f64, frame_index: u64) -> Self {
        Self {
            data,
            width,
            height,
            pts,
            frame_index,
            is_gpu_native: false,
            is_bc3: false,
        }
    }
    
    /// Create a new GPU-native (DXT/BC) frame
    pub fn new_gpu_native(data: Vec<u8>, width: u32, height: u32, pts: f64, frame_index: u64, is_bc3: bool) -> Self {
        Self {
            data,
            width,
            height,
            pts,
            frame_index,
            is_gpu_native: true,
            is_bc3,
        }
    }

    /// Get the expected data size for RGBA frame dimensions (width * height * 4)
    pub fn expected_size(width: u32, height: u32) -> usize {
        (width as usize) * (height as usize) * 4
    }
    
    /// Get the expected data size for DXT/BC frame
    pub fn expected_dxt_size(width: u32, height: u32, is_bc3: bool) -> usize {
        let blocks_wide = (width as usize + 3) / 4;
        let blocks_high = (height as usize + 3) / 4;
        let bytes_per_block = if is_bc3 { 16 } else { 8 };
        blocks_wide * blocks_high * bytes_per_block
    }

    /// Check if the frame data has the correct size
    pub fn is_valid(&self) -> bool {
        if self.is_gpu_native {
            // For DXT, just check we have some data (size varies by format)
            !self.data.is_empty()
        } else {
            self.data.len() == Self::expected_size(self.width, self.height)
        }
    }

    /// Get the stride (bytes per row) - only valid for RGBA frames
    pub fn stride(&self) -> usize {
        if self.is_gpu_native {
            // For DXT, return bytes per block row
            let blocks_wide = (self.width as usize + 3) / 4;
            let bytes_per_block = if self.is_bc3 { 16 } else { 8 };
            blocks_wide * bytes_per_block
        } else {
            (self.width as usize) * 4
        }
    }
    
    /// Get the wgpu texture format for this frame
    pub fn texture_format(&self) -> wgpu::TextureFormat {
        if self.is_gpu_native {
            // Use sRGB variants to match video color space
            if self.is_bc3 {
                wgpu::TextureFormat::Bc3RgbaUnormSrgb
            } else {
                wgpu::TextureFormat::Bc1RgbaUnormSrgb
            }
        } else {
            wgpu::TextureFormat::Rgba8UnormSrgb
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_creation() {
        let width = 1920;
        let height = 1080;
        let data = vec![0u8; DecodedFrame::expected_size(width, height)];
        let frame = DecodedFrame::new(data, width, height, 0.0, 0);
        
        assert_eq!(frame.width, 1920);
        assert_eq!(frame.height, 1080);
        assert!(frame.is_valid());
        assert_eq!(frame.stride(), 1920 * 4);
    }

    #[test]
    fn test_expected_size() {
        assert_eq!(DecodedFrame::expected_size(1920, 1080), 1920 * 1080 * 4);
        assert_eq!(DecodedFrame::expected_size(1280, 720), 1280 * 720 * 4);
    }
}




