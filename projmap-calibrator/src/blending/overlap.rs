//! Automatic overlap detection from calibration data.
//!
//! Detects overlapping regions between projectors by finding camera pixels
//! that map to valid coordinates in multiple projectors.

use super::{BlendMask, OverlapEdge, OverlapRegion};
use crate::calibration::ProjectorCalibration;
use crate::config::BlendCurve;

/// Result of overlap detection.
#[derive(Debug, Clone)]
pub struct OverlapDetectionResult {
    /// Detected overlap regions between projector pairs.
    pub overlaps: Vec<OverlapRegion>,
    /// Per-projector blend masks.
    pub blend_masks: Vec<BlendMask>,
}

/// Configuration for overlap detection.
#[derive(Debug, Clone)]
pub struct OverlapConfig {
    /// Minimum overlap width to consider valid (pixels).
    pub min_overlap_width: u32,
    /// Blend curve to use for generated masks.
    pub blend_curve: BlendCurve,
    /// Padding to add to detected overlap (pixels).
    pub padding: u32,
}

impl Default for OverlapConfig {
    fn default() -> Self {
        Self {
            min_overlap_width: 10,
            blend_curve: BlendCurve::Smoothstep,
            padding: 0,
        }
    }
}

/// Overlap detector that analyzes calibration data.
pub struct OverlapDetector {
    config: OverlapConfig,
}

impl OverlapDetector {
    pub fn new(config: OverlapConfig) -> Self {
        Self { config }
    }

    /// Detect overlaps between all projector pairs.
    pub fn detect(&self, projectors: &[ProjectorCalibration]) -> OverlapDetectionResult {
        let mut overlaps = Vec::new();
        let mut blend_masks: Vec<BlendMask> = projectors
            .iter()
            .map(|p| BlendMask::new(p.projector_width, p.projector_height))
            .collect();

        // Compare each pair of projectors
        for i in 0..projectors.len() {
            for j in (i + 1)..projectors.len() {
                if let Some(overlap) = self.detect_pair(&projectors[i], &projectors[j]) {
                    // Apply blend to both projectors using split_at_mut to avoid double borrow
                    let (left, right) = blend_masks.split_at_mut(j);
                    self.apply_overlap_blend(&mut left[i], &mut right[0], &overlap);
                    overlaps.push(overlap);
                }
            }
        }

        OverlapDetectionResult {
            overlaps,
            blend_masks,
        }
    }

    /// Detect overlap between two projectors using their correspondences.
    fn detect_pair(
        &self,
        proj_a: &ProjectorCalibration,
        proj_b: &ProjectorCalibration,
    ) -> Option<OverlapRegion> {
        let corr_a = proj_a.correspondences.as_ref()?;
        let corr_b = proj_b.correspondences.as_ref()?;

        // Ensure camera dimensions match
        if corr_a.camera_width != corr_b.camera_width || corr_a.camera_height != corr_b.camera_height {
            log::warn!("Camera dimensions don't match between projectors");
            return None;
        }

        let camera_pixels = (corr_a.camera_width * corr_a.camera_height) as usize;

        // Find camera pixels that are valid in both projectors
        let mut overlap_pixels_a: Vec<(i32, i32)> = Vec::new(); // Projector A coords
        let mut overlap_pixels_b: Vec<(i32, i32)> = Vec::new(); // Projector B coords

        for i in 0..camera_pixels {
            if corr_a.valid_mask[i] && corr_b.valid_mask[i] {
                let ax = corr_a.projector_x[i];
                let ay = corr_a.projector_y[i];
                let bx = corr_b.projector_x[i];
                let by = corr_b.projector_y[i];

                // Valid coordinates
                if ax >= 0 && ay >= 0 && bx >= 0 && by >= 0 {
                    overlap_pixels_a.push((ax, ay));
                    overlap_pixels_b.push((bx, by));
                }
            }
        }

        if overlap_pixels_a.is_empty() {
            log::info!(
                "No overlap detected between projector {} and {}",
                proj_a.projector_id,
                proj_b.projector_id
            );
            return None;
        }

        log::info!(
            "Found {} overlapping camera pixels between projector {} and {}",
            overlap_pixels_a.len(),
            proj_a.projector_id,
            proj_b.projector_id
        );

        // Determine overlap region bounds in each projector
        let bounds_a = self.compute_bounds(&overlap_pixels_a);
        let _bounds_b = self.compute_bounds(&overlap_pixels_b);

        // Determine which edge of projector A overlaps with projector B
        let edge = self.determine_overlap_edge(
            &bounds_a,
            proj_a.projector_width,
            proj_a.projector_height,
        );

        // Calculate overlap width based on the detected edge
        let overlap_width = match edge {
            OverlapEdge::Left => bounds_a.max_x - bounds_a.min_x,
            OverlapEdge::Right => bounds_a.max_x - bounds_a.min_x,
            OverlapEdge::Top => bounds_a.max_y - bounds_a.min_y,
            OverlapEdge::Bottom => bounds_a.max_y - bounds_a.min_y,
        };

        let overlap_width = (overlap_width as u32).saturating_add(self.config.padding);

        if overlap_width < self.config.min_overlap_width {
            log::info!(
                "Overlap width {} is below minimum {}, ignoring",
                overlap_width,
                self.config.min_overlap_width
            );
            return None;
        }

        log::info!(
            "Detected {:?} overlap of {} pixels between projector {} and {}",
            edge,
            overlap_width,
            proj_a.projector_id,
            proj_b.projector_id
        );

        Some(OverlapRegion {
            projector_a: proj_a.projector_id,
            projector_b: proj_b.projector_id,
            overlap_width,
            edge,
        })
    }

    /// Compute bounding box of overlap pixels in projector space.
    fn compute_bounds(&self, pixels: &[(i32, i32)]) -> Bounds {
        let mut bounds = Bounds {
            min_x: i32::MAX,
            max_x: i32::MIN,
            min_y: i32::MAX,
            max_y: i32::MIN,
        };

        for &(x, y) in pixels {
            bounds.min_x = bounds.min_x.min(x);
            bounds.max_x = bounds.max_x.max(x);
            bounds.min_y = bounds.min_y.min(y);
            bounds.max_y = bounds.max_y.max(y);
        }

        bounds
    }

    /// Determine which edge of the projector the overlap is on.
    fn determine_overlap_edge(&self, bounds: &Bounds, proj_width: u32, proj_height: u32) -> OverlapEdge {
        let center_x = (bounds.min_x + bounds.max_x) / 2;
        let center_y = (bounds.min_y + bounds.max_y) / 2;
        let half_w = proj_width as i32 / 2;
        let half_h = proj_height as i32 / 2;

        // Determine which edge by comparing center position to projector center
        let dx = center_x - half_w;
        let dy = center_y - half_h;

        // Normalize by aspect ratio for fair comparison
        let dx_norm = dx.abs() as f32 / proj_width as f32;
        let dy_norm = dy.abs() as f32 / proj_height as f32;

        if dx_norm > dy_norm {
            // Horizontal overlap
            if dx > 0 {
                OverlapEdge::Right
            } else {
                OverlapEdge::Left
            }
        } else {
            // Vertical overlap
            if dy > 0 {
                OverlapEdge::Bottom
            } else {
                OverlapEdge::Top
            }
        }
    }

    /// Apply overlap blend to both projectors' masks.
    fn apply_overlap_blend(
        &self,
        mask_a: &mut BlendMask,
        mask_b: &mut BlendMask,
        overlap: &OverlapRegion,
    ) {
        let curve = self.config.blend_curve;
        let width = overlap.overlap_width;

        // Apply complementary blends to ensure sum = 1.0 in overlap
        match overlap.edge {
            OverlapEdge::Right => {
                mask_a.apply_right_blend(width, curve);
                mask_b.apply_left_blend(width, curve);
            }
            OverlapEdge::Left => {
                mask_a.apply_left_blend(width, curve);
                mask_b.apply_right_blend(width, curve);
            }
            OverlapEdge::Top => {
                self.apply_top_blend(mask_a, width, curve);
                self.apply_bottom_blend(mask_b, width, curve);
            }
            OverlapEdge::Bottom => {
                self.apply_bottom_blend(mask_a, width, curve);
                self.apply_top_blend(mask_b, width, curve);
            }
        }
    }

    /// Apply blend falloff on the top edge.
    fn apply_top_blend(&self, mask: &mut BlendMask, blend_height: u32, curve: BlendCurve) {
        if blend_height == 0 {
            return;
        }

        mask.curve = curve;

        for y in 0..blend_height.min(mask.height) {
            let t = 1.0 - (y as f32 / blend_height as f32);
            let blend = 1.0 - BlendMask::apply_curve(t, curve);
            for x in 0..mask.width {
                let idx = (y * mask.width + x) as usize;
                mask.data[idx] *= blend;
            }
        }
    }

    /// Apply blend falloff on the bottom edge.
    fn apply_bottom_blend(&self, mask: &mut BlendMask, blend_height: u32, curve: BlendCurve) {
        if blend_height == 0 {
            return;
        }

        mask.curve = curve;
        let start_y = mask.height.saturating_sub(blend_height);

        for y in start_y..mask.height {
            let t = (y - start_y) as f32 / blend_height as f32;
            let blend = 1.0 - BlendMask::apply_curve(t, curve);
            for x in 0..mask.width {
                let idx = (y * mask.width + x) as usize;
                mask.data[idx] *= blend;
            }
        }
    }
}

/// Bounding box helper.
#[derive(Debug, Clone, Copy)]
struct Bounds {
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounds_computation() {
        let detector = OverlapDetector::new(OverlapConfig::default());
        let pixels = vec![(10, 20), (100, 50), (50, 80)];
        let bounds = detector.compute_bounds(&pixels);

        assert_eq!(bounds.min_x, 10);
        assert_eq!(bounds.max_x, 100);
        assert_eq!(bounds.min_y, 20);
        assert_eq!(bounds.max_y, 80);
    }

    #[test]
    fn test_edge_detection_right() {
        let detector = OverlapDetector::new(OverlapConfig::default());
        let bounds = Bounds {
            min_x: 1800,
            max_x: 1920,
            min_y: 200,
            max_y: 800,
        };
        let edge = detector.determine_overlap_edge(&bounds, 1920, 1080);
        assert_eq!(edge, OverlapEdge::Right);
    }

    #[test]
    fn test_edge_detection_left() {
        let detector = OverlapDetector::new(OverlapConfig::default());
        let bounds = Bounds {
            min_x: 0,
            max_x: 200,
            min_y: 200,
            max_y: 800,
        };
        let edge = detector.determine_overlap_edge(&bounds, 1920, 1080);
        assert_eq!(edge, OverlapEdge::Left);
    }
}
