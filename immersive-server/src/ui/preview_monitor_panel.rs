//! Preview Monitor Panel
//!
//! Displays a preview of the selected clip or layer.
//! - Clip mode: Shows preview of a clip with timeline scrubber and transport controls
//! - Layer mode: Shows live layer output with effects applied (no scrubber)

use crate::preview_player::VideoInfo;
use egui_widgets::{video_scrubber, ScrubberAction, ScrubberState};

/// Actions that can be returned from the preview monitor panel
#[derive(Debug, Clone)]
pub enum PreviewMonitorAction {
    /// Toggle preview playback (pause/resume) - clip mode only
    TogglePlayback,
    /// Restart preview from beginning - clip mode only
    RestartPreview,
    /// Seek to a specific time in seconds - clip mode only
    SeekTo { time_secs: f64 },
    /// Start scrubbing - pause and store play state - clip mode only
    StartScrub,
    /// End scrubbing - seek and restore play state - clip mode only
    EndScrub { time_secs: f64 },
    /// Trigger the previewed clip to its layer (go live) - clip mode only
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

/// Information about the layer being previewed
#[derive(Debug, Clone)]
pub struct PreviewLayerInfo {
    /// Layer ID
    pub layer_id: u32,
    /// Layer display name
    pub name: String,
}

/// Current preview mode
#[derive(Debug, Clone)]
pub enum PreviewMode {
    /// No preview active
    None,
    /// Previewing a clip (with scrubber and transport controls)
    Clip(PreviewClipInfo),
    /// Previewing a layer (live output, no scrubber)
    Layer(PreviewLayerInfo),
}

/// State for the preview monitor panel
pub struct PreviewMonitorPanel {
    /// Current preview mode (clip, layer, or none)
    mode: PreviewMode,
    /// Scrubber state (only used in clip mode)
    scrubber_state: ScrubberState,
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
            mode: PreviewMode::None,
            scrubber_state: ScrubberState::new(),
        }
    }

    /// Set the clip to preview (switches to clip mode)
    pub fn set_preview_clip(&mut self, info: PreviewClipInfo) {
        self.mode = PreviewMode::Clip(info);
    }

    /// Set the layer to preview (switches to layer mode)
    pub fn set_preview_layer(&mut self, info: PreviewLayerInfo) {
        self.mode = PreviewMode::Layer(info);
    }

    /// Get the current preview mode
    pub fn mode(&self) -> &PreviewMode {
        &self.mode
    }

    /// Get the currently previewing clip info (if in clip mode)
    pub fn current_clip(&self) -> Option<&PreviewClipInfo> {
        match &self.mode {
            PreviewMode::Clip(info) => Some(info),
            _ => None,
        }
    }

    /// Get the currently previewing layer info (if in layer mode)
    pub fn current_layer(&self) -> Option<&PreviewLayerInfo> {
        match &self.mode {
            PreviewMode::Layer(info) => Some(info),
            _ => None,
        }
    }

    /// Check if in layer preview mode
    pub fn is_layer_mode(&self) -> bool {
        matches!(self.mode, PreviewMode::Layer(_))
    }

    /// Clear the preview
    pub fn clear_preview(&mut self) {
        self.mode = PreviewMode::None;
    }

    /// Render the preview monitor panel contents
    ///
    /// # Arguments
    /// * `ui` - egui UI context
    /// * `has_frame` - Whether the preview has a valid frame to display
    /// * `is_playing` - Whether the preview is currently playing (not paused) - only relevant for clip mode
    /// * `video_info` - Video information (dimensions, fps, duration) - only for clip mode
    /// * `layer_dimensions` - Layer video dimensions (width, height) - only for layer mode
    /// * `render_preview` - Callback to render the preview texture into the given rect
    pub fn render_contents<F>(
        &mut self,
        ui: &mut egui::Ui,
        has_frame: bool,
        is_playing: bool,
        video_info: Option<VideoInfo>,
        layer_dimensions: Option<(u32, u32)>,
        render_preview: F,
    ) -> Vec<PreviewMonitorAction>
    where
        F: FnOnce(&mut egui::Ui, egui::Rect),
    {
        let mut actions = Vec::new();

        // Header based on mode
        match &self.mode {
            PreviewMode::Clip(clip) => {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("📎 ")
                            .size(14.0),
                    );
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
            PreviewMode::Layer(layer) => {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("🎬 Layer: ")
                            .size(14.0),
                    );
                    ui.label(
                        egui::RichText::new(&layer.name)
                            .strong()
                            .size(14.0),
                    );
                });

                ui.label(
                    egui::RichText::new("Live output with effects")
                        .weak()
                        .small(),
                );

                ui.add_space(4.0);
                ui.separator();
            }
            PreviewMode::None => {}
        }

        // Preview area
        let available_size = ui.available_size();
        // Leave more room for controls in clip mode (scrubber + transport), less in layer mode
        let controls_height = match &self.mode {
            PreviewMode::Clip(_) => 80.0,
            PreviewMode::Layer(_) => 30.0,
            PreviewMode::None => 30.0,
        };
        let preview_height = (available_size.y - controls_height).max(100.0);

        // Get dimensions for aspect ratio calculation
        let dimensions: Option<(u32, u32)> = match &self.mode {
            PreviewMode::Clip(_) => video_info.as_ref().map(|i| (i.width, i.height)),
            PreviewMode::Layer(_) => layer_dimensions,
            PreviewMode::None => None,
        };

        // Calculate aspect ratio for preview
        let preview_rect = if let Some((width, height)) = dimensions {
            let aspect = width as f32 / height as f32;
            let display_width = available_size.x.min(preview_height * aspect);
            let display_height = display_width / aspect;

            // Center the preview
            let x_offset = (available_size.x - display_width) / 2.0;
            let rect = egui::Rect::from_min_size(
                ui.cursor().min + egui::vec2(x_offset, 0.0),
                egui::vec2(display_width, display_height),
            );

            // Reserve the space
            ui.allocate_rect(
                egui::Rect::from_min_size(ui.cursor().min, egui::vec2(available_size.x, display_height)),
                egui::Sense::hover(),
            );

            rect
        } else {
            // No dimensions - use default square
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
        } else {
            // Show appropriate message based on mode
            let message = match &self.mode {
                PreviewMode::Clip(_) => "Loading...",
                PreviewMode::Layer(_) => "No video playing",
                PreviewMode::None => "Click a clip or layer to preview",
            };
            ui.painter().text(
                preview_rect.center(),
                egui::Align2::CENTER_CENTER,
                message,
                egui::FontId::proportional(12.0),
                egui::Color32::GRAY,
            );
        }

        ui.add_space(4.0);

        // Clip mode: show timeline scrubber and transport controls
        if let PreviewMode::Clip(clip) = &self.mode {
            // Timeline / Progress bar with shared scrubber widget
            if let Some(ref info) = video_info {
                let (scrub_actions, _display_pos) = video_scrubber(
                    ui,
                    &mut self.scrubber_state,
                    info.position,
                    info.duration,
                );

                // Convert scrubber actions to preview monitor actions
                for action in scrub_actions {
                    match action {
                        ScrubberAction::StartScrub => {
                            actions.push(PreviewMonitorAction::StartScrub);
                        }
                        ScrubberAction::Seek { time_secs } => {
                            actions.push(PreviewMonitorAction::SeekTo { time_secs });
                        }
                        ScrubberAction::EndScrub { time_secs } => {
                            actions.push(PreviewMonitorAction::EndScrub { time_secs });
                        }
                    }
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
                // Restart button
                if ui
                    .add(egui::Button::new("⏮").min_size(egui::vec2(28.0, 24.0)))
                    .on_hover_text("Restart from beginning")
                    .clicked()
                {
                    actions.push(PreviewMonitorAction::RestartPreview);
                }

                // Play/Pause button
                let play_pause_icon = if is_playing { "⏸" } else { "▶" };
                if ui
                    .add(egui::Button::new(play_pause_icon).min_size(egui::vec2(32.0, 24.0)))
                    .on_hover_text(if is_playing { "Pause" } else { "Play" })
                    .clicked()
                {
                    actions.push(PreviewMonitorAction::TogglePlayback);
                }

                ui.add_space(8.0);

                // Trigger (go live) button
                let layer_id = clip.layer_id;
                let slot = clip.slot;

                if ui
                    .add_enabled(
                        has_frame,
                        egui::Button::new("▶ GO LIVE")
                            .fill(egui::Color32::from_rgb(40, 120, 40))
                            .min_size(egui::vec2(80.0, 24.0)),
                    )
                    .on_hover_text("Trigger this clip to its layer")
                    .clicked()
                {
                    actions.push(PreviewMonitorAction::TriggerToLayer { layer_id, slot });
                }
            });
        }

        // Layer mode: show dimension info only (no scrubber or transport)
        if let PreviewMode::Layer(_) = &self.mode {
            if let Some((width, height)) = layer_dimensions {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{}x{}", width, height))
                            .small()
                            .weak(),
                    );
                });
            }
        }

        actions
    }
}
