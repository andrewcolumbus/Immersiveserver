//! HAP Converter Window
//!
//! A Resolume Alley-style video converter that converts any format to HAP.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use egui::{Color32, RichText, Vec2};

/// Supported input file extensions for conversion.
const SUPPORTED_EXTENSIONS: &[&str] = &[
    "mp4", "mov", "avi", "mkv", "webm", "m4v", "mxf", "prores", "dnxhd", "dxv",
];

/// HAP video format variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HapVariant {
    /// HAP - DXT1 compression, RGB only, smallest files
    #[default]
    Hap,
    /// HAP Alpha - DXT5 compression, RGBA with transparency
    HapAlpha,
    /// HAP Q - BC7 compression, highest quality, larger files
    HapQ,
}

impl HapVariant {
    /// Returns the FFmpeg codec format string.
    pub fn ffmpeg_format(&self) -> &'static str {
        match self {
            HapVariant::Hap => "hap",
            HapVariant::HapAlpha => "hap_alpha",
            HapVariant::HapQ => "hap_q",
        }
    }

    /// Returns a human-readable name.
    pub fn display_name(&self) -> &'static str {
        match self {
            HapVariant::Hap => "HAP",
            HapVariant::HapAlpha => "HAP Alpha",
            HapVariant::HapQ => "HAP Q",
        }
    }

    /// Returns a description of the variant.
    pub fn description(&self) -> &'static str {
        match self {
            HapVariant::Hap => "RGB video, smallest files (DXT1)",
            HapVariant::HapAlpha => "RGBA with transparency (DXT5)",
            HapVariant::HapQ => "Highest quality, larger files (BC7)",
        }
    }

    /// All available variants.
    pub fn all() -> &'static [HapVariant] {
        &[HapVariant::Hap, HapVariant::HapAlpha, HapVariant::HapQ]
    }
}

/// Status of a conversion job.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum JobStatus {
    /// Job is waiting in queue
    Pending,
    /// Job is currently being converted
    Converting {
        /// Progress percentage (0-100)
        percent: f32,
        /// Processing speed (e.g., "2.5x")
        speed: Option<String>,
        /// When conversion started
        started_at: Instant,
    },
    /// Job completed successfully
    Complete {
        /// How long the conversion took
        duration_secs: f32,
        /// Output file size in bytes
        output_size: u64,
    },
    /// Job failed with an error
    Failed { error: String },
}

impl JobStatus {
    /// Check if the job is finished.
    pub fn is_finished(&self) -> bool {
        matches!(self, JobStatus::Complete { .. } | JobStatus::Failed { .. })
    }
}

/// A video file to be converted.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ConversionJob {
    /// Unique identifier
    pub id: u64,
    /// Input file path
    pub input_path: PathBuf,
    /// Output file path
    pub output_path: PathBuf,
    /// Target HAP variant
    pub variant: HapVariant,
    /// Current status
    pub status: JobStatus,
    /// Total frame count (if known)
    pub total_frames: Option<u64>,
}

impl ConversionJob {
    /// Create a new conversion job.
    pub fn new(id: u64, input_path: PathBuf, output_path: PathBuf, variant: HapVariant, total_frames: Option<u64>) -> Self {
        Self {
            id,
            input_path,
            output_path,
            variant,
            status: JobStatus::Pending,
            total_frames,
        }
    }

    /// Get the input file name.
    pub fn filename(&self) -> String {
        self.input_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown".to_string())
    }
}

/// Shared state between UI and worker thread.
struct SharedState {
    /// All jobs
    jobs: Vec<ConversionJob>,
    /// Index of currently converting job (if any)
    current_job_index: Option<usize>,
    /// Whether conversion should stop
    should_stop: bool,
    /// Whether worker is running
    is_running: bool,
}

/// HAP Converter window.
pub struct ConverterWindow {
    /// Whether the window is open
    pub is_open: bool,
    /// Shared state with worker
    state: Arc<Mutex<SharedState>>,
    /// Worker thread handle
    worker_handle: Option<JoinHandle<()>>,
    /// Cancel flag for current conversion
    cancel_flag: Arc<AtomicBool>,
    /// Next job ID
    next_job_id: u64,
    /// Output directory
    output_dir: PathBuf,
    /// Selected HAP variant for new jobs
    selected_variant: HapVariant,
    /// FFmpeg path (if found)
    ffmpeg_path: Option<PathBuf>,
    /// FFprobe path (if found)
    ffprobe_path: Option<PathBuf>,
    /// FFmpeg error message (if not found)
    ffmpeg_error: Option<String>,
}

impl ConverterWindow {
    /// Create a new converter window.
    pub fn new() -> Self {
        // Try to find FFmpeg
        let (ffmpeg_path, ffmpeg_error) = match which::which("ffmpeg") {
            Ok(path) => (Some(path), None),
            Err(_) => {
                // Check common locations
                let common_paths = if cfg!(target_os = "macos") {
                    vec![
                        "/usr/local/bin/ffmpeg",
                        "/opt/homebrew/bin/ffmpeg",
                        "/opt/local/bin/ffmpeg",
                    ]
                } else if cfg!(target_os = "windows") {
                    vec![
                        "C:\\ffmpeg\\bin\\ffmpeg.exe",
                        "C:\\Program Files\\ffmpeg\\bin\\ffmpeg.exe",
                    ]
                } else {
                    vec!["/usr/bin/ffmpeg", "/usr/local/bin/ffmpeg"]
                };

                let found = common_paths
                    .iter()
                    .map(PathBuf::from)
                    .find(|p| p.exists());

                match found {
                    Some(path) => (Some(path), None),
                    None => (
                        None,
                        Some("FFmpeg not found. Please install FFmpeg.".to_string()),
                    ),
                }
            }
        };

        // Try to find FFprobe (for getting frame counts)
        let ffprobe_path = which::which("ffprobe").ok().or_else(|| {
            let common_paths = if cfg!(target_os = "macos") {
                vec![
                    "/usr/local/bin/ffprobe",
                    "/opt/homebrew/bin/ffprobe",
                    "/opt/local/bin/ffprobe",
                ]
            } else if cfg!(target_os = "windows") {
                vec![
                    "C:\\ffmpeg\\bin\\ffprobe.exe",
                    "C:\\Program Files\\ffmpeg\\bin\\ffprobe.exe",
                ]
            } else {
                vec!["/usr/bin/ffprobe", "/usr/local/bin/ffprobe"]
            };
            common_paths.iter().map(PathBuf::from).find(|p| p.exists())
        });

        // Default output to current directory
        let output_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        Self {
            is_open: false,
            state: Arc::new(Mutex::new(SharedState {
                jobs: Vec::new(),
                current_job_index: None,
                should_stop: false,
                is_running: false,
            })),
            worker_handle: None,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            next_job_id: 0,
            output_dir,
            selected_variant: HapVariant::default(),
            ffmpeg_path,
            ffprobe_path,
            ffmpeg_error,
        }
    }

    /// Open the window.
    pub fn open(&mut self) {
        self.is_open = true;
    }

    /// Close the window.
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Show the converter window.
    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.is_open {
            return;
        }

        let mut is_open = self.is_open;
        egui::Window::new("🎬 HAP Converter")
            .id(egui::Id::new("hap_converter_window"))
            .default_size(Vec2::new(650.0, 500.0))
            .resizable(true)
            .collapsible(true)
            .open(&mut is_open)
            .show(ctx, |ui| {
                self.show_contents(ui);
            });
        self.is_open = is_open;

        // Request repaint while converting
        let is_running = self.state.lock().unwrap().is_running;
        if is_running {
            ctx.request_repaint();
        }
    }

    /// Show window contents.
    fn show_contents(&mut self, ui: &mut egui::Ui) {
        // FFmpeg warning
        if let Some(ref error) = self.ffmpeg_error {
            ui.horizontal(|ui| {
                ui.label(RichText::new("⚠").color(Color32::YELLOW));
                ui.label(RichText::new(error).color(Color32::YELLOW).small());
            });
            ui.separator();
        }

        // Toolbar
        self.show_toolbar(ui);
        ui.separator();

        // File list
        self.show_file_list(ui);
        ui.separator();

        // Output settings
        self.show_settings(ui);
        ui.separator();

        // Progress and controls
        self.show_controls(ui);
    }

    /// Show toolbar with add/clear buttons.
    fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("➕ Add Files").clicked() {
                self.open_file_dialog();
            }

            if ui.button("📁 Set Output Folder").clicked() {
                self.open_folder_dialog();
            }

            ui.separator();

            if ui.button("🗑 Clear All").clicked() {
                let mut state = self.state.lock().unwrap();
                state.jobs.clear();
            }

            if ui.button("Clear Completed").clicked() {
                let mut state = self.state.lock().unwrap();
                state.jobs.retain(|j| !j.status.is_finished());
            }
        });

        // Show output directory
        ui.horizontal(|ui| {
            ui.label("Output:");
            let path_str = self.output_dir.display().to_string();
            let truncated = if path_str.len() > 60 {
                format!("...{}", &path_str[path_str.len() - 57..])
            } else {
                path_str
            };
            ui.label(RichText::new(truncated).monospace().small());
        });
    }

    /// Show the file list.
    fn show_file_list(&mut self, ui: &mut egui::Ui) {
        let available_height = (ui.available_height() - 180.0).max(100.0);

        egui::ScrollArea::vertical()
            .max_height(available_height)
            .show(ui, |ui| {
                let state = self.state.lock().unwrap();
                let jobs = state.jobs.clone();
                drop(state);

                if jobs.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            RichText::new("Drop video files here or click Add Files")
                                .italics()
                                .color(Color32::GRAY),
                        );
                    });
                } else {
                    // Header
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("File").strong());
                        ui.add_space(ui.available_width() - 220.0);
                        ui.label(RichText::new("Format").strong());
                        ui.add_space(60.0);
                        ui.label(RichText::new("Status").strong());
                    });
                    ui.separator();

                    // Jobs
                    for job in &jobs {
                        self.show_job_row(ui, job);
                    }
                }
            });

        // Handle drag and drop
        self.handle_dropped_files(ui);
    }

    /// Show a single job row.
    fn show_job_row(&self, ui: &mut egui::Ui, job: &ConversionJob) {
        ui.horizontal(|ui| {
            // Filename
            let filename = job.filename();
            let truncated = if filename.len() > 35 {
                format!("{}...", &filename[..32])
            } else {
                filename
            };
            ui.label(truncated);

            ui.add_space(ui.available_width() - 220.0);

            // Format
            ui.label(
                RichText::new(job.variant.display_name())
                    .monospace()
                    .small(),
            );

            ui.add_space(60.0);

            // Status with color
            let (status_text, color) = match &job.status {
                JobStatus::Pending => ("Pending".to_string(), Color32::GRAY),
                JobStatus::Converting { percent, .. } => {
                    (format!("{:.0}%", percent), Color32::LIGHT_BLUE)
                }
                JobStatus::Complete { .. } => ("Done ✓".to_string(), Color32::GREEN),
                JobStatus::Failed { .. } => ("Failed ✗".to_string(), Color32::RED),
            };
            ui.label(RichText::new(status_text).color(color));
        });

        // Progress bar for converting jobs
        if let JobStatus::Converting { percent, speed, .. } = &job.status {
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                let progress_bar =
                    egui::ProgressBar::new(*percent / 100.0).show_percentage();
                ui.add_sized(
                    Vec2::new(ui.available_width() - 100.0, 8.0),
                    progress_bar,
                );

                if let Some(ref spd) = speed {
                    ui.label(RichText::new(spd).small());
                }
            });
        }

        // Error message for failed jobs
        if let JobStatus::Failed { error } = &job.status {
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.label(
                    RichText::new(error)
                        .small()
                        .color(Color32::from_rgb(255, 100, 100)),
                );
            });
        }
    }

    /// Show output settings.
    fn show_settings(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Format:");

            for variant in HapVariant::all() {
                let selected = self.selected_variant == *variant;
                if ui
                    .selectable_label(selected, variant.display_name())
                    .clicked()
                {
                    self.selected_variant = *variant;
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label("Info:");
            ui.label(
                RichText::new(self.selected_variant.description())
                    .italics()
                    .color(Color32::GRAY),
            );
        });
    }

    /// Show progress and control buttons.
    fn show_controls(&mut self, ui: &mut egui::Ui) {
        let state = self.state.lock().unwrap();
        let pending_count = state
            .jobs
            .iter()
            .filter(|j| matches!(j.status, JobStatus::Pending))
            .count();
        let complete_count = state
            .jobs
            .iter()
            .filter(|j| matches!(j.status, JobStatus::Complete { .. }))
            .count();
        let total_count = state.jobs.len();
        let is_running = state.is_running;
        drop(state);

        // Overall progress
        ui.horizontal(|ui| {
            ui.label(format!(
                "Progress: {}/{} files complete",
                complete_count, total_count
            ));

            if pending_count > 0 {
                ui.label(
                    RichText::new(format!("({} pending)", pending_count)).color(Color32::GRAY),
                );
            }
        });

        ui.add_space(8.0);

        // Control buttons
        ui.horizontal(|ui| {
            let has_pending = pending_count > 0;
            let has_ffmpeg = self.ffmpeg_path.is_some();

            ui.add_enabled_ui(!is_running && has_pending && has_ffmpeg, |ui| {
                if ui.button("▶ Start Conversion").clicked() {
                    self.start_conversion();
                }
            });

            ui.add_enabled_ui(is_running, |ui| {
                if ui.button("⏹ Stop").clicked() {
                    self.stop_conversion();
                }
            });
        });
    }

    /// Handle dropped files.
    fn handle_dropped_files(&mut self, ui: &mut egui::Ui) {
        // Check for dropped files
        let dropped_files: Vec<PathBuf> = ui.ctx().input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .filter(|p| {
                    p.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| {
                            SUPPORTED_EXTENSIONS
                                .iter()
                                .any(|ext| ext.eq_ignore_ascii_case(e))
                        })
                        .unwrap_or(false)
                })
                .collect()
        });

        if !dropped_files.is_empty() {
            self.add_files(dropped_files);
        }

        // Visual feedback for drag
        let is_dragging = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
        if is_dragging {
            let painter = ui.painter();
            let rect = ui.max_rect();
            painter.rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(2.0, Color32::from_rgb(100, 200, 255)),
                egui::StrokeKind::Outside,
            );
        }
    }

    /// Open file dialog to add files.
    fn open_file_dialog(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Video Files", SUPPORTED_EXTENSIONS)
            .add_filter("All Files", &["*"])
            .pick_files()
        {
            self.add_files(paths);
        }
    }

    /// Open folder dialog to set output directory.
    fn open_folder_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.output_dir = path.clone();
            
            // Update output paths for all pending jobs
            let mut state = self.state.lock().unwrap();
            for job in state.jobs.iter_mut() {
                if matches!(job.status, JobStatus::Pending) {
                    let stem = job.input_path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "output".to_string());
                    
                    let suffix = match job.variant {
                        HapVariant::Hap => "_hap",
                        HapVariant::HapAlpha => "_hap_alpha",
                        HapVariant::HapQ => "_hap_q",
                    };
                    
                    job.output_path = path.join(format!("{}{}.mov", stem, suffix));
                }
            }
        }
    }

    /// Get the frame count of a video using ffprobe.
    fn get_frame_count(&self, input: &PathBuf) -> Option<u64> {
        let ffprobe = self.ffprobe_path.as_ref()?;
        
        let output = Command::new(ffprobe)
            .args([
                "-v", "error",
                "-select_streams", "v:0",
                "-count_frames",
                "-show_entries", "stream=nb_read_frames",
                "-of", "default=nokey=1:noprint_wrappers=1",
            ])
            .arg(input)
            .output()
            .ok()?;
        
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.trim().parse().ok()
        } else {
            None
        }
    }

    /// Add files to the conversion queue.
    fn add_files(&mut self, paths: Vec<PathBuf>) {
        // Get frame counts for all files (this can be slow, but we do it upfront)
        let jobs_data: Vec<_> = paths
            .into_iter()
            .map(|input_path| {
                let total_frames = self.get_frame_count(&input_path);
                
                let stem = input_path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "output".to_string());

                let suffix = match self.selected_variant {
                    HapVariant::Hap => "_hap",
                    HapVariant::HapAlpha => "_hap_alpha",
                    HapVariant::HapQ => "_hap_q",
                };

                let output_path = self.output_dir.join(format!("{}{}.mov", stem, suffix));
                
                (input_path, output_path, total_frames)
            })
            .collect();
        
        let mut state = self.state.lock().unwrap();
        for (input_path, output_path, total_frames) in jobs_data {
            let job = ConversionJob::new(
                self.next_job_id,
                input_path,
                output_path,
                self.selected_variant,
                total_frames,
            );
            self.next_job_id += 1;
            state.jobs.push(job);
        }
    }

    /// Start conversion.
    fn start_conversion(&mut self) {
        let Some(ffmpeg_path) = self.ffmpeg_path.clone() else {
            return;
        };

        // Reset state
        {
            let mut state = self.state.lock().unwrap();
            state.should_stop = false;
            state.is_running = true;
        }
        self.cancel_flag.store(false, Ordering::Relaxed);

        // Spawn worker thread
        let state = Arc::clone(&self.state);
        let cancel_flag = Arc::clone(&self.cancel_flag);

        self.worker_handle = Some(thread::spawn(move || {
            Self::worker_loop(state, cancel_flag, ffmpeg_path);
        }));
    }

    /// Stop conversion.
    fn stop_conversion(&mut self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
        {
            let mut state = self.state.lock().unwrap();
            state.should_stop = true;
        }
    }

    /// Worker thread main loop.
    fn worker_loop(
        state: Arc<Mutex<SharedState>>,
        cancel_flag: Arc<AtomicBool>,
        ffmpeg_path: PathBuf,
    ) {
        loop {
            // Find next pending job
            let job_info = {
                let mut state = state.lock().unwrap();

                if state.should_stop {
                    state.is_running = false;
                    break;
                }

                // Find first pending job
                let pending_idx = state
                    .jobs
                    .iter()
                    .position(|j| matches!(j.status, JobStatus::Pending));

                if let Some(idx) = pending_idx {
                    state.current_job_index = Some(idx);
                    state.jobs[idx].status = JobStatus::Converting {
                        percent: 0.0,
                        speed: None,
                        started_at: Instant::now(),
                    };

                    let job = &state.jobs[idx];
                    Some((
                        idx,
                        job.input_path.clone(),
                        job.output_path.clone(),
                        job.variant,
                        job.total_frames,
                    ))
                } else {
                    // No more pending jobs
                    state.is_running = false;
                    state.current_job_index = None;
                    None
                }
            };

            let Some((job_idx, input, output, variant, total_frames)) = job_info else {
                break;
            };

            // Run FFmpeg
            let result = Self::run_ffmpeg(
                &ffmpeg_path,
                &input,
                &output,
                variant,
                total_frames,
                &state,
                job_idx,
                &cancel_flag,
            );

            // Update job status
            {
                let mut state = state.lock().unwrap();

                if cancel_flag.load(Ordering::Relaxed) {
                    state.jobs[job_idx].status = JobStatus::Failed {
                        error: "Cancelled".to_string(),
                    };
                    state.is_running = false;
                    break;
                }

                match result {
                    Ok(()) => {
                        let duration_secs = if let JobStatus::Converting { started_at, .. } =
                            &state.jobs[job_idx].status
                        {
                            started_at.elapsed().as_secs_f32()
                        } else {
                            0.0
                        };

                        let output_size = std::fs::metadata(&output)
                            .map(|m| m.len())
                            .unwrap_or(0);

                        state.jobs[job_idx].status = JobStatus::Complete {
                            duration_secs,
                            output_size,
                        };
                    }
                    Err(e) => {
                        state.jobs[job_idx].status = JobStatus::Failed { error: e };
                    }
                }

                state.current_job_index = None;
            }
        }
    }

    /// Run FFmpeg for a single job.
    fn run_ffmpeg(
        ffmpeg_path: &PathBuf,
        input: &PathBuf,
        output: &PathBuf,
        variant: HapVariant,
        total_frames: Option<u64>,
        state: &Arc<Mutex<SharedState>>,
        job_idx: usize,
        cancel_flag: &Arc<AtomicBool>,
    ) -> Result<(), String> {
        // Build FFmpeg command
        let mut cmd = Command::new(ffmpeg_path);

        cmd.args(["-y", "-progress", "pipe:1", "-i"])
            .arg(input)
            .args([
                "-c:v",
                "hap",
                "-format",
                variant.ffmpeg_format(),
                "-compressor",
                "snappy",
                "-an", // No audio
            ])
            .arg(output)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child: Child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn FFmpeg: {}", e))?;

        // Read progress from stdout
        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            #[allow(unused_assignments)]
            let mut current_frame: u64 = 0;
            let mut current_speed: Option<String> = None;

            for line in reader.lines().map_while(Result::ok) {
                // Check for cancel
                if cancel_flag.load(Ordering::Relaxed) {
                    let _ = child.kill();
                    return Err("Cancelled".to_string());
                }

                // Parse progress line
                if let Some((key, value)) = line.trim().split_once('=') {
                    match key {
                        "frame" => {
                            if let Ok(frame) = value.parse::<u64>() {
                                current_frame = frame;
                                
                                // Calculate progress based on frame count
                                let percent = if let Some(total) = total_frames {
                                    if total > 0 {
                                        ((current_frame as f64 / total as f64) * 100.0).min(100.0) as f32
                                    } else {
                                        0.0
                                    }
                                } else {
                                    0.0 // Unknown total, can't show progress
                                };

                                let mut state = state.lock().unwrap();
                                if let JobStatus::Converting { started_at, .. } =
                                    &state.jobs[job_idx].status
                                {
                                    let started = *started_at;
                                    state.jobs[job_idx].status = JobStatus::Converting {
                                        percent,
                                        speed: current_speed.clone(),
                                        started_at: started,
                                    };
                                }
                            }
                        }
                        "speed" => {
                            current_speed = Some(value.trim().to_string());
                        }
                        "progress" => {
                            if value == "end" {
                                // Conversion complete - set to 100%
                                let mut state = state.lock().unwrap();
                                if let JobStatus::Converting { started_at, .. } =
                                    &state.jobs[job_idx].status
                                {
                                    let started = *started_at;
                                    state.jobs[job_idx].status = JobStatus::Converting {
                                        percent: 100.0,
                                        speed: current_speed.clone(),
                                        started_at: started,
                                    };
                                }
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Wait for process to finish
        let status = child
            .wait()
            .map_err(|e| format!("Failed to wait for FFmpeg: {}", e))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!(
                "FFmpeg exited with code: {:?}",
                status.code()
            ))
        }
    }
}

impl Default for ConverterWindow {
    fn default() -> Self {
        Self::new()
    }
}

