//! Projector output management module.

use crate::calibration::HomographyResult;
use crate::config::ProjectorConfig;

/// Runtime state for a projector.
#[derive(Debug)]
pub struct ProjectorRuntime {
    /// Configuration.
    pub config: ProjectorConfig,
    /// Calibration result (if calibrated).
    pub calibration: Option<HomographyResult>,
    /// Is this projector currently being calibrated.
    pub calibrating: bool,
}

impl ProjectorRuntime {
    pub fn new(config: ProjectorConfig) -> Self {
        Self {
            config,
            calibration: None,
            calibrating: false,
        }
    }

    pub fn is_calibrated(&self) -> bool {
        self.calibration.is_some()
    }
}

/// Manager for multiple projectors.
#[derive(Debug, Default)]
pub struct ProjectorManager {
    /// All configured projectors.
    projectors: Vec<ProjectorRuntime>,
    /// Currently calibrating projector index.
    active_calibration: Option<usize>,
}

impl ProjectorManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_projector(&mut self, config: ProjectorConfig) {
        self.projectors.push(ProjectorRuntime::new(config));
    }

    pub fn projectors(&self) -> &[ProjectorRuntime] {
        &self.projectors
    }

    pub fn projectors_mut(&mut self) -> &mut [ProjectorRuntime] {
        &mut self.projectors
    }

    pub fn count(&self) -> usize {
        self.projectors.len()
    }

    pub fn start_calibration(&mut self, index: usize) -> bool {
        if index < self.projectors.len() {
            self.active_calibration = Some(index);
            self.projectors[index].calibrating = true;
            true
        } else {
            false
        }
    }

    pub fn stop_calibration(&mut self) {
        if let Some(index) = self.active_calibration {
            if index < self.projectors.len() {
                self.projectors[index].calibrating = false;
            }
        }
        self.active_calibration = None;
    }

    pub fn active_calibration_index(&self) -> Option<usize> {
        self.active_calibration
    }
}
