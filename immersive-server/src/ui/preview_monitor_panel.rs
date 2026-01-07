//! Preview Monitor Panel
//!
//! Displays a preview of the selected clip, layer, or source.
//! - Clip mode: Shows preview of a clip with timeline scrubber and transport controls
//! - Layer mode: Shows live layer output with effects applied (no scrubber)
//! - Source mode: Shows live network source stream (NDI, OMT) with connection info

use crate::compositor::Viewport;
use crate::network::SourceType;
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

/// Information about the network source being previewed
#[derive(Debug, Clone)]
pub struct PreviewSourceInfo {
    /// Type of source (NDI, OMT)
    pub source_type: SourceType,
    /// Display name
    pub name: String,
    /// NDI source name (for NDI sources)
    pub ndi_name: Option<String>,
    /// Network address (for OMT sources)
    pub address: Option<String>,
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
    /// Previewing a network source (live stream, no scrubber)
    Source(PreviewSourceInfo),
}

/// State for the preview monitor panel
pub struct PreviewMonitorPanel {
    /// Current preview mode (clip, layer, or none)
    mode: PreviewMode,
    /// Scrubber state (only used in clip mode)
    scrubber_state: ScrubberState,
    /// Viewport for pan/zoom navigation
    viewport: Viewport,
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
            viewport: Viewport::new(),
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

    /// Set the source to preview (switches to source mode)
    pub fn set_preview_source(&mut self, info: PreviewSourceInfo) {
        self.mode = PreviewMode::Source(info);
    }

    /// Get the currently previewing source info (if in source mode)
    pub fn current_source(&self) -> Option<&PreviewSourceInfo> {
        match &self.mode {
            PreviewMode::Source(info) => Some(info),
            _ => None,
        }
    }

    /// Check if in source preview mode
    pub fn is_source_mode(&self) -> bool {
        matches!(self.mode, PreviewMode::Source(_))
    }

    /// Clear the preview
    pub fn clear_preview(&mut self) {
        self.mode = PreviewMode::None;
    }

    /// Get the viewport (immutable)
    pub fn viewport(&self) -> &Viewport {
        &self.viewport
    }

    /// Get the viewport (mutable)
    pub fn viewport_mut(&mut self) -> &mut Viewport {
        &mut self.viewport
    }

    /// Reset the viewport to default (fit-to-window)
    pub fn reset_viewport(&mut self) {
        self.viewport.reset();
    }

    /// Update viewport animation (call each frame)
    pub fn update_viewport(&mut self, dt: f32, preview_size: (f32, f32), content_size: (f32, f32)) {
        self.viewport.update(dt, preview_size, content_size);
    }

    /// Render the preview monitor panel contents
    ///
    /// # Arguments
    /// * `ui` - egui UI context
    /// * `has_frame` - Whether the preview has a valid frame to display
    /// * `is_playing` - Whether the preview is currently playing (not paused) - only relevant for clip mode
    /// * `video_info` - Video information (dimensions, fps, duration) - only for clip mode
    /// * `layer_dimensions` - Layer video dimensions (width, height) - only for layer mode
    /// * `source_dimensions` - Source video dimensions (width, height) - only for source mode
    /// * `render_preview` - Callback to render the preview texture into the given rect with UV coordinates
    ///   - First Rect is the display rect (where to draw)
    ///   - Second Rect is the UV rect (which part of texture to show, affected by zoom/pan)
    pub fn render_contents<F>(
        &mut self,
        ui: &mut egui::Ui,
        has_frame: bool,
        is_playing: bool,
        video_info: Option<VideoInfo>,
        layer_dimensions: Option<(u32, u32)>,
        source_dimensions: Option<(u32, u32)>,
        render_preview: F,
    ) -> Vec<PreviewMonitorAction>
    where
        F: FnOnce(&mut egui::Ui, egui::Rect, egui::Rect),
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
            PreviewMode::Source(source) => {
                ui.horizontal(|ui| {
                    let icon = match source.source_type {
                        SourceType::Ndi => "📺",
                        SourceType::Omt => "📡",
                    };
                    ui.label(
                        egui::RichText::new(format!("{} ", icon))
                            .size(14.0),
                    );
                    ui.label(
                        egui::RichText::new(&source.name)
                            .strong()
                            .size(14.0),
                    );
                    // Live indicator
                    ui.label(
                        egui::RichText::new(" LIVE")
                            .size(10.0)
                            .color(egui::Color32::from_rgb(255, 80, 80))
                            .strong(),
                    );
                });

                let type_name = match source.source_type {
                    SourceType::Ndi => "NDI",
                    SourceType::Omt => "OMT",
                };
                ui.label(
                    egui::RichText::new(format!("{} network source", type_name))
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
        // Leave more room for controls in clip mode (scrubber + transport), less in layer/source mode
        let controls_height = match &self.mode {
            PreviewMode::Clip(_) => 80.0,
            PreviewMode::Layer(_) => 30.0,
            PreviewMode::Source(_) => 30.0,
            PreviewMode::None => 30.0,
        };
        let preview_height = (available_size.y - controls_height).max(100.0);

        // Get dimensions for aspect ratio calculation
        let dimensions: Option<(u32, u32)> = match &self.mode {
            PreviewMode::Clip(_) => video_info.as_ref().map(|i| (i.width, i.height)),
            PreviewMode::Layer(_) => layer_dimensions,
            PreviewMode::Source(_) => source_dimensions,
            PreviewMode::None => None,
        };

        // Calculate aspect ratio for preview
        let (preview_rect, response) = if let Some((width, height)) = dimensions {
            let aspect = width as f32 / height as f32;
            let display_width = available_size.x.min(preview_height * aspect);
            let display_height = display_width / aspect;

            // Center the preview
            let x_offset = (available_size.x - display_width) / 2.0;
            let rect = egui::Rect::from_min_size(
                ui.cursor().min + egui::vec2(x_offset, 0.0),
                egui::vec2(display_width, display_height),
            );

            // Reserve the space with click_and_drag sense for viewport interaction
            let response = ui.allocate_rect(
                egui::Rect::from_min_size(ui.cursor().min, egui::vec2(available_size.x, display_height)),
                egui::Sense::click_and_drag(),
            );

            (rect, response)
        } else {
            // No dimensions - use default square
            let size = available_size.x.min(preview_height);
            let x_offset = (available_size.x - size) / 2.0;
            let rect = egui::Rect::from_min_size(
                ui.cursor().min + egui::vec2(x_offset, 0.0),
                egui::vec2(size, size),
            );

            let response = ui.allocate_rect(
                egui::Rect::from_min_size(ui.cursor().min, egui::vec2(available_size.x, size)),
                egui::Sense::click_and_drag(),
            );

            (rect, response)
        };

        // Handle viewport interactions (right-click drag for pan, scroll for zoom)
        let content_size = dimensions
            .map(|(w, h)| (w as f32, h as f32))
            .unwrap_or((preview_rect.width(), preview_rect.height()));
        let preview_size = (preview_rect.width(), preview_rect.height());

        // Handle scroll wheel zoom
        if response.hovered() {
            let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
            if scroll_delta.abs() > 0.0 {
                if let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    // Convert to position relative to preview rect
                    let local_pos = (
                        pointer_pos.x - preview_rect.min.x,
                        pointer_pos.y - preview_rect.min.y,
                    );
                    self.viewport.on_scroll(
                        scroll_delta / 50.0, // Scale down scroll sensitivity
                        local_pos,
                        preview_size,
                        content_size,
                    );
                }
            }
        }

        // Handle right-click drag for panning
        if response.dragged_by(egui::PointerButton::Secondary) {
            if let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos()) {
                let local_pos = (
                    pointer_pos.x - preview_rect.min.x,
                    pointer_pos.y - preview_rect.min.y,
                );

                // Start drag if not already dragging
                if !self.viewport.offset().0.is_nan() {
                    // First check if this is a new drag
                    if response.drag_started_by(egui::PointerButton::Secondary) {
                        self.viewport.on_right_mouse_down(local_pos);
                    }
                    self.viewport.on_mouse_move(local_pos, preview_size, content_size);
                }
            }
        } else if response.drag_stopped_by(egui::PointerButton::Secondary) {
            self.viewport.on_right_mouse_up();
        }

        // Handle double-right-click to reset viewport
        if response.double_clicked_by(egui::PointerButton::Secondary) {
            self.viewport.reset();
        }

        // Compute UV rect based on viewport zoom/pan
        let (scale_x, scale_y, offset_x, offset_y) = self.viewport.get_shader_params(preview_size, content_size);

        // Convert shader params to UV rect
        // The shader does: adjusted_uv = (uv - 0.5) / scale + 0.5 + offset
        // So we need to invert this for the UV rect
        let half_width = 0.5 / scale_x;
        let half_height = 0.5 / scale_y;
        let center_u = 0.5 - offset_x / scale_x;
        let center_v = 0.5 - offset_y / scale_y;

        let uv_rect = egui::Rect::from_min_max(
            egui::pos2(center_u - half_width, center_v - half_height),
            egui::pos2(center_u + half_width, center_v + half_height),
        );

        // Draw preview background
        ui.painter().rect_filled(
            preview_rect,
            4.0,
            egui::Color32::from_rgb(20, 20, 25),
        );

        if has_frame {
            // Render the preview texture with viewport-adjusted UVs
            render_preview(ui, preview_rect, uv_rect);
        } else {
            // Show appropriate message based on mode
            let message = match &self.mode {
                PreviewMode::Clip(_) => "Loading...",
                PreviewMode::Layer(_) => "No video playing",
                PreviewMode::Source(_) => "Connecting...",
                PreviewMode::None => "Click a clip, layer, or source to preview",
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

        // Source mode: show dimension info only (no scrubber or transport)
        if let PreviewMode::Source(_) = &self.mode {
            if let Some((width, height)) = source_dimensions {
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
