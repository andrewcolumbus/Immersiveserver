//! Gray code pattern decoder.

/// Result of decoding Gray code patterns.
#[derive(Debug, Clone)]
pub struct DecodedCorrespondences {
    /// Camera image dimensions.
    pub camera_width: u32,
    pub camera_height: u32,
    /// Projector dimensions.
    pub projector_width: u32,
    pub projector_height: u32,
    /// Per-pixel decoded X coordinate in projector space (-1 = invalid).
    pub projector_x: Vec<i32>,
    /// Per-pixel decoded Y coordinate in projector space (-1 = invalid).
    pub projector_y: Vec<i32>,
    /// Confidence mask (0.0-1.0 per pixel).
    pub confidence: Vec<f32>,
    /// Shadow/occlusion mask (true = valid pixel).
    pub valid_mask: Vec<bool>,
}

impl DecodedCorrespondences {
    pub fn new(camera_width: u32, camera_height: u32, projector_width: u32, projector_height: u32) -> Self {
        let size = (camera_width * camera_height) as usize;
        Self {
            camera_width,
            camera_height,
            projector_width,
            projector_height,
            projector_x: vec![-1; size],
            projector_y: vec![-1; size],
            confidence: vec![0.0; size],
            valid_mask: vec![false; size],
        }
    }

    /// Get correspondence at camera pixel (x, y).
    pub fn get(&self, x: u32, y: u32) -> Option<(f32, f32)> {
        let idx = (y * self.camera_width + x) as usize;
        if self.valid_mask[idx] {
            Some((self.projector_x[idx] as f32, self.projector_y[idx] as f32))
        } else {
            None
        }
    }

    /// Count valid correspondences.
    pub fn valid_count(&self) -> usize {
        self.valid_mask.iter().filter(|&&v| v).count()
    }
}
