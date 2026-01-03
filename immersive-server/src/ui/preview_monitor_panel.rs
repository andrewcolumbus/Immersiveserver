//! Preview Monitor Panel
//!
//! Displays a preview of the selected clip before triggering it live.
//! This allows users to see what a clip looks like without affecting the live output.

use crate::preview_player::VideoInfo;

/// Actions that can be returned from the preview monitor panel
#[derive(Debug, Clone)]
pub enum PreviewMonitorAction {
    /// Toggle preview playback (pause/resume)
    TogglePlayback,
    /// Restart preview from beginning
    RestartPreview,
    /// Trigger the previewed clip to its layer (go live)
    TriggerToLayer { layer_id: u32, slot: usize },
}

/// Information about the clip being previewed
#[derive(Debug, Clone)]
pub struct PreviewClipInfo {
    /// Layer ID the clip belongs to
    pub layer_id: u32,
    /// Slot index within the layer
    pub slot: usize,
    /// Clip display name
    pub name: String,
    /// Source path or address
    pub source_info: String,
}

/// State for the preview monitor panel
pub struct PreviewMonitorPanel {
    /// Currently previewing clip info
    current_clip: Option<PreviewClipInfo>,
}

impl Default for PreviewMonitorPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl PreviewMonitorPanel {
    /// Create a new preview monitor panel
    pub fn new() -> Self {
        Self {
            current_clip: None,
        }
    }

    /// Set the clip to preview
    pub fn set_preview_clip(&mut self, info: PreviewClipInfo) {
        self.current_clip = Some(info);
    }

    /// Get the currently previewing clip info
    pub fn current_clip(&self) -> Option<&PreviewClipInfo> {
        self.current_clip.as_ref()
    }

    /// Clear the preview
    pub fn clear_preview(&mut self) {
        self.current_clip = None;
    }

    /// Render the preview monitor panel contents
    ///
    /// # Arguments
    /// * `ui` - egui UI context
    /// * `has_frame` - Whether the preview player has a valid frame to display
    /// * `is_playing` - Whether the preview is currently playing (not paused)
    /// * `video_info` - Video information (dimensions, fps, duration)
    /// * `render_preview` - Callback to render the preview texture into the given rect
    pub fn render_contents<F>(
        &mut self,
        ui: &mut egui::Ui,
        has_frame: bool,
        is_playing: bool,
        video_info: Option<VideoInfo>,
        render_preview: F,
    ) -> Vec<PreviewMonitorAction>
    where
        F: FnOnce(&mut egui::Ui, egui::Rect),
    {
        let mut actions = Vec::new();

        // Clip info header
        if let Some(clip) = &self.current_clip {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&clip.name)
                        .strong()
                        .size(14.0),
                );
            });

            ui.label(
                egui::RichText::new(&clip.source_info)
                    .weak()
                    .small(),
            );

            ui.add_space(4.0);
            ui.separator();
        }

        // Preview area
        let available_size = ui.available_size();
        let preview_height = (available_size.y - 60.0).max(100.0); // Leave room for controls

        // Calculate aspect ratio for preview
        let preview_rect = if let Some(info) = &video_info {
            let aspect = info.width as f32 / info.height as f32;
            let width = available_size.x.min(preview_height * aspect);
            let height = width / aspect;

            // Center the preview
            let x_offset = (available_size.x - width) / 2.0;
            let rect = egui::Rect::from_min_size(
                ui.cursor().min + egui::vec2(x_offset, 0.0),
                egui::vec2(width, height),
            );

            // Reserve the space
            ui.allocate_rect(
                egui::Rect::from_min_size(ui.cursor().min, egui::vec2(available_size.x, height)),
                egui::Sense::hover(),
            );

            rect
        } else {
            // No video info - use default square
            let size = available_size.x.min(preview_height);
            let x_offset = (available_size.x - size) / 2.0;
            let rect = egui::Rect::from_min_size(
                ui.cursor().min + egui::vec2(x_offset, 0.0),
                egui::vec2(size, size),
            );

            ui.allocate_rect(
                egui::Rect::from_min_size(ui.cursor().min, egui::vec2(available_size.x, size)),
                egui::Sense::hover(),
            );

            rect
        };

        // Draw preview background
        ui.painter().rect_filled(
            preview_rect,
            4.0,
            egui::Color32::from_rgb(20, 20, 25),
        );

        if has_frame {
            // Render the preview texture
            render_preview(ui, preview_rect);
        } else if self.current_clip.is_some() {
            // Loading state
            ui.painter().text(
                preview_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Loading...",
                egui::FontId::proportional(14.0),
                egui::Color32::GRAY,
            );
        } else {
            // No clip selected
            ui.painter().text(
                preview_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Click a clip name to preview",
                egui::FontId::proportional(12.0),
                egui::Color32::GRAY,
            );
        }

        ui.add_space(4.0);

        // Timeline / Progress bar
        if let Some(ref info) = video_info {
            let duration = info.duration;
            let position = info.position;
            let progress = if duration > 0.0 {
                (position / duration).clamp(0.0, 1.0) as f32
            } else {
                0.0
            };

            // Time display: current / total
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format_time(position))
                        .small()
                        .monospace(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format_time(duration))
                            .small()
                            .monospace()
                            .weak(),
                    );
                });
            });

            // Progress bar
            let available_width = ui.available_width();
            let bar_height = 6.0;
            let (rect, _response) = ui.allocate_exact_size(
                egui::vec2(available_width, bar_height),
                egui::Sense::hover(),
            );

            // Background
            ui.painter().rect_filled(
                rect,
                2.0,
                egui::Color32::from_gray(40),
            );

            // Progress fill
            let fill_width = rect.width() * progress;
            if fill_width > 0.0 {
                let fill_rect = egui::Rect::from_min_size(
                    rect.min,
                    egui::vec2(fill_width, bar_height),
                );
                ui.painter().rect_filled(
                    fill_rect,
                    2.0,
                    egui::Color32::from_rgb(80, 160, 80),
                );
            }

            ui.add_space(4.0);

            // Video info line
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{}x{} @ {:.1}fps",
                        info.width, info.height, info.frame_rate
                    ))
                    .small()
                    .weak(),
                );
            });
        }

        ui.add_space(6.0);

        // Transport controls
        ui.horizontal(|ui| {
            let enabled = self.current_clip.is_some();

            // Restart button
            if ui
                .add_enabled(enabled, egui::Button::new("⏮").min_size(egui::vec2(28.0, 24.0)))
                .on_hover_text("Restart from beginning")
                .clicked()
            {
                actions.push(PreviewMonitorAction::RestartPreview);
            }

            // Play/Pause button
            let play_pause_icon = if is_playing { "⏸" } else { "▶" };
            if ui
                .add_enabled(enabled, egui::Button::new(play_pause_icon).min_size(egui::vec2(32.0, 24.0)))
                .on_hover_text(if is_playing { "Pause" } else { "Play" })
                .clicked()
            {
                actions.push(PreviewMonitorAction::TogglePlayback);
            }

            ui.add_space(8.0);

            // Trigger (go live) button
            if let Some(clip) = &self.current_clip {
                let layer_id = clip.layer_id;
                let slot = clip.slot;

                if ui
                    .add_enabled(
                        enabled && has_frame,
                        egui::Button::new("▶ GO LIVE")
                            .fill(egui::Color32::from_rgb(40, 120, 40))
                            .min_size(egui::vec2(80.0, 24.0)),
                    )
                    .on_hover_text("Trigger this clip to its layer")
                    .clicked()
                {
                    actions.push(PreviewMonitorAction::TriggerToLayer { layer_id, slot });
                }
            }
        });

        actions
    }
}

/// Format time in MM:SS.f format
fn format_time(seconds: f64) -> String {
    let mins = (seconds / 60.0).floor() as u32;
    let secs = seconds % 60.0;
    format!("{:02}:{:05.2}", mins, secs)
}
