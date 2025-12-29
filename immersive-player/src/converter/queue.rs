//! Job queue management with background worker.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crossbeam_channel::{bounded, Receiver, Sender};

use super::ffmpeg::{ConversionProgress, FFmpegError, FFmpegWrapper};
use super::formats::{HapVariant, QualityPreset};
use super::job::{ConversionJob, JobId, JobStatus};

/// Commands sent to the worker thread.
#[derive(Debug)]
enum WorkerCommand {
    /// Process the next job in queue
    ProcessNext,
    /// Stop the worker thread
    Stop,
    /// Cancel the current job
    CancelCurrent,
}

/// Events from the worker thread.
#[derive(Debug, Clone)]
pub enum WorkerEvent {
    /// Job started processing
    JobStarted(JobId),
    /// Job progress update
    Progress(JobId, ConversionProgress),
    /// Job completed successfully
    JobCompleted(JobId, u64), // output file size
    /// Job failed with error
    JobFailed(JobId, String),
    /// Job was cancelled
    JobCancelled(JobId),
    /// Worker is idle (no more jobs)
    Idle,
}

/// Thread-safe job queue.
pub struct JobQueue {
    /// All jobs (pending, active, and completed)
    jobs: Arc<Mutex<VecDeque<ConversionJob>>>,
    /// Command channel to worker
    command_tx: Sender<WorkerCommand>,
    /// Event channel from worker
    event_rx: Receiver<WorkerEvent>,
    /// Worker thread handle
    worker_handle: Option<JoinHandle<()>>,
    /// Output directory for converted files
    output_dir: PathBuf,
    /// Selected HAP variant for new jobs
    pub variant: HapVariant,
    /// Selected quality preset for new jobs
    pub preset: QualityPreset,
    /// Whether conversion is running
    pub is_running: bool,
}

impl JobQueue {
    /// Create a new job queue with a background worker.
    pub fn new() -> Self {
        let (command_tx, command_rx) = bounded::<WorkerCommand>(16);
        let (event_tx, event_rx) = bounded::<WorkerEvent>(64);
        let jobs = Arc::new(Mutex::new(VecDeque::new()));
        let jobs_clone = Arc::clone(&jobs);

        // Spawn worker thread
        let worker_handle = thread::spawn(move || {
            Self::worker_loop(jobs_clone, command_rx, event_tx);
        });

        // Default output to current directory
        let output_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        Self {
            jobs,
            command_tx,
            event_rx,
            worker_handle: Some(worker_handle),
            output_dir,
            variant: HapVariant::default(),
            preset: QualityPreset::default(),
            is_running: false,
        }
    }

    /// Worker thread main loop.
    fn worker_loop(
        jobs: Arc<Mutex<VecDeque<ConversionJob>>>,
        command_rx: Receiver<WorkerCommand>,
        event_tx: Sender<WorkerEvent>,
    ) {
        let ffmpeg = match FFmpegWrapper::new() {
            Ok(f) => f,
            Err(e) => {
                log::error!("Failed to initialize FFmpeg: {}", e);
                return;
            }
        };

        loop {
            match command_rx.recv() {
                Ok(WorkerCommand::ProcessNext) => {
                    // Find next pending job
                    let job_info = {
                        let mut jobs_lock = jobs.lock().unwrap();
                        jobs_lock.iter_mut()
                            .find(|j| matches!(j.status, JobStatus::Pending) && j.selected)
                            .map(|j| {
                                j.start();
                                (j.id, j.input_path.clone(), j.output_path.clone(), j.variant, j.preset)
                            })
                    };

                    if let Some((id, input, output, variant, preset)) = job_info {
                        let _ = event_tx.send(WorkerEvent::JobStarted(id));

                        // Get video info for progress calculation
                        let video_info = ffmpeg.get_video_info(&input).ok();
                        let duration = video_info.as_ref().map(|i| i.duration_seconds);

                        // Start conversion
                        match ffmpeg.start_conversion(&input, &output, variant, preset) {
                            Ok(mut process) => {
                                // Poll for progress
                                loop {
                                    // Check for cancel command (non-blocking)
                                    if let Ok(cmd) = command_rx.try_recv() {
                                        match cmd {
                                            WorkerCommand::CancelCurrent => {
                                                process.cancel();
                                                let mut jobs_lock = jobs.lock().unwrap();
                                                if let Some(job) = jobs_lock.iter_mut().find(|j| j.id == id) {
                                                    job.cancel();
                                                }
                                                let _ = event_tx.send(WorkerEvent::JobCancelled(id));
                                                break;
                                            }
                                            WorkerCommand::Stop => {
                                                process.cancel();
                                                return;
                                            }
                                            _ => {}
                                        }
                                    }

                                    match process.poll_progress() {
                                        Some(Ok(mut progress)) => {
                                            progress.duration_seconds = duration;
                                            if let Some(dur) = duration {
                                                progress.percent = (progress.time_seconds / dur * 100.0).min(100.0);
                                            }
                                            
                                            // Update job status
                                            {
                                                let mut jobs_lock = jobs.lock().unwrap();
                                                if let Some(job) = jobs_lock.iter_mut().find(|j| j.id == id) {
                                                    job.update_progress(progress.clone());
                                                }
                                            }
                                            
                                            let _ = event_tx.send(WorkerEvent::Progress(id, progress));
                                            thread::sleep(std::time::Duration::from_millis(100));
                                        }
                                        Some(Err(FFmpegError::Cancelled)) => {
                                            let mut jobs_lock = jobs.lock().unwrap();
                                            if let Some(job) = jobs_lock.iter_mut().find(|j| j.id == id) {
                                                job.cancel();
                                            }
                                            let _ = event_tx.send(WorkerEvent::JobCancelled(id));
                                            break;
                                        }
                                        Some(Err(e)) => {
                                            let error_msg = e.to_string();
                                            let mut jobs_lock = jobs.lock().unwrap();
                                            if let Some(job) = jobs_lock.iter_mut().find(|j| j.id == id) {
                                                job.fail(error_msg.clone());
                                            }
                                            let _ = event_tx.send(WorkerEvent::JobFailed(id, error_msg));
                                            break;
                                        }
                                        None => {
                                            // Process finished successfully
                                            let output_size = std::fs::metadata(&output)
                                                .map(|m| m.len())
                                                .unwrap_or(0);
                                            
                                            let mut jobs_lock = jobs.lock().unwrap();
                                            if let Some(job) = jobs_lock.iter_mut().find(|j| j.id == id) {
                                                job.complete(output_size);
                                            }
                                            let _ = event_tx.send(WorkerEvent::JobCompleted(id, output_size));
                                            break;
                                        }
                                    }
                                }

                                // Continue with next job
                                let _ = command_tx_clone_trick(&command_rx);
                            }
                            Err(e) => {
                                let error_msg = e.to_string();
                                let mut jobs_lock = jobs.lock().unwrap();
                                if let Some(job) = jobs_lock.iter_mut().find(|j| j.id == id) {
                                    job.fail(error_msg.clone());
                                }
                                let _ = event_tx.send(WorkerEvent::JobFailed(id, error_msg));
                            }
                        }
                    } else {
                        // No more pending jobs
                        let _ = event_tx.send(WorkerEvent::Idle);
                    }
                }
                Ok(WorkerCommand::Stop) => {
                    break;
                }
                Ok(WorkerCommand::CancelCurrent) => {
                    // Will be handled in the conversion loop
                }
                Err(_) => {
                    // Channel closed, exit
                    break;
                }
            }
        }
    }

    /// Add a file to the conversion queue.
    pub fn add_file(&mut self, input_path: PathBuf) {
        let output_path = FFmpegWrapper::generate_output_path(
            &input_path,
            &self.output_dir,
            self.variant,
        );

        let mut job = ConversionJob::new(
            input_path.clone(),
            output_path,
            self.variant,
            self.preset,
        );

        // Try to get video info
        if let Ok(ffmpeg) = FFmpegWrapper::new() {
            job.video_info = ffmpeg.get_video_info(&input_path).ok();
        }

        let mut jobs = self.jobs.lock().unwrap();
        jobs.push_back(job);
    }

    /// Add multiple files to the queue.
    pub fn add_files(&mut self, paths: Vec<PathBuf>) {
        for path in paths {
            self.add_file(path);
        }
    }

    /// Start processing the queue.
    pub fn start(&mut self) {
        self.is_running = true;
        let _ = self.command_tx.send(WorkerCommand::ProcessNext);
    }

    /// Stop processing (cancels current job).
    pub fn stop(&mut self) {
        self.is_running = false;
        let _ = self.command_tx.send(WorkerCommand::CancelCurrent);
    }

    /// Clear all pending jobs.
    pub fn clear_pending(&mut self) {
        let mut jobs = self.jobs.lock().unwrap();
        jobs.retain(|j| !matches!(j.status, JobStatus::Pending));
    }

    /// Clear all completed jobs.
    pub fn clear_completed(&mut self) {
        let mut jobs = self.jobs.lock().unwrap();
        jobs.retain(|j| !j.status.is_finished());
    }

    /// Clear all jobs.
    pub fn clear_all(&mut self) {
        self.stop();
        let mut jobs = self.jobs.lock().unwrap();
        jobs.clear();
    }

    /// Remove a specific job.
    pub fn remove_job(&mut self, id: JobId) {
        let mut jobs = self.jobs.lock().unwrap();
        jobs.retain(|j| j.id != id);
    }

    /// Get all jobs (for UI display).
    pub fn get_jobs(&self) -> Vec<ConversionJob> {
        self.jobs.lock().unwrap().iter().cloned().collect()
    }

    /// Toggle job selection.
    pub fn toggle_selection(&mut self, id: JobId) {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(job) = jobs.iter_mut().find(|j| j.id == id) {
            job.selected = !job.selected;
        }
    }

    /// Set output directory.
    pub fn set_output_dir(&mut self, dir: PathBuf) {
        self.output_dir = dir;
        
        // Update output paths for pending jobs
        let mut jobs = self.jobs.lock().unwrap();
        for job in jobs.iter_mut() {
            if matches!(job.status, JobStatus::Pending) {
                job.output_path = FFmpegWrapper::generate_output_path(
                    &job.input_path,
                    &self.output_dir,
                    job.variant,
                );
            }
        }
    }

    /// Get current output directory.
    pub fn output_dir(&self) -> &PathBuf {
        &self.output_dir
    }

    /// Poll for worker events (non-blocking).
    pub fn poll_events(&mut self) -> Vec<WorkerEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            match &event {
                WorkerEvent::Idle => {
                    self.is_running = false;
                }
                WorkerEvent::JobCompleted(_, _) | 
                WorkerEvent::JobFailed(_, _) | 
                WorkerEvent::JobCancelled(_) => {
                    // Trigger next job if still running
                    if self.is_running {
                        let _ = self.command_tx.send(WorkerCommand::ProcessNext);
                    }
                }
                _ => {}
            }
            events.push(event);
        }
        events
    }

    /// Get count statistics.
    pub fn stats(&self) -> (usize, usize, usize) {
        let jobs = self.jobs.lock().unwrap();
        let pending = jobs.iter().filter(|j| matches!(j.status, JobStatus::Pending)).count();
        let complete = jobs.iter().filter(|j| matches!(j.status, JobStatus::Complete { .. })).count();
        let total = jobs.len();
        (pending, complete, total)
    }
}

impl Default for JobQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for JobQueue {
    fn drop(&mut self) {
        let _ = self.command_tx.send(WorkerCommand::Stop);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

// Helper to send ProcessNext after completing a job
fn command_tx_clone_trick(_rx: &Receiver<WorkerCommand>) {
    // The worker will receive the next command through normal flow
    // This is a placeholder - in practice, we'd need to restructure
    // to allow the worker to self-trigger next job
}



