//! Decoded video frame representation
//!
//! Contains the raw RGBA pixel data and metadata for a decoded video frame.

/// A decoded video frame with RGBA pixel data
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    /// Raw RGBA pixel data (4 bytes per pixel)
    pub data: Vec<u8>,
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Presentation timestamp in seconds
    pub pts: f64,
    /// Frame index (0-based)
    pub frame_index: u64,
}

impl DecodedFrame {
    /// Create a new decoded frame
    pub fn new(data: Vec<u8>, width: u32, height: u32, pts: f64, frame_index: u64) -> Self {
        Self {
            data,
            width,
            height,
            pts,
            frame_index,
        }
    }

    /// Get the expected data size for the frame dimensions (width * height * 4 for RGBA)
    pub fn expected_size(width: u32, height: u32) -> usize {
        (width as usize) * (height as usize) * 4
    }

    /// Check if the frame data has the correct size
    pub fn is_valid(&self) -> bool {
        self.data.len() == Self::expected_size(self.width, self.height)
    }

    /// Get the stride (bytes per row)
    pub fn stride(&self) -> usize {
        (self.width as usize) * 4
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




