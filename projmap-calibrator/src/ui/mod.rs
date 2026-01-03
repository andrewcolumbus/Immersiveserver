//! UI module for calibration workflow.

use crate::blending::OverlapDetectionResult;

/// UI state for the calibration application.
pub struct UiState {
    /// Number of configured projectors.
    pub projector_count: u32,
    /// Default projector width.
    pub projector_width: u32,
    /// Default projector height.
    pub projector_height: u32,
    /// Edge blend width in pixels.
    pub blend_width: u32,
    /// Edge blend curve type.
    pub blend_curve: String,
    /// Show camera preview.
    pub show_preview: bool,
    /// Detected overlap result.
    pub overlap_result: Option<OverlapDetectionResult>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            projector_count: 1,
            projector_width: 1920,
            projector_height: 1080,
            blend_width: 200,
            blend_curve: "Gamma".to_string(),
            show_preview: true,
            overlap_result: None,
        }
    }
}
