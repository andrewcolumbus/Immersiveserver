//! Homography computation using OpenCV.
//!
//! Computes camera-to-projector homography from decoded correspondences
//! using RANSAC for robust outlier rejection.

use super::decoder::DecodedCorrespondences;

/// Result of homography computation.
#[derive(Debug, Clone)]
pub struct HomographyResult {
    /// 3x3 homography matrix (row-major).
    pub matrix: [[f64; 3]; 3],
    /// Number of inlier points.
    pub inlier_count: usize,
    /// Ratio of inliers to total points.
    pub inlier_ratio: f32,
    /// Mean reprojection error in pixels.
    pub reprojection_error: f64,
}

impl Default for HomographyResult {
    fn default() -> Self {
        Self {
            matrix: [
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
            inlier_count: 0,
            inlier_ratio: 0.0,
            reprojection_error: 0.0,
        }
    }
}

#[cfg(feature = "opencv")]
mod opencv_impl {
    use super::*;
    use opencv::core::{Mat, Point2f, Vector};
    use opencv::calib3d;
    use opencv::prelude::*;

    /// Homography computer using OpenCV.
    pub struct HomographyComputer {
        /// RANSAC reprojection threshold in pixels.
        pub ransac_threshold: f64,
        /// Maximum RANSAC iterations.
        pub max_iters: i32,
        /// Confidence level for RANSAC.
        pub confidence: f64,
        /// Minimum points required for homography.
        pub min_points: usize,
        /// Sampling stride for correspondences.
        pub sample_stride: u32,
    }

    impl Default for HomographyComputer {
        fn default() -> Self {
            Self {
                ransac_threshold: 3.0,
                max_iters: 2000,
                confidence: 0.995,
                min_points: 100,
                sample_stride: 4,
            }
        }
    }

    impl HomographyComputer {
        pub fn new() -> Self {
            Self::default()
        }

        /// Compute homography from decoded correspondences.
        pub fn compute(&self, correspondences: &DecodedCorrespondences) -> Result<HomographyResult, String> {
            // Extract point pairs from correspondences
            let (src_points, dst_points) = self.extract_points(correspondences);

            if src_points.len() < self.min_points {
                return Err(format!(
                    "Not enough valid correspondences: {} (need at least {})",
                    src_points.len(),
                    self.min_points
                ));
            }

            log::info!("Computing homography from {} point pairs", src_points.len());

            // Compute homography with RANSAC
            let mut mask = Mat::default();
            let homography = calib3d::find_homography_ext(
                &src_points,
                &dst_points,
                calib3d::RANSAC,
                self.ransac_threshold,
                &mut mask,
                self.max_iters,
                self.confidence,
            ).map_err(|e| format!("OpenCV error: {}", e))?;

            if homography.empty() {
                return Err("Failed to compute homography".to_string());
            }

            // Count inliers
            let total_points = src_points.len();
            let inlier_count = self.count_inliers(&mask);
            let inlier_ratio = inlier_count as f32 / total_points as f32;

            // Compute reprojection error
            let reprojection_error = self.compute_reprojection_error(
                &src_points,
                &dst_points,
                &homography,
                &mask,
            )?;

            // Extract matrix values
            let matrix = self.mat_to_array(&homography)?;

            log::info!(
                "Homography computed: {} inliers ({:.1}%), error: {:.2}px",
                inlier_count,
                inlier_ratio * 100.0,
                reprojection_error
            );

            Ok(HomographyResult {
                matrix,
                inlier_count,
                inlier_ratio,
                reprojection_error,
            })
        }

        /// Extract point pairs from correspondences with sampling.
        fn extract_points(&self, corr: &DecodedCorrespondences) -> (Vector<Point2f>, Vector<Point2f>) {
            let mut src_points = Vector::<Point2f>::new();
            let mut dst_points = Vector::<Point2f>::new();

            let stride = self.sample_stride;

            for y in (0..corr.camera_height).step_by(stride as usize) {
                for x in (0..corr.camera_width).step_by(stride as usize) {
                    let idx = (y * corr.camera_width + x) as usize;

                    if corr.valid_mask[idx] && corr.confidence[idx] > 0.5 {
                        let proj_x = corr.projector_x[idx] as f32;
                        let proj_y = corr.projector_y[idx] as f32;

                        // Skip invalid projector coordinates
                        if proj_x >= 0.0 && proj_y >= 0.0 {
                            src_points.push(Point2f::new(x as f32, y as f32));
                            dst_points.push(Point2f::new(proj_x, proj_y));
                        }
                    }
                }
            }

            (src_points, dst_points)
        }

        /// Count inliers from RANSAC mask.
        fn count_inliers(&self, mask: &Mat) -> usize {
            let mut count = 0;
            let rows = mask.rows();
            for i in 0..rows {
                if let Ok(val) = mask.at::<u8>(i) {
                    if *val > 0 {
                        count += 1;
                    }
                }
            }
            count
        }

        /// Compute mean reprojection error for inliers.
        fn compute_reprojection_error(
            &self,
            src: &Vector<Point2f>,
            dst: &Vector<Point2f>,
            homography: &Mat,
            mask: &Mat,
        ) -> Result<f64, String> {
            let mut total_error = 0.0;
            let mut count = 0;

            for i in 0..src.len() {
                // Check if this point is an inlier
                if let Ok(val) = mask.at::<u8>(i as i32) {
                    if *val == 0 {
                        continue;
                    }
                }

                let src_pt = src.get(i).map_err(|e| format!("Point access error: {}", e))?;
                let dst_pt = dst.get(i).map_err(|e| format!("Point access error: {}", e))?;

                // Transform source point through homography
                let h: &[f64] = homography.data_typed()
                    .map_err(|e| format!("Homography data access error: {}", e))?;

                let x = src_pt.x as f64;
                let y = src_pt.y as f64;

                let w = h[6] * x + h[7] * y + h[8];
                if w.abs() < 1e-10 {
                    continue;
                }

                let tx = (h[0] * x + h[1] * y + h[2]) / w;
                let ty = (h[3] * x + h[4] * y + h[5]) / w;

                let dx = tx - dst_pt.x as f64;
                let dy = ty - dst_pt.y as f64;
                total_error += (dx * dx + dy * dy).sqrt();
                count += 1;
            }

            if count > 0 {
                Ok(total_error / count as f64)
            } else {
                Ok(0.0)
            }
        }

        /// Convert OpenCV Mat to 3x3 array.
        fn mat_to_array(&self, mat: &Mat) -> Result<[[f64; 3]; 3], String> {
            if mat.rows() != 3 || mat.cols() != 3 {
                return Err(format!("Invalid matrix size: {}x{}", mat.rows(), mat.cols()));
            }

            let data: &[f64] = mat.data_typed()
                .map_err(|e| format!("Matrix data access error: {}", e))?;

            Ok([
                [data[0], data[1], data[2]],
                [data[3], data[4], data[5]],
                [data[6], data[7], data[8]],
            ])
        }

        /// Apply homography to a point (camera -> projector).
        pub fn transform_point(matrix: &[[f64; 3]; 3], x: f64, y: f64) -> (f64, f64) {
            let h = matrix;
            let w = h[2][0] * x + h[2][1] * y + h[2][2];
            if w.abs() < 1e-10 {
                return (0.0, 0.0);
            }
            let tx = (h[0][0] * x + h[0][1] * y + h[0][2]) / w;
            let ty = (h[1][0] * x + h[1][1] * y + h[1][2]) / w;
            (tx, ty)
        }

        /// Compute inverse homography matrix.
        pub fn invert(matrix: &[[f64; 3]; 3]) -> Result<[[f64; 3]; 3], String> {
            let h = matrix;

            // Compute determinant
            let det = h[0][0] * (h[1][1] * h[2][2] - h[1][2] * h[2][1])
                    - h[0][1] * (h[1][0] * h[2][2] - h[1][2] * h[2][0])
                    + h[0][2] * (h[1][0] * h[2][1] - h[1][1] * h[2][0]);

            if det.abs() < 1e-10 {
                return Err("Matrix is singular, cannot invert".to_string());
            }

            let inv_det = 1.0 / det;

            Ok([
                [
                    (h[1][1] * h[2][2] - h[1][2] * h[2][1]) * inv_det,
                    (h[0][2] * h[2][1] - h[0][1] * h[2][2]) * inv_det,
                    (h[0][1] * h[1][2] - h[0][2] * h[1][1]) * inv_det,
                ],
                [
                    (h[1][2] * h[2][0] - h[1][0] * h[2][2]) * inv_det,
                    (h[0][0] * h[2][2] - h[0][2] * h[2][0]) * inv_det,
                    (h[0][2] * h[1][0] - h[0][0] * h[1][2]) * inv_det,
                ],
                [
                    (h[1][0] * h[2][1] - h[1][1] * h[2][0]) * inv_det,
                    (h[0][1] * h[2][0] - h[0][0] * h[2][1]) * inv_det,
                    (h[0][0] * h[1][1] - h[0][1] * h[1][0]) * inv_det,
                ],
            ])
        }
    }
}

#[cfg(feature = "opencv")]
pub use opencv_impl::HomographyComputer;

#[cfg(not(feature = "opencv"))]
pub struct HomographyComputer;

#[cfg(not(feature = "opencv"))]
impl HomographyComputer {
    pub fn new() -> Self {
        Self
    }

    pub fn compute(&self, _correspondences: &DecodedCorrespondences) -> Result<HomographyResult, String> {
        Err("OpenCV feature not enabled. Build with --features opencv".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_transform() {
        let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

        #[cfg(feature = "opencv")]
        {
            let (tx, ty) = HomographyComputer::transform_point(&identity, 100.0, 200.0);
            assert!((tx - 100.0).abs() < 1e-6);
            assert!((ty - 200.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_matrix_invert() {
        #[cfg(feature = "opencv")]
        {
            let matrix = [[2.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 1.0]];
            let inv = HomographyComputer::invert(&matrix).unwrap();

            // Inverse should be [0.5, 0, 0], [0, 0.333, 0], [0, 0, 1]
            assert!((inv[0][0] - 0.5).abs() < 1e-6);
            assert!((inv[1][1] - 1.0/3.0).abs() < 1e-6);
        }
    }
}
