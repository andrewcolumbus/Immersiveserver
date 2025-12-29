//! Preview/Composition Monitor
//!
//! Displays a real-time preview of the composition output, similar to Resolume's preview monitor.

use crate::video::VideoPlayer;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

/// Preview monitor display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreviewMode {
    /// Show the main composition output
    #[default]
    Composition,
    /// Show a specific screen's output
    Screen(usize),
}

/// Preview monitor UI component
pub struct PreviewMonitor {
    /// Current preview mode
    pub mode: PreviewMode,
    /// Whether the preview is expanded
    pub expanded: bool,
    /// Preview aspect ratio (width / height)
    pub aspect_ratio: f32,
    /// Show frame info overlay
    pub show_info: bool,
    /// Animation time for test pattern
    animation_time: f32,
}

impl Default for PreviewMonitor {
    fn default() -> Self {
        Self {
            mode: PreviewMode::Composition,
            expanded: true,
            aspect_ratio: 16.0 / 9.0,
            show_info: true,
            animation_time: 0.0,
        }
    }
}

impl PreviewMonitor {
    /// Create a new preview monitor
    pub fn new() -> Self {
        Self::default()
    }

    /// Show the preview monitor panel
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        player: &VideoPlayer,
        screen_count: usize,
        composition_texture: Option<egui::TextureId>,
    ) {
        // Header with controls
        ui.horizontal(|ui| {
            ui.heading("📺 Preview");
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Expand/collapse toggle
                let expand_icon = if self.expanded { "▼" } else { "▶" };
                if ui.button(expand_icon).clicked() {
                    self.expanded = !self.expanded;
                }
                
                // Info overlay toggle
                ui.checkbox(&mut self.show_info, "Info");
            });
        });

        if !self.expanded {
            return;
        }

        ui.separator();

        // Mode selector
        ui.horizontal(|ui| {
            ui.label("View:");
            ui.selectable_value(&mut self.mode, PreviewMode::Composition, "Composition");
            
            // Screen options
            for i in 0..screen_count {
                ui.selectable_value(&mut self.mode, PreviewMode::Screen(i), format!("Screen {}", i + 1));
            }
        });

        ui.separator();

        // Preview area
        let available_width = ui.available_width();
        let preview_height = (available_width / self.aspect_ratio).min(300.0);
        let preview_size = Vec2::new(available_width, preview_height);
        
        let (response, painter) = ui.allocate_painter(preview_size, egui::Sense::click());
        let rect = response.rect;

        // Draw preview content
        match self.mode {
            PreviewMode::Composition => {
                self.draw_composition_preview(&painter, rect, player, composition_texture);
            }
            PreviewMode::Screen(idx) => {
                self.draw_screen_preview(&painter, rect, player, idx, composition_texture);
            }
        }

        // Draw border
        painter.rect_stroke(rect, 4.0, Stroke::new(2.0, Color32::from_gray(60)));

        // Info overlay
        if self.show_info {
            self.draw_info_overlay(&painter, rect, player);
        }

        // Update animation time
        self.animation_time += ui.input(|i| i.predicted_dt);
    }

    /// Draw the composition preview (video frame or placeholder)
    fn draw_composition_preview(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        player: &VideoPlayer,
        texture: Option<egui::TextureId>,
    ) {
        // Background
        painter.rect_filled(rect, 4.0, Color32::from_gray(20));

        if let Some(tex_id) = texture {
            // Draw the actual composition texture
            painter.image(
                tex_id,
                rect,
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        } else if player.is_loaded() {
            // Draw animated gradient to represent video playback
            self.draw_video_placeholder(painter, rect, player);
            
            // "COMPOSITION" label
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "COMPOSITION OUTPUT",
                egui::FontId::proportional(16.0),
                Color32::from_rgba_unmultiplied(255, 255, 255, 180),
            );
        } else {
            // No video loaded
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No Video Loaded",
                egui::FontId::proportional(18.0),
                Color32::from_gray(100),
            );
        }
    }

    /// Draw a screen-specific preview
    fn draw_screen_preview(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        player: &VideoPlayer,
        screen_idx: usize,
        texture: Option<egui::TextureId>,
    ) {
        // Background
        painter.rect_filled(rect, 4.0, Color32::from_gray(20));

        if let Some(tex_id) = texture {
            // For now, screens also show the main composition
            // In a full implementation, this would show the warped output for that screen
            painter.image(
                tex_id,
                rect,
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
            
            painter.text(
                rect.left_top() + Vec2::new(8.0, 8.0),
                egui::Align2::LEFT_TOP,
                format!("SCREEN {}", screen_idx + 1),
                egui::FontId::proportional(12.0),
                Color32::from_rgba_unmultiplied(255, 255, 255, 180),
            );
        } else if player.is_loaded() {
            self.draw_video_placeholder(painter, rect, player);
            
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("SCREEN {} OUTPUT", screen_idx + 1),
                egui::FontId::proportional(16.0),
                Color32::from_rgba_unmultiplied(255, 255, 255, 180),
            );
        } else {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("Screen {} - No Video", screen_idx + 1),
                egui::FontId::proportional(18.0),
                Color32::from_gray(100),
            );
        }
    }

    /// Draw an animated video placeholder
    fn draw_video_placeholder(&self, painter: &egui::Painter, rect: Rect, player: &VideoPlayer) {
        let progress = player.progress();
        let time = self.animation_time;
        
        // Animated gradient background
        let steps = 32;
        let step_width = rect.width() / steps as f32;
        
        for i in 0..steps {
            let t = i as f32 / steps as f32;
            let wave = ((t * 4.0 + time * 2.0).sin() * 0.5 + 0.5) * 0.3;
            let _base = 0.1 + progress * 0.2;
            
            let r = ((t + wave) * 80.0 + 40.0) as u8;
            let g = ((1.0 - t + wave) * 60.0 + 40.0) as u8;
            let b = ((wave * 2.0) * 100.0 + 80.0) as u8;
            
            let x = rect.min.x + i as f32 * step_width;
            let strip_rect = Rect::from_min_size(
                Pos2::new(x, rect.min.y),
                Vec2::new(step_width + 1.0, rect.height()),
            );
            
            painter.rect_filled(strip_rect, 0.0, Color32::from_rgb(r, g, b));
        }

        // Scanlines effect
        for y in (0..rect.height() as i32).step_by(4) {
            let line_y = rect.min.y + y as f32;
            painter.line_segment(
                [Pos2::new(rect.min.x, line_y), Pos2::new(rect.max.x, line_y)],
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 30)),
            );
        }
    }

    /// Draw info overlay
    fn draw_info_overlay(&self, painter: &egui::Painter, rect: Rect, player: &VideoPlayer) {
        let margin = 8.0;
        let bg_color = Color32::from_rgba_unmultiplied(0, 0, 0, 180);
        let text_color = Color32::from_gray(220);
        
        // Top-left: Resolution
        if let Some((w, h)) = player.dimensions() {
            let text = format!("{}×{}", w, h);
            let text_pos = rect.min + Vec2::new(margin, margin);
            let text_rect = Rect::from_min_size(text_pos, Vec2::new(80.0, 18.0));
            painter.rect_filled(text_rect, 2.0, bg_color);
            painter.text(
                text_pos + Vec2::new(4.0, 2.0),
                egui::Align2::LEFT_TOP,
                text,
                egui::FontId::monospace(12.0),
                text_color,
            );
        }

        // Top-right: FPS
        let fps_text = format!("{:.1} fps", player.current_fps());
        let fps_pos = Pos2::new(rect.max.x - margin - 70.0, rect.min.y + margin);
        let fps_rect = Rect::from_min_size(fps_pos, Vec2::new(70.0, 18.0));
        painter.rect_filled(fps_rect, 2.0, bg_color);
        painter.text(
            fps_pos + Vec2::new(4.0, 2.0),
            egui::Align2::LEFT_TOP,
            fps_text,
            egui::FontId::monospace(12.0),
            if player.current_fps() >= 30.0 { Color32::GREEN } else { Color32::YELLOW },
        );

        // Bottom-left: Timecode
        let time = player.current_time();
        let duration = player.duration();
        let timecode = format!(
            "{:02}:{:02}.{:02} / {:02}:{:02}.{:02}",
            (time / 60.0) as u32,
            (time % 60.0) as u32,
            ((time % 1.0) * 100.0) as u32,
            (duration / 60.0) as u32,
            (duration % 60.0) as u32,
            ((duration % 1.0) * 100.0) as u32,
        );
        let tc_pos = Pos2::new(rect.min.x + margin, rect.max.y - margin - 18.0);
        let tc_rect = Rect::from_min_size(tc_pos, Vec2::new(160.0, 18.0));
        painter.rect_filled(tc_rect, 2.0, bg_color);
        painter.text(
            tc_pos + Vec2::new(4.0, 2.0),
            egui::Align2::LEFT_TOP,
            timecode,
            egui::FontId::monospace(12.0),
            text_color,
        );

        // Bottom-right: Play state
        let state_text = if player.is_playing() { "▶ PLAY" } else { "⏸ PAUSE" };
        let state_color = if player.is_playing() { Color32::GREEN } else { Color32::YELLOW };
        let state_pos = Pos2::new(rect.max.x - margin - 60.0, rect.max.y - margin - 18.0);
        let state_rect = Rect::from_min_size(state_pos, Vec2::new(60.0, 18.0));
        painter.rect_filled(state_rect, 2.0, bg_color);
        painter.text(
            state_pos + Vec2::new(4.0, 2.0),
            egui::Align2::LEFT_TOP,
            state_text,
            egui::FontId::monospace(12.0),
            state_color,
        );
    }

    /// Set the aspect ratio based on video dimensions
    pub fn set_aspect_ratio(&mut self, width: u32, height: u32) {
        if height > 0 {
            self.aspect_ratio = width as f32 / height as f32;
        }
    }
}

