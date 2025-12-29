//! Converter window UI.

#![allow(dead_code)]

use std::path::PathBuf;

use egui::{Color32, RichText, Vec2};

use super::formats::{is_supported_extension, HapVariant, QualityPreset};
use super::job::JobStatus;
use super::queue::JobQueue;

/// HAP Converter window.
pub struct ConverterWindow {
    /// Job queue for conversions
    pub queue: JobQueue,
    /// Whether the window is open
    pub is_open: bool,
    /// FFmpeg status message
    ffmpeg_status: Option<String>,
}

impl ConverterWindow {
    /// Create a new converter window.
    pub fn new() -> Self {
        // Check FFmpeg availability
        let ffmpeg_status = match super::ffmpeg::FFmpegWrapper::new() {
            Ok(_) => None,
            Err(e) => Some(e.to_string()),
        };

        Self {
            queue: JobQueue::new(),
            is_open: false,
            ffmpeg_status,
        }
    }

    /// Show the converter window.
    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.is_open {
            return;
        }

        // Poll for worker events
        let _events = self.queue.poll_events();

        let mut is_open = self.is_open;
        egui::Window::new("🎬 HAP Converter")
            .id(egui::Id::new("hap_converter_window"))
            .default_size(Vec2::new(600.0, 500.0))
            .resizable(true)
            .collapsible(true)
            .open(&mut is_open)
            .show(ctx, |ui| {
                self.show_contents(ui);
            });
        self.is_open = is_open;

        // Request repaint while converting
        if self.queue.is_running {
            ctx.request_repaint();
        }
    }

    /// Show window contents.
    fn show_contents(&mut self, ui: &mut egui::Ui) {
        // FFmpeg warning
        if let Some(ref error) = self.ffmpeg_status {
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
        self.show_progress(ui);
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
                self.queue.clear_all();
            }

            if ui.button("Clear Completed").clicked() {
                self.queue.clear_completed();
            }
        });

        // Show output directory
        ui.horizontal(|ui| {
            ui.label("Output:");
            let path_str = self.queue.output_dir().display().to_string();
            let truncated = if path_str.len() > 50 {
                format!("...{}", &path_str[path_str.len() - 47..])
            } else {
                path_str
            };
            ui.label(RichText::new(truncated).monospace().small());
        });
    }

    /// Show the file list.
    fn show_file_list(&mut self, ui: &mut egui::Ui) {
        let available_height = (ui.available_height() - 200.0).max(100.0);
        
        egui::ScrollArea::vertical()
            .max_height(available_height)
            .show(ui, |ui| {
                let jobs = self.queue.get_jobs();
                
                if jobs.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label(RichText::new("Drop files here or click Add Files")
                            .italics()
                            .color(Color32::GRAY));
                    });
                } else {
                    // Header
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("  ").monospace());
                        ui.add_space(20.0);
                        ui.label(RichText::new("File").strong());
                        ui.add_space(ui.available_width() - 200.0);
                        ui.label(RichText::new("Resolution").strong());
                        ui.add_space(40.0);
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
    fn show_job_row(&mut self, ui: &mut egui::Ui, job: &super::job::ConversionJob) {
        let id = job.id;
        let is_pending = matches!(job.status, JobStatus::Pending);
        
        ui.horizontal(|ui| {
            // Checkbox (only for pending jobs)
            if is_pending {
                let mut selected = job.selected;
                if ui.checkbox(&mut selected, "").changed() {
                    self.queue.toggle_selection(id);
                }
            } else {
                ui.add_space(20.0);
            }

            // Filename
            let filename = job.input_filename();
            let truncated = if filename.len() > 30 {
                format!("{}...", &filename[..27])
            } else {
                filename
            };
            ui.label(truncated);

            ui.add_space(ui.available_width() - 200.0);

            // Resolution
            ui.label(RichText::new(job.resolution_string()).monospace().small());

            ui.add_space(40.0);

            // Status with color
            let (status_text, color) = match &job.status {
                JobStatus::Pending => ("Pending".to_string(), Color32::GRAY),
                JobStatus::Converting { progress, .. } => {
                    (format!("{:.0}%", progress.percent), Color32::LIGHT_BLUE)
                }
                JobStatus::Complete { .. } => ("Done ✓".to_string(), Color32::GREEN),
                JobStatus::Failed { .. } => ("Failed ✗".to_string(), Color32::RED),
                JobStatus::Cancelled => ("Cancelled".to_string(), Color32::YELLOW),
            };
            ui.label(RichText::new(status_text).color(color));

            // Remove button for pending/completed jobs
            if !matches!(job.status, JobStatus::Converting { .. }) {
                if ui.small_button("✕").clicked() {
                    self.queue.remove_job(id);
                }
            }
        });

        // Progress bar for converting jobs
        if let JobStatus::Converting { progress, .. } = &job.status {
            ui.horizontal(|ui| {
                ui.add_space(25.0);
                let progress_bar = egui::ProgressBar::new(progress.percent as f32 / 100.0)
                    .show_percentage();
                ui.add_sized(Vec2::new(ui.available_width() - 100.0, 8.0), progress_bar);
                
                if let Some(ref speed) = progress.speed {
                    ui.label(RichText::new(speed).small());
                }
            });
        }
    }

    /// Show output settings.
    fn show_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Output Settings");
        
        ui.horizontal(|ui| {
            ui.label("Format:");
            
            for variant in HapVariant::all() {
                let selected = self.queue.variant == *variant;
                if ui.selectable_label(selected, variant.display_name()).clicked() {
                    self.queue.variant = *variant;
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label("Description:");
            ui.label(RichText::new(self.queue.variant.description())
                .italics()
                .color(Color32::GRAY));
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Quality:");
            
            for preset in QualityPreset::all() {
                let selected = self.queue.preset == *preset;
                if ui.radio(selected, preset.display_name()).clicked() {
                    self.queue.preset = *preset;
                }
            }
        });
    }

    /// Show progress and control buttons.
    fn show_progress(&mut self, ui: &mut egui::Ui) {
        let (pending, complete, total) = self.queue.stats();
        let _active = if self.queue.is_running { 1 } else { 0 };

        // Overall progress
        ui.horizontal(|ui| {
            ui.label(format!(
                "Progress: {}/{} files complete",
                complete,
                total
            ));
            
            if pending > 0 {
                ui.label(RichText::new(format!("({} pending)", pending))
                    .color(Color32::GRAY));
            }
        });

        // Active job info
        if let Some(job) = self.queue.get_jobs().iter().find(|j| j.status.is_active()) {
            if let JobStatus::Converting { progress, started_at } = &job.status {
                ui.horizontal(|ui| {
                    ui.label("Converting:");
                    ui.label(RichText::new(&job.input_filename()).strong());
                });

                let elapsed = started_at.elapsed().as_secs_f64();
                let eta = if progress.percent > 0.0 {
                    let total_est = elapsed / (progress.percent / 100.0);
                    Some(total_est - elapsed)
                } else {
                    None
                };

                ui.horizontal(|ui| {
                    let bar = egui::ProgressBar::new(progress.percent as f32 / 100.0)
                        .show_percentage()
                        .animate(true);
                    ui.add_sized(Vec2::new(ui.available_width() - 150.0, 20.0), bar);

                    if let Some(eta_secs) = eta {
                        let mins = (eta_secs / 60.0).floor() as u64;
                        let secs = (eta_secs % 60.0).floor() as u64;
                        ui.label(format!("ETA: {}:{:02}", mins, secs));
                    }
                });
            }
        }

        ui.add_space(8.0);

        // Control buttons
        ui.horizontal(|ui| {
            let has_pending = pending > 0;
            
            ui.add_enabled_ui(!self.queue.is_running && has_pending, |ui| {
                if ui.button("▶ Start Conversion").clicked() {
                    self.queue.start();
                }
            });

            ui.add_enabled_ui(self.queue.is_running, |ui| {
                if ui.button("⏹ Stop").clicked() {
                    self.queue.stop();
                }
            });
        });
    }

    /// Handle dropped files.
    fn handle_dropped_files(&mut self, ui: &mut egui::Ui) {
        // Check for dropped files
        let dropped_files: Vec<PathBuf> = ui.ctx().input(|i| {
            i.raw.dropped_files.iter()
                .filter_map(|f| f.path.clone())
                .filter(|p| {
                    p.extension()
                        .and_then(|e| e.to_str())
                        .map(is_supported_extension)
                        .unwrap_or(false)
                })
                .collect()
        });

        if !dropped_files.is_empty() {
            self.queue.add_files(dropped_files);
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
            );
        }
    }

    /// Open file dialog to add files.
    fn open_file_dialog(&mut self) {
        let extensions: Vec<&str> = super::formats::supported_input_extensions().to_vec();
        
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Video Files", &extensions)
            .add_filter("All Files", &["*"])
            .pick_files()
        {
            self.queue.add_files(paths);
        }
    }

    /// Open folder dialog to set output directory.
    fn open_folder_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.queue.set_output_dir(path);
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

    /// Toggle window visibility.
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }
}

impl Default for ConverterWindow {
    fn default() -> Self {
        Self::new()
    }
}

