//! Window output for virtual displays
//!
//! Creates additional windows for output displays, supporting both windowed and fullscreen modes.
//! Uses egui's viewport system to create and manage output windows.

#![allow(dead_code)]

use super::DisplayInfo;
use egui::ViewportId;

/// Represents an output window that can be positioned on any display
pub struct WindowOutput {
    /// Window title
    pub title: String,
    /// Target resolution
    pub width: u32,
    pub height: u32,
    /// Target display ID (None = primary)
    pub target_display: Option<u32>,
    /// Whether to run fullscreen
    pub fullscreen: bool,
    /// Window position (for windowed mode)
    pub position: Option<(i32, i32)>,
    /// Whether the window is currently active
    pub active: bool,
    /// Egui viewport ID for this output window
    pub viewport_id: ViewportId,
    /// Whether to show test pattern instead of video
    pub show_test_pattern: bool,
}

/// Counter for generating unique viewport IDs
static VIEWPORT_ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Generate a unique viewport ID
fn generate_viewport_id() -> ViewportId {
    let id = VIEWPORT_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    ViewportId::from_hash_of(format!("output_viewport_{}", id))
}

impl Default for WindowOutput {
    fn default() -> Self {
        Self {
            title: "Output".to_string(),
            width: 800,
            height: 600,
            target_display: None,
            fullscreen: false,
            position: None,
            active: false,
            viewport_id: generate_viewport_id(),
            show_test_pattern: false,
        }
    }
}

impl WindowOutput {
    /// Create a new windowed output
    pub fn new_windowed(title: String, width: u32, height: u32) -> Self {
        Self {
            title,
            width,
            height,
            target_display: None,
            fullscreen: false,
            position: None,
            active: false,
            viewport_id: generate_viewport_id(),
            show_test_pattern: false,
        }
    }

    /// Create a new fullscreen output on a specific display
    pub fn new_fullscreen(title: String, display: &DisplayInfo) -> Self {
        Self {
            title,
            width: display.resolution.0,
            height: display.resolution.1,
            target_display: Some(display.id),
            fullscreen: true,
            position: Some(display.position),
            active: false,
            viewport_id: generate_viewport_id(),
            show_test_pattern: false,
        }
    }

    /// Get the viewport ID for this output
    pub fn get_viewport_id(&self) -> ViewportId {
        self.viewport_id
    }

    /// Set the target resolution
    pub fn set_resolution(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    /// Set fullscreen mode
    pub fn set_fullscreen(&mut self, fullscreen: bool, display: Option<&DisplayInfo>) {
        self.fullscreen = fullscreen;
        if let Some(d) = display {
            self.target_display = Some(d.id);
            self.position = Some(d.position);
            if fullscreen {
                self.width = d.resolution.0;
                self.height = d.resolution.1;
            }
        }
    }

    /// Start the output window
    /// 
    /// Returns a handle that can be used for rendering.
    /// The actual window creation happens in the egui viewport system during the update loop.
    pub fn start(&mut self) -> Result<WindowHandle, WindowOutputError> {
        if self.active {
            return Err(WindowOutputError::AlreadyActive);
        }

        log::info!(
            "Starting output window '{}' {}×{} {} (viewport: {:?})",
            self.title,
            self.width,
            self.height,
            if self.fullscreen { "fullscreen" } else { "windowed" },
            self.viewport_id
        );

        // Mark as active - the actual viewport will be created in the app update loop
        self.active = true;

        Ok(WindowHandle {
            id: 0,
            width: self.width,
            height: self.height,
            viewport_id: self.viewport_id,
        })
    }

    /// Stop the output window
    pub fn stop(&mut self) {
        if self.active {
            log::info!("Stopping output window '{}'", self.title);
            self.active = false;
        }
    }

    /// Check if the window is active
    pub fn is_active(&self) -> bool {
        self.active
    }
}

/// Handle to an active output window
#[derive(Debug, Clone, Copy)]
pub struct WindowHandle {
    pub id: u64,
    pub width: u32,
    pub height: u32,
    pub viewport_id: ViewportId,
}

/// Errors that can occur with window outputs
#[derive(Debug, Clone)]
pub enum WindowOutputError {
    /// Window is already active
    AlreadyActive,
    /// Failed to create window
    CreationFailed(String),
    /// Display not found
    DisplayNotFound(u32),
    /// Window system error
    WindowSystemError(String),
}

impl std::fmt::Display for WindowOutputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyActive => write!(f, "Window is already active"),
            Self::CreationFailed(msg) => write!(f, "Failed to create window: {}", msg),
            Self::DisplayNotFound(id) => write!(f, "Display {} not found", id),
            Self::WindowSystemError(msg) => write!(f, "Window system error: {}", msg),
        }
    }
}

impl std::error::Error for WindowOutputError {}

/// Render output content to a viewport
/// 
/// This function renders either a test pattern or the video content to the output window.
pub fn render_output_content(
    ui: &mut egui::Ui,
    window: &WindowOutput,
    animation_time: f32,
) {
    let available = ui.available_size();
    let (response, painter) = ui.allocate_painter(available, egui::Sense::hover());
    let rect = response.rect;

    if window.show_test_pattern {
        // Draw SMPTE color bars test pattern
        draw_test_pattern(&painter, rect);
    } else {
        // Draw video placeholder (in a full implementation, this would be the actual video frame)
        draw_video_placeholder(&painter, rect, animation_time);
    }

    // Draw output info overlay
    draw_output_info(&painter, rect, window);
}

/// Draw SMPTE color bars test pattern
fn draw_test_pattern(painter: &egui::Painter, rect: egui::Rect) {
    use egui::{Color32, Pos2, Vec2};

    let colors = [
        Color32::WHITE,
        Color32::YELLOW,
        Color32::from_rgb(0, 255, 255), // Cyan
        Color32::GREEN,
        Color32::from_rgb(255, 0, 255), // Magenta
        Color32::RED,
        Color32::BLUE,
        Color32::BLACK,
    ];

    let bar_width = rect.width() / colors.len() as f32;
    let main_height = rect.height() * 0.75;

    // Main color bars
    for (i, color) in colors.iter().enumerate() {
        let x = rect.min.x + i as f32 * bar_width;
        let bar_rect = egui::Rect::from_min_size(
            Pos2::new(x, rect.min.y),
            Vec2::new(bar_width + 1.0, main_height),
        );
        painter.rect_filled(bar_rect, 0.0, *color);
    }

    // Bottom gradient section
    let gradient_height = rect.height() * 0.15;
    let gradient_y = rect.min.y + main_height;
    let gradient_steps = 32;
    let step_width = rect.width() / gradient_steps as f32;

    for i in 0..gradient_steps {
        let gray = (i as f32 / gradient_steps as f32 * 255.0) as u8;
        let x = rect.min.x + i as f32 * step_width;
        let step_rect = egui::Rect::from_min_size(
            Pos2::new(x, gradient_y),
            Vec2::new(step_width + 1.0, gradient_height),
        );
        painter.rect_filled(step_rect, 0.0, Color32::from_gray(gray));
    }

    // Bottom info bar
    let info_rect = egui::Rect::from_min_size(
        Pos2::new(rect.min.x, gradient_y + gradient_height),
        Vec2::new(rect.width(), rect.height() * 0.10),
    );
    painter.rect_filled(info_rect, 0.0, Color32::from_gray(30));

    // Center crosshair
    let center = Pos2::new(rect.center().x, rect.min.y + main_height / 2.0);
    let cross_size = 30.0;
    painter.line_segment(
        [
            Pos2::new(center.x - cross_size, center.y),
            Pos2::new(center.x + cross_size, center.y),
        ],
        egui::Stroke::new(2.0, Color32::WHITE),
    );
    painter.line_segment(
        [
            Pos2::new(center.x, center.y - cross_size),
            Pos2::new(center.x, center.y + cross_size),
        ],
        egui::Stroke::new(2.0, Color32::WHITE),
    );
    painter.circle_stroke(center, cross_size * 0.7, egui::Stroke::new(1.0, Color32::WHITE));
}

/// Draw video placeholder with animated gradient
fn draw_video_placeholder(painter: &egui::Painter, rect: egui::Rect, time: f32) {
    use egui::{Color32, Pos2, Vec2};

    // Animated gradient background
    let steps = 32;
    let step_width = rect.width() / steps as f32;

    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let wave = ((t * 4.0 + time * 2.0).sin() * 0.5 + 0.5) * 0.3;

        let r = ((t + wave) * 80.0 + 40.0) as u8;
        let g = ((1.0 - t + wave) * 60.0 + 40.0) as u8;
        let b = ((wave * 2.0) * 100.0 + 80.0) as u8;

        let x = rect.min.x + i as f32 * step_width;
        let strip_rect = egui::Rect::from_min_size(
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
            egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 30)),
        );
    }

    // "LIVE" indicator
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "▶ LIVE",
        egui::FontId::proportional(48.0),
        Color32::from_rgba_unmultiplied(255, 255, 255, 180),
    );
}

/// Draw output info overlay
fn draw_output_info(painter: &egui::Painter, rect: egui::Rect, window: &WindowOutput) {
    use egui::{Color32, Pos2, Vec2};

    let margin = 16.0;
    let bg_color = Color32::from_rgba_unmultiplied(0, 0, 0, 180);
    let text_color = Color32::from_gray(220);

    // Top-left: Title and resolution
    let info_text = format!("{} - {}×{}", window.title, window.width, window.height);
    let text_pos = rect.min + Vec2::new(margin, margin);
    let text_rect = egui::Rect::from_min_size(text_pos, Vec2::new(250.0, 24.0));
    painter.rect_filled(text_rect, 4.0, bg_color);
    painter.text(
        text_pos + Vec2::new(8.0, 4.0),
        egui::Align2::LEFT_TOP,
        info_text,
        egui::FontId::proportional(14.0),
        text_color,
    );

    // Top-right: Fullscreen indicator
    let mode_text = if window.fullscreen { "FULLSCREEN" } else { "WINDOWED" };
    let mode_pos = Pos2::new(rect.max.x - margin - 100.0, rect.min.y + margin);
    let mode_rect = egui::Rect::from_min_size(mode_pos, Vec2::new(100.0, 24.0));
    painter.rect_filled(mode_rect, 4.0, bg_color);
    painter.text(
        mode_pos + Vec2::new(8.0, 4.0),
        egui::Align2::LEFT_TOP,
        mode_text,
        egui::FontId::proportional(14.0),
        if window.fullscreen { Color32::GREEN } else { Color32::YELLOW },
    );
}

/// Manager for all output windows
#[derive(Default)]
pub struct WindowManager {
    /// Active output windows by screen ID
    windows: std::collections::HashMap<u32, WindowOutput>,
}

impl WindowManager {
    pub fn new() -> Self {
        Self {
            windows: std::collections::HashMap::new(),
        }
    }

    /// Create a windowed output for a screen
    pub fn create_windowed(
        &mut self,
        screen_id: u32,
        title: String,
        width: u32,
        height: u32,
    ) -> &mut WindowOutput {
        let output = WindowOutput::new_windowed(title, width, height);
        self.windows.insert(screen_id, output);
        self.windows.get_mut(&screen_id).unwrap()
    }

    /// Create a fullscreen output for a screen
    pub fn create_fullscreen(
        &mut self,
        screen_id: u32,
        title: String,
        display: &DisplayInfo,
    ) -> &mut WindowOutput {
        let output = WindowOutput::new_fullscreen(title, display);
        self.windows.insert(screen_id, output);
        self.windows.get_mut(&screen_id).unwrap()
    }

    /// Get an output window
    pub fn get(&self, screen_id: u32) -> Option<&WindowOutput> {
        self.windows.get(&screen_id)
    }

    /// Get an output window mutably
    pub fn get_mut(&mut self, screen_id: u32) -> Option<&mut WindowOutput> {
        self.windows.get_mut(&screen_id)
    }

    /// Remove an output window
    pub fn remove(&mut self, screen_id: u32) {
        if let Some(mut window) = self.windows.remove(&screen_id) {
            window.stop();
        }
    }

    /// Stop all windows
    pub fn stop_all(&mut self) {
        for window in self.windows.values_mut() {
            window.stop();
        }
    }

    /// Get all active windows
    pub fn active_windows(&self) -> impl Iterator<Item = (&u32, &WindowOutput)> {
        self.windows.iter().filter(|(_, w)| w.active)
    }

    /// Get all active windows mutably
    pub fn active_windows_mut(&mut self) -> impl Iterator<Item = (&u32, &mut WindowOutput)> {
        self.windows.iter_mut().filter(|(_, w)| w.active)
    }

    /// Check if any windows are active
    pub fn has_active_windows(&self) -> bool {
        self.windows.values().any(|w| w.active)
    }

    /// Get the count of active windows
    pub fn active_count(&self) -> usize {
        self.windows.values().filter(|w| w.active).count()
    }

    /// Start output for a screen based on its device configuration
    pub fn start_for_screen(
        &mut self,
        screen_id: u32,
        screen: &super::Screen,
        displays: &[DisplayInfo],
    ) -> Result<WindowHandle, WindowOutputError> {
        // Remove existing window if any
        self.remove(screen_id);

        match &screen.device {
            super::OutputDevice::Virtual { width, height, .. } => {
                let output = self.create_windowed(
                    screen_id,
                    screen.name.clone(),
                    *width,
                    *height,
                );
                output.start()
            }
            super::OutputDevice::Fullscreen { display_id, .. } => {
                let display = displays
                    .iter()
                    .find(|d| d.id == *display_id)
                    .ok_or(WindowOutputError::DisplayNotFound(*display_id))?;
                
                let output = self.create_fullscreen(
                    screen_id,
                    screen.name.clone(),
                    display,
                );
                output.start()
            }
            super::OutputDevice::Aqueduct { .. } => {
                // Aqueduct outputs don't need a window
                Err(WindowOutputError::CreationFailed(
                    "Aqueduct outputs don't use windows".to_string(),
                ))
            }
            super::OutputDevice::Ndi { .. } => {
                // NDI outputs don't need a window (coming soon anyway)
                Err(WindowOutputError::CreationFailed(
                    "NDI outputs are not yet supported".to_string(),
                ))
            }
        }
    }
}


