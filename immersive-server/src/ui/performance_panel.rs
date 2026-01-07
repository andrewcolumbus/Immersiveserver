//! Performance Panel
//!
//! Displays real-time performance metrics:
//! - FPS with color coding (green/yellow/red)
//! - Frame timing statistics (avg, min, max, p95, p99)
//! - GPU timing breakdown (if available)
//! - Resource counts (layers, clips, effects)
//! - GPU memory usage

use crate::telemetry::PerformanceMetrics;

/// State for the performance panel
#[derive(Default)]
pub struct PerformancePanel {
    /// Whether the panel is open
    pub open: bool,
    /// Whether to show detailed frame timing stats
    show_frame_stats: bool,
    /// Whether to show GPU timing breakdown
    show_gpu_timings: bool,
}

impl PerformancePanel {
    /// Create a new performance panel
    pub fn new() -> Self {
        Self {
            open: false,
            show_frame_stats: true,
            show_gpu_timings: true,
        }
    }

    /// Render the performance panel contents
    pub fn render_contents(&mut self, ui: &mut egui::Ui, metrics: &PerformanceMetrics) {
        // FPS Display with color coding
        let fps_color = fps_color(metrics.fps, metrics.target_fps);
        ui.horizontal(|ui| {
            ui.label("FPS:");
            ui.label(
                egui::RichText::new(format!("{:.1}", metrics.fps))
                    .color(fps_color)
                    .monospace()
                    .size(18.0),
            );
            ui.label(
                egui::RichText::new(format!("/ {}", metrics.target_fps))
                    .color(egui::Color32::GRAY)
                    .small(),
            );
        });

        ui.add_space(4.0);

        // Frame time with visual bar
        let frame_time_ms = if metrics.fps > 0.0 {
            1000.0 / metrics.fps
        } else {
            0.0
        };
        let target_frame_time = 1000.0 / metrics.target_fps as f64;

        ui.horizontal(|ui| {
            ui.label("Frame time:");
            ui.label(
                egui::RichText::new(format!("{:.2} ms", frame_time_ms))
                    .monospace()
                    .color(fps_color),
            );
        });

        // Frame time bar
        let frame_ratio = (frame_time_ms / (target_frame_time * 2.0)).min(1.0);
        let bar_height = 8.0;
        let (rect, _response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), bar_height),
            egui::Sense::hover(),
        );
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            // Background
            painter.rect_filled(rect, 2.0, egui::Color32::from_rgb(40, 40, 50));
            // Filled portion
            let filled_rect = egui::Rect::from_min_size(
                rect.min,
                egui::vec2(rect.width() * frame_ratio as f32, bar_height),
            );
            painter.rect_filled(filled_rect, 2.0, fps_color);
            // Target line at 50%
            let target_x = rect.min.x + rect.width() * 0.5;
            painter.line_segment(
                [
                    egui::pos2(target_x, rect.min.y),
                    egui::pos2(target_x, rect.max.y),
                ],
                egui::Stroke::new(1.0, egui::Color32::WHITE),
            );
        }

        ui.add_space(8.0);
        ui.separator();

        // Frame Timing Statistics (collapsible)
        let header = egui::CollapsingHeader::new("Frame Timing")
            .default_open(self.show_frame_stats)
            .show(ui, |ui| {
                egui::Grid::new("frame_stats_grid")
                    .num_columns(2)
                    .spacing([20.0, 4.0])
                    .show(ui, |ui| {
                        ui.label("Average:");
                        ui.label(
                            egui::RichText::new(format!("{:.2} ms", metrics.frame_stats.avg_ms))
                                .monospace(),
                        );
                        ui.end_row();

                        ui.label("Min:");
                        ui.label(
                            egui::RichText::new(format!("{:.2} ms", metrics.frame_stats.min_ms))
                                .monospace(),
                        );
                        ui.end_row();

                        ui.label("Max:");
                        ui.label(
                            egui::RichText::new(format!("{:.2} ms", metrics.frame_stats.max_ms))
                                .monospace()
                                .color(if metrics.frame_stats.max_ms > target_frame_time * 1.5 {
                                    egui::Color32::from_rgb(255, 100, 100)
                                } else {
                                    egui::Color32::WHITE
                                }),
                        );
                        ui.end_row();

                        ui.label("P50:");
                        ui.label(
                            egui::RichText::new(format!("{:.2} ms", metrics.frame_stats.p50_ms))
                                .monospace(),
                        );
                        ui.end_row();

                        ui.label("P95:");
                        ui.label(
                            egui::RichText::new(format!("{:.2} ms", metrics.frame_stats.p95_ms))
                                .monospace()
                                .color(if metrics.frame_stats.p95_ms > target_frame_time {
                                    egui::Color32::from_rgb(255, 200, 100)
                                } else {
                                    egui::Color32::WHITE
                                }),
                        );
                        ui.end_row();

                        ui.label("P99:");
                        ui.label(
                            egui::RichText::new(format!("{:.2} ms", metrics.frame_stats.p99_ms))
                                .monospace()
                                .color(if metrics.frame_stats.p99_ms > target_frame_time {
                                    egui::Color32::from_rgb(255, 150, 100)
                                } else {
                                    egui::Color32::WHITE
                                }),
                        );
                        ui.end_row();
                    });
            });
        self.show_frame_stats = header.openness > 0.0;

        // GPU Timings (collapsible)
        if !metrics.gpu_timings.is_empty() {
            let gpu_header = egui::CollapsingHeader::new("GPU Timings")
                .default_open(self.show_gpu_timings)
                .show(ui, |ui| {
                    egui::Grid::new("gpu_timings_grid")
                        .num_columns(2)
                        .spacing([20.0, 4.0])
                        .show(ui, |ui| {
                            // Sort timings by value descending
                            let mut timings: Vec<_> = metrics.gpu_timings.iter().collect();
                            timings.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));

                            for (name, ms) in timings {
                                ui.label(format!("{}:", name));
                                ui.label(
                                    egui::RichText::new(format!("{:.2} ms", ms)).monospace(),
                                );
                                ui.end_row();
                            }

                            ui.separator();
                            ui.end_row();

                            ui.label(egui::RichText::new("Total:").strong());
                            ui.label(
                                egui::RichText::new(format!("{:.2} ms", metrics.gpu_total_ms))
                                    .monospace()
                                    .strong(),
                            );
                            ui.end_row();
                        });
                });
            self.show_gpu_timings = gpu_header.openness > 0.0;
        } else {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("GPU timing not available")
                    .italics()
                    .color(egui::Color32::GRAY)
                    .small(),
            );
        }

        ui.add_space(4.0);
        ui.separator();

        // CPU Timings section
        ui.label(egui::RichText::new("CPU Timings").strong());
        egui::Grid::new("cpu_timings_grid")
            .num_columns(2)
            .spacing([20.0, 4.0])
            .show(ui, |ui| {
                // Video upload time
                ui.label("Video Upload:");
                let video_color = if metrics.video_frame_time_ms > 5.0 {
                    egui::Color32::from_rgb(255, 150, 100)
                } else if metrics.video_frame_time_ms > 2.0 {
                    egui::Color32::from_rgb(255, 230, 100)
                } else {
                    egui::Color32::WHITE
                };
                ui.label(
                    egui::RichText::new(format!("{:.2} ms", metrics.video_frame_time_ms))
                        .monospace()
                        .color(video_color),
                );
                ui.end_row();

                // UI render time
                ui.label("UI Render:");
                let ui_color = if metrics.ui_frame_time_ms > 8.0 {
                    egui::Color32::from_rgb(255, 150, 100)
                } else if metrics.ui_frame_time_ms > 4.0 {
                    egui::Color32::from_rgb(255, 230, 100)
                } else {
                    egui::Color32::WHITE
                };
                ui.label(
                    egui::RichText::new(format!("{:.2} ms", metrics.ui_frame_time_ms))
                        .monospace()
                        .color(ui_color),
                );
                ui.end_row();
            });

        ui.add_space(4.0);
        ui.separator();

        // Resource Counts
        ui.label(egui::RichText::new("Resources").strong());
        egui::Grid::new("resources_grid")
            .num_columns(2)
            .spacing([20.0, 4.0])
            .show(ui, |ui| {
                ui.label("Layers:");
                ui.label(egui::RichText::new(format!("{}", metrics.layer_count)).monospace());
                ui.end_row();

                ui.label("Active clips:");
                ui.label(
                    egui::RichText::new(format!("{}", metrics.active_clip_count)).monospace(),
                );
                ui.end_row();

                ui.label("Effects:");
                ui.label(egui::RichText::new(format!("{}", metrics.effect_count)).monospace());
                ui.end_row();
            });

        ui.add_space(4.0);
        ui.separator();

        // GPU Memory
        ui.label(egui::RichText::new("GPU Memory").strong());
        let total_mb = metrics.gpu_memory.total_mb();
        let formatted = if total_mb >= 1024.0 {
            format!("{:.2} GB", total_mb / 1024.0)
        } else {
            format!("{:.1} MB", total_mb)
        };
        ui.label(egui::RichText::new(formatted).monospace());

        egui::Grid::new("gpu_memory_grid")
            .num_columns(2)
            .spacing([20.0, 2.0])
            .show(ui, |ui| {
                let env_mb = metrics.gpu_memory.environment_texture as f64 / (1024.0 * 1024.0);
                let layers_mb = metrics.gpu_memory.layer_textures as f64 / (1024.0 * 1024.0);
                let effects_mb = metrics.gpu_memory.effect_buffers as f64 / (1024.0 * 1024.0);

                ui.label(egui::RichText::new("Environment:").small());
                ui.label(
                    egui::RichText::new(format!("{:.1} MB", env_mb))
                        .monospace()
                        .small(),
                );
                ui.end_row();

                ui.label(egui::RichText::new("Layers:").small());
                ui.label(
                    egui::RichText::new(format!("{:.1} MB", layers_mb))
                        .monospace()
                        .small(),
                );
                ui.end_row();

                ui.label(egui::RichText::new("Effects:").small());
                ui.label(
                    egui::RichText::new(format!("{:.1} MB", effects_mb))
                        .monospace()
                        .small(),
                );
                ui.end_row();
            });
    }

    /// Render as a floating window
    pub fn render(&mut self, ctx: &egui::Context, metrics: &PerformanceMetrics) {
        if !self.open {
            return;
        }

        egui::Window::new("Performance")
            .default_size([280.0, 400.0])
            .resizable(true)
            .collapsible(true)
            .show(ctx, |ui| {
                self.render_contents(ui, metrics);
            });
    }
}

/// Get color for FPS display based on performance
fn fps_color(fps: f64, target_fps: u32) -> egui::Color32 {
    let target = target_fps as f64;
    let ratio = fps / target;

    if ratio >= 0.95 {
        // Green: at or near target
        egui::Color32::from_rgb(100, 255, 100)
    } else if ratio >= 0.8 {
        // Yellow: slightly below target
        egui::Color32::from_rgb(255, 230, 100)
    } else if ratio >= 0.5 {
        // Orange: significantly below target
        egui::Color32::from_rgb(255, 150, 80)
    } else {
        // Red: very poor performance
        egui::Color32::from_rgb(255, 80, 80)
    }
}
