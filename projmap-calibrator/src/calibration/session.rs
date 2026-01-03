//! Calibration session state machine and workflow management.

use super::decoder::DecodedCorrespondences;
use super::gray_code::{GrayCodeGenerator, PatternConfig, PatternDirection, PatternSpec};
use super::homography::HomographyResult;
use std::time::{Duration, Instant};

/// Phase of the pattern capture process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapturePhase {
    /// Waiting for projector to display pattern.
    DisplayingPattern,
    /// Waiting for camera frames to arrive.
    WaitingForCapture,
    /// Averaging captured frames.
    Accumulating,
}

/// State of the calibration session.
#[derive(Debug, Clone)]
pub enum CalibrationState {
    /// Waiting to start.
    Idle,
    /// Projecting white reference.
    WhiteReference {
        projector_id: u32,
        phase: CapturePhase,
        start_time: Instant,
    },
    /// Projecting black reference.
    BlackReference {
        projector_id: u32,
        phase: CapturePhase,
        start_time: Instant,
    },
    /// Projecting Gray code pattern.
    ProjectingPattern {
        projector_id: u32,
        pattern_index: usize,
        phase: CapturePhase,
        start_time: Instant,
    },
    /// Decoding captured patterns.
    Decoding { projector_id: u32 },
    /// Computing homography.
    ComputingHomography { projector_id: u32 },
    /// Calibration complete for this projector.
    ProjectorComplete { projector_id: u32 },
    /// All calibrations complete.
    Complete,
    /// Error occurred.
    Error(String),
}

impl CalibrationState {
    pub fn is_idle(&self) -> bool {
        matches!(self, CalibrationState::Idle)
    }

    pub fn is_complete(&self) -> bool {
        matches!(self, CalibrationState::Complete | CalibrationState::ProjectorComplete { .. })
    }

    pub fn is_error(&self) -> bool {
        matches!(self, CalibrationState::Error(_))
    }
}

impl std::fmt::Display for CalibrationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CalibrationState::Idle => write!(f, "Idle"),
            CalibrationState::WhiteReference { .. } => write!(f, "White Reference"),
            CalibrationState::BlackReference { .. } => write!(f, "Black Reference"),
            CalibrationState::ProjectingPattern { pattern_index, .. } => {
                write!(f, "Pattern {}", pattern_index)
            }
            CalibrationState::Decoding { .. } => write!(f, "Decoding"),
            CalibrationState::ComputingHomography { .. } => write!(f, "Computing Homography"),
            CalibrationState::ProjectorComplete { .. } => write!(f, "Projector Complete"),
            CalibrationState::Complete => write!(f, "Complete"),
            CalibrationState::Error(msg) => write!(f, "Error: {}", msg),
        }
    }
}

/// Captured frame pair (positive and inverted).
#[derive(Clone)]
pub struct CapturedPair {
    /// Positive pattern (bit = 1 → white).
    pub positive: Vec<u8>,
    /// Inverted pattern (bit = 1 → black).
    pub inverted: Vec<u8>,
    /// Frame width.
    pub width: u32,
    /// Frame height.
    pub height: u32,
}

/// Per-projector calibration data.
pub struct ProjectorCalibration {
    /// Projector ID.
    pub projector_id: u32,
    /// Projector resolution.
    pub projector_width: u32,
    pub projector_height: u32,
    /// Pattern configuration.
    pub pattern_config: PatternConfig,
    /// White reference frame (for contrast).
    pub white_reference: Option<Vec<u8>>,
    /// Black reference frame (for contrast).
    pub black_reference: Option<Vec<u8>>,
    /// Captured horizontal patterns (Y coordinate).
    pub horizontal_pairs: Vec<CapturedPair>,
    /// Captured vertical patterns (X coordinate).
    pub vertical_pairs: Vec<CapturedPair>,
    /// Decoded correspondences.
    pub correspondences: Option<DecodedCorrespondences>,
    /// Computed homography.
    pub homography: Option<HomographyResult>,
}

impl ProjectorCalibration {
    pub fn new(id: u32, width: u32, height: u32) -> Self {
        Self {
            projector_id: id,
            projector_width: width,
            projector_height: height,
            pattern_config: PatternConfig::new(width, height),
            white_reference: None,
            black_reference: None,
            horizontal_pairs: Vec::new(),
            vertical_pairs: Vec::new(),
            correspondences: None,
            homography: None,
        }
    }

    pub fn total_patterns(&self) -> usize {
        self.pattern_config.total_patterns()
    }

    pub fn pattern_sequence(&self) -> Vec<PatternSpec> {
        self.pattern_config.pattern_sequence()
    }
}

/// Configuration for calibration timing.
#[derive(Debug, Clone)]
pub struct CalibrationConfig {
    /// Time to wait for projector to display pattern.
    pub settle_time: Duration,
    /// Number of frames to average for each pattern.
    pub frames_to_average: usize,
    /// Minimum contrast threshold for valid pixels.
    pub contrast_threshold: f32,
    /// Camera frame width.
    pub camera_width: u32,
    /// Camera frame height.
    pub camera_height: u32,
}

impl Default for CalibrationConfig {
    fn default() -> Self {
        Self {
            settle_time: Duration::from_millis(100),
            frames_to_average: 3,
            contrast_threshold: 0.1,
            camera_width: 1920,
            camera_height: 1080,
        }
    }
}

/// Manages the calibration session workflow.
pub struct CalibrationSession {
    /// Current state.
    pub state: CalibrationState,
    /// Configuration.
    pub config: CalibrationConfig,
    /// Per-projector calibration data.
    pub projectors: Vec<ProjectorCalibration>,
    /// Current projector index.
    current_projector: usize,
    /// Accumulated frames for averaging.
    accumulated_frames: Vec<Vec<u8>>,
    /// Current pattern spec being captured.
    current_pattern: Option<PatternSpec>,
}

impl CalibrationSession {
    pub fn new(config: CalibrationConfig) -> Self {
        Self {
            state: CalibrationState::Idle,
            config,
            projectors: Vec::new(),
            current_projector: 0,
            accumulated_frames: Vec::new(),
            current_pattern: None,
        }
    }

    /// Add a projector to calibrate.
    pub fn add_projector(&mut self, id: u32, width: u32, height: u32) {
        self.projectors.push(ProjectorCalibration::new(id, width, height));
    }

    /// Start the calibration process.
    pub fn start(&mut self) -> Result<(), String> {
        if self.projectors.is_empty() {
            return Err("No projectors configured".to_string());
        }

        self.current_projector = 0;
        self.state = CalibrationState::WhiteReference {
            projector_id: self.projectors[0].projector_id,
            phase: CapturePhase::DisplayingPattern,
            start_time: Instant::now(),
        };

        log::info!("Starting calibration for {} projector(s)", self.projectors.len());
        Ok(())
    }

    /// Cancel the calibration.
    pub fn cancel(&mut self) {
        self.state = CalibrationState::Idle;
        self.accumulated_frames.clear();
        self.current_pattern = None;
        log::info!("Calibration cancelled");
    }

    /// Get current pattern to display (returns None if not in pattern projection state).
    pub fn current_pattern(&self) -> Option<CurrentPattern> {
        match &self.state {
            CalibrationState::WhiteReference { .. } => Some(CurrentPattern::White),
            CalibrationState::BlackReference { .. } => Some(CurrentPattern::Black),
            CalibrationState::ProjectingPattern { pattern_index, .. } => {
                if let Some(projector) = self.projectors.get(self.current_projector) {
                    let patterns = projector.pattern_sequence();
                    patterns.get(*pattern_index).cloned().map(CurrentPattern::GrayCode)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Get progress (0.0 to 1.0).
    pub fn progress(&self) -> f32 {
        if self.projectors.is_empty() {
            return 0.0;
        }

        let patterns_per_projector = self.projectors[0].total_patterns() + 2; // +2 for white/black
        let total = patterns_per_projector * self.projectors.len();

        let completed_projectors = self.current_projector * patterns_per_projector;
        let current = match &self.state {
            CalibrationState::Idle => 0,
            CalibrationState::WhiteReference { .. } => 0,
            CalibrationState::BlackReference { .. } => 1,
            CalibrationState::ProjectingPattern { pattern_index, .. } => 2 + pattern_index,
            CalibrationState::Decoding { .. } | CalibrationState::ComputingHomography { .. } => {
                patterns_per_projector
            }
            CalibrationState::ProjectorComplete { .. } | CalibrationState::Complete => {
                patterns_per_projector
            }
            CalibrationState::Error(_) => 0,
        };

        (completed_projectors + current) as f32 / total as f32
    }

    /// Update state machine (call once per frame).
    pub fn update(&mut self) {
        let now = Instant::now();

        match &self.state {
            CalibrationState::WhiteReference { phase, start_time, projector_id }
            | CalibrationState::BlackReference { phase, start_time, projector_id } => {
                let projector_id = *projector_id;
                let is_white = matches!(self.state, CalibrationState::WhiteReference { .. });

                match phase {
                    CapturePhase::DisplayingPattern => {
                        if now.duration_since(*start_time) >= self.config.settle_time {
                            if is_white {
                                self.state = CalibrationState::WhiteReference {
                                    projector_id,
                                    phase: CapturePhase::WaitingForCapture,
                                    start_time: now,
                                };
                            } else {
                                self.state = CalibrationState::BlackReference {
                                    projector_id,
                                    phase: CapturePhase::WaitingForCapture,
                                    start_time: now,
                                };
                            }
                        }
                    }
                    CapturePhase::WaitingForCapture | CapturePhase::Accumulating => {
                        // Frame submission handles this transition
                    }
                }
            }
            CalibrationState::ProjectingPattern {
                phase,
                start_time,
                projector_id,
                pattern_index,
            } => {
                let projector_id = *projector_id;
                let pattern_index = *pattern_index;

                if *phase == CapturePhase::DisplayingPattern {
                    if now.duration_since(*start_time) >= self.config.settle_time {
                        self.state = CalibrationState::ProjectingPattern {
                            projector_id,
                            pattern_index,
                            phase: CapturePhase::WaitingForCapture,
                            start_time: now,
                        };
                    }
                }
            }
            _ => {}
        }
    }

    /// Submit a captured camera frame.
    pub fn submit_frame(&mut self, frame: Vec<u8>, width: u32, height: u32) {
        // Update camera dimensions if different
        if width != self.config.camera_width || height != self.config.camera_height {
            self.config.camera_width = width;
            self.config.camera_height = height;
        }

        match &self.state {
            CalibrationState::WhiteReference {
                phase,
                projector_id,
                ..
            } if *phase != CapturePhase::DisplayingPattern => {
                let projector_id = *projector_id;
                self.accumulated_frames.push(frame);

                if self.accumulated_frames.len() >= self.config.frames_to_average {
                    let averaged = self.average_frames();
                    if let Some(proj) = self.projectors.get_mut(self.current_projector) {
                        proj.white_reference = Some(averaged);
                    }
                    self.accumulated_frames.clear();

                    // Move to black reference
                    self.state = CalibrationState::BlackReference {
                        projector_id,
                        phase: CapturePhase::DisplayingPattern,
                        start_time: Instant::now(),
                    };
                }
            }
            CalibrationState::BlackReference {
                phase,
                projector_id,
                ..
            } if *phase != CapturePhase::DisplayingPattern => {
                let projector_id = *projector_id;
                self.accumulated_frames.push(frame);

                if self.accumulated_frames.len() >= self.config.frames_to_average {
                    let averaged = self.average_frames();
                    if let Some(proj) = self.projectors.get_mut(self.current_projector) {
                        proj.black_reference = Some(averaged);
                    }
                    self.accumulated_frames.clear();

                    // Start pattern projection
                    let patterns = if let Some(proj) = self.projectors.get(self.current_projector) {
                        proj.pattern_sequence()
                    } else {
                        Vec::new()
                    };

                    if let Some(first_pattern) = patterns.first() {
                        self.current_pattern = Some(first_pattern.clone());
                        self.state = CalibrationState::ProjectingPattern {
                            projector_id,
                            pattern_index: 0,
                            phase: CapturePhase::DisplayingPattern,
                            start_time: Instant::now(),
                        };
                    }
                }
            }
            CalibrationState::ProjectingPattern {
                phase,
                projector_id,
                pattern_index,
                ..
            } if *phase != CapturePhase::DisplayingPattern => {
                let projector_id = *projector_id;
                let pattern_index = *pattern_index;
                self.accumulated_frames.push(frame);

                if self.accumulated_frames.len() >= self.config.frames_to_average {
                    let averaged = self.average_frames();
                    self.store_pattern_capture(pattern_index, averaged);
                    self.accumulated_frames.clear();

                    // Move to next pattern or complete
                    let patterns = if let Some(proj) = self.projectors.get(self.current_projector) {
                        proj.pattern_sequence()
                    } else {
                        Vec::new()
                    };

                    let next_index = pattern_index + 1;
                    if next_index < patterns.len() {
                        self.current_pattern = patterns.get(next_index).cloned();
                        self.state = CalibrationState::ProjectingPattern {
                            projector_id,
                            pattern_index: next_index,
                            phase: CapturePhase::DisplayingPattern,
                            start_time: Instant::now(),
                        };
                    } else {
                        // All patterns captured, start decoding
                        self.state = CalibrationState::Decoding { projector_id };
                    }
                }
            }
            _ => {}
        }
    }

    /// Average accumulated frames.
    fn average_frames(&self) -> Vec<u8> {
        if self.accumulated_frames.is_empty() {
            return Vec::new();
        }

        let len = self.accumulated_frames[0].len();
        let count = self.accumulated_frames.len() as u32;
        let mut result = vec![0u8; len];

        for i in 0..len {
            let sum: u32 = self.accumulated_frames.iter().map(|f| f[i] as u32).sum();
            result[i] = (sum / count) as u8;
        }

        result
    }

    /// Store a captured pattern.
    fn store_pattern_capture(&mut self, pattern_index: usize, data: Vec<u8>) {
        let projector = match self.projectors.get_mut(self.current_projector) {
            Some(p) => p,
            None => return,
        };

        let patterns = projector.pattern_sequence();
        let spec = match patterns.get(pattern_index) {
            Some(s) => s,
            None => return,
        };

        let pair_index = pattern_index / 2;
        let is_positive = pattern_index % 2 == 0;

        let pairs = match spec.direction {
            PatternDirection::Horizontal => &mut projector.horizontal_pairs,
            PatternDirection::Vertical => &mut projector.vertical_pairs,
        };

        // Ensure we have enough pairs
        while pairs.len() <= pair_index {
            pairs.push(CapturedPair {
                positive: Vec::new(),
                inverted: Vec::new(),
                width: self.config.camera_width,
                height: self.config.camera_height,
            });
        }

        if is_positive {
            pairs[pair_index].positive = data;
        } else {
            pairs[pair_index].inverted = data;
        }
    }

    /// Decode captured patterns and compute homography.
    /// Returns true if decoding/homography is complete.
    pub fn process_calibration(&mut self) -> bool {
        match &self.state {
            CalibrationState::Decoding { projector_id } => {
                let projector_id = *projector_id;
                log::info!("Decoding patterns for projector {}", projector_id);

                // Decode patterns - copy config to avoid borrow issues
                let config = self.config.clone();
                if let Some(projector) = self.projectors.get_mut(self.current_projector) {
                    match Self::decode_patterns_for(&config, projector) {
                        Ok(correspondences) => {
                            projector.correspondences = Some(correspondences);
                            self.state = CalibrationState::ComputingHomography { projector_id };
                        }
                        Err(e) => {
                            self.state = CalibrationState::Error(format!("Decoding failed: {}", e));
                            return true;
                        }
                    }
                }
                false
            }
            CalibrationState::ComputingHomography { projector_id } => {
                let projector_id = *projector_id;
                log::info!("Computing homography for projector {}", projector_id);

                if let Some(projector) = self.projectors.get_mut(self.current_projector) {
                    #[cfg(feature = "opencv")]
                    {
                        use super::homography::HomographyComputer;

                        if let Some(ref correspondences) = projector.correspondences {
                            let computer = HomographyComputer::new();
                            match computer.compute(correspondences) {
                                Ok(result) => {
                                    log::info!(
                                        "Homography computed: {} inliers, {:.2}px error",
                                        result.inlier_count,
                                        result.reprojection_error
                                    );
                                    projector.homography = Some(result);
                                }
                                Err(e) => {
                                    log::error!("Homography computation failed: {}", e);
                                    // Continue anyway, can still use correspondences for mesh warp
                                }
                            }
                        }
                    }

                    #[cfg(not(feature = "opencv"))]
                    {
                        log::warn!("OpenCV not available, skipping homography computation");
                    }
                }

                // Move to next projector or complete
                self.current_projector += 1;
                if self.current_projector < self.projectors.len() {
                    let next_id = self.projectors[self.current_projector].projector_id;
                    self.state = CalibrationState::WhiteReference {
                        projector_id: next_id,
                        phase: CapturePhase::DisplayingPattern,
                        start_time: Instant::now(),
                    };
                } else {
                    self.state = CalibrationState::Complete;
                }
                true
            }
            _ => true,
        }
    }

    /// Decode patterns for a projector (static to avoid borrow issues).
    fn decode_patterns_for(
        config: &CalibrationConfig,
        projector: &ProjectorCalibration,
    ) -> Result<DecodedCorrespondences, String> {
        let camera_width = config.camera_width;
        let camera_height = config.camera_height;
        let pixel_count = (camera_width * camera_height) as usize;

        let mut correspondences = DecodedCorrespondences {
            camera_width,
            camera_height,
            projector_width: projector.projector_width,
            projector_height: projector.projector_height,
            projector_x: vec![-1; pixel_count],
            projector_y: vec![-1; pixel_count],
            confidence: vec![0.0; pixel_count],
            valid_mask: vec![false; pixel_count],
        };

        let white = projector.white_reference.as_ref()
            .ok_or("Missing white reference")?;
        let black = projector.black_reference.as_ref()
            .ok_or("Missing black reference")?;

        // Decode X coordinates from vertical patterns
        Self::decode_coordinate(
            config.contrast_threshold,
            &mut correspondences.projector_x,
            &mut correspondences.confidence,
            &mut correspondences.valid_mask,
            &projector.vertical_pairs,
            white,
            black,
            projector.pattern_config.horizontal_bits,
        )?;

        // Decode Y coordinates from horizontal patterns
        Self::decode_coordinate(
            config.contrast_threshold,
            &mut correspondences.projector_y,
            &mut correspondences.confidence,
            &mut correspondences.valid_mask,
            &projector.horizontal_pairs,
            white,
            black,
            projector.pattern_config.vertical_bits,
        )?;

        // Count valid pixels
        let valid_count = correspondences.valid_mask.iter().filter(|&&v| v).count();
        log::info!("Decoded {} valid correspondences", valid_count);

        Ok(correspondences)
    }

    /// Decode a single coordinate (X or Y) from pattern pairs.
    fn decode_coordinate(
        threshold: f32,
        coords: &mut [i32],
        confidence: &mut [f32],
        valid: &mut [bool],
        pairs: &[CapturedPair],
        white: &[u8],
        black: &[u8],
        bits: u32,
    ) -> Result<(), String> {
        if pairs.len() != bits as usize {
            return Err(format!("Expected {} pattern pairs, got {}", bits, pairs.len()));
        }

        let pixel_count = coords.len();

        for i in 0..pixel_count {
            // Check if pixel has enough contrast (illuminated by projector)
            let white_val = white.get(i).copied().unwrap_or(0) as f32 / 255.0;
            let black_val = black.get(i).copied().unwrap_or(0) as f32 / 255.0;
            let contrast = white_val - black_val;

            if contrast < threshold {
                continue; // Not enough contrast, skip this pixel
            }

            // Decode Gray code from pattern pairs
            let mut gray_code: u32 = 0;
            let mut total_confidence = 0.0f32;

            for (bit_idx, pair) in pairs.iter().enumerate() {
                let positive = pair.positive.get(i).copied().unwrap_or(128) as f32;
                let inverted = pair.inverted.get(i).copied().unwrap_or(128) as f32;

                let diff = positive - inverted;
                let bit_confidence = diff.abs() / 255.0;
                total_confidence += bit_confidence;

                if diff > 0.0 {
                    let bit_position = bits - 1 - bit_idx as u32;
                    gray_code |= 1 << bit_position;
                }
            }

            // Convert Gray code to binary
            let binary = GrayCodeGenerator::gray_to_binary(gray_code);

            coords[i] = binary as i32;
            confidence[i] = (confidence[i] + total_confidence / bits as f32) / 2.0;
            valid[i] = true;
        }

        Ok(())
    }
}

/// Current pattern to display.
#[derive(Debug, Clone)]
pub enum CurrentPattern {
    White,
    Black,
    GrayCode(PatternSpec),
}
