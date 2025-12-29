//! Conversion job definition.

#![allow(dead_code)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::ffmpeg::{ConversionProgress, VideoInfo};
use super::formats::{HapVariant, QualityPreset};

/// Unique identifier for a conversion job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(pub u64);

impl JobId {
    /// Create a new unique job ID based on current time.
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        JobId(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

/// Status of a conversion job.
#[derive(Debug, Clone, PartialEq)]
pub enum JobStatus {
    /// Job is waiting in queue
    Pending,
    /// Job is currently being converted
    Converting {
        progress: ConversionProgress,
        started_at: Instant,
    },
    /// Job completed successfully
    Complete {
        duration: Duration,
        output_size: u64,
    },
    /// Job failed with an error
    Failed {
        error: String,
    },
    /// Job was cancelled by user
    Cancelled,
}

impl JobStatus {
    /// Check if the job is finished (complete, failed, or cancelled).
    pub fn is_finished(&self) -> bool {
        matches!(self, JobStatus::Complete { .. } | JobStatus::Failed { .. } | JobStatus::Cancelled)
    }

    /// Check if the job is currently active.
    pub fn is_active(&self) -> bool {
        matches!(self, JobStatus::Converting { .. })
    }

    /// Get a display string for the status.
    pub fn display(&self) -> String {
        match self {
            JobStatus::Pending => "Pending".to_string(),
            JobStatus::Converting { progress, .. } => {
                format!("{:.1}%", progress.percent)
            }
            JobStatus::Complete { duration, .. } => {
                format!("Done ({:.1}s)", duration.as_secs_f64())
            }
            JobStatus::Failed { error } => {
                format!("Failed: {}", error)
            }
            JobStatus::Cancelled => "Cancelled".to_string(),
        }
    }
}

/// A video file to be converted.
#[derive(Debug, Clone)]
pub struct ConversionJob {
    /// Unique job identifier
    pub id: JobId,
    /// Input file path
    pub input_path: PathBuf,
    /// Output file path
    pub output_path: PathBuf,
    /// Target HAP variant
    pub variant: HapVariant,
    /// Quality preset
    pub preset: QualityPreset,
    /// Current status
    pub status: JobStatus,
    /// Video metadata (if available)
    pub video_info: Option<VideoInfo>,
    /// Whether this job is selected in the UI
    pub selected: bool,
}

impl ConversionJob {
    /// Create a new conversion job.
    pub fn new(
        input_path: PathBuf,
        output_path: PathBuf,
        variant: HapVariant,
        preset: QualityPreset,
    ) -> Self {
        Self {
            id: JobId::new(),
            input_path,
            output_path,
            variant,
            preset,
            status: JobStatus::Pending,
            video_info: None,
            selected: true,
        }
    }

    /// Get the input file name.
    pub fn input_filename(&self) -> String {
        self.input_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown".to_string())
    }

    /// Get the resolution string (e.g., "1920x1080").
    pub fn resolution_string(&self) -> String {
        self.video_info
            .as_ref()
            .map(|info| format!("{}x{}", info.width, info.height))
            .unwrap_or_else(|| "Unknown".to_string())
    }

    /// Get the duration string (e.g., "2:34").
    pub fn duration_string(&self) -> String {
        self.video_info
            .as_ref()
            .map(|info| {
                let secs = info.duration_seconds as u64;
                let mins = secs / 60;
                let secs = secs % 60;
                format!("{}:{:02}", mins, secs)
            })
            .unwrap_or_else(|| "--:--".to_string())
    }

    /// Mark the job as converting.
    pub fn start(&mut self) {
        self.status = JobStatus::Converting {
            progress: ConversionProgress::default(),
            started_at: Instant::now(),
        };
    }

    /// Update progress.
    pub fn update_progress(&mut self, progress: ConversionProgress) {
        if let JobStatus::Converting { started_at, .. } = &self.status {
            let started_at = *started_at;
            self.status = JobStatus::Converting { progress, started_at };
        }
    }

    /// Mark the job as complete.
    pub fn complete(&mut self, output_size: u64) {
        if let JobStatus::Converting { started_at, .. } = &self.status {
            self.status = JobStatus::Complete {
                duration: started_at.elapsed(),
                output_size,
            };
        }
    }

    /// Mark the job as failed.
    pub fn fail(&mut self, error: String) {
        self.status = JobStatus::Failed { error };
    }

    /// Mark the job as cancelled.
    pub fn cancel(&mut self) {
        self.status = JobStatus::Cancelled;
    }
}



