//! Calibration module for structured light pattern generation and decoding.

mod gray_code;
mod session;
mod decoder;
mod homography;

pub use gray_code::{GrayCodeGenerator, PatternConfig, PatternDirection, PatternSpec};
pub use session::{
    CalibrationConfig, CalibrationSession, CalibrationState, CapturePhase,
    CapturedPair, CurrentPattern, ProjectorCalibration,
};
pub use decoder::DecodedCorrespondences;
pub use homography::{HomographyResult, HomographyComputer};
