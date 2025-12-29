//! Immersive Server - Main Entry Point
//!
//! A high-performance, cross-platform media server for macOS and Windows.
//! Designed for professional projection mapping, NDI/OMT streaming, and real-time web control.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use immersive_server::settings::{AppPreferences, EnvironmentSettings};
use immersive_server::App;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};

const WINDOW_TITLE: &str = "Immersive Server";

/// Application state machine
enum AppState {
    /// Initial state before window is created
    Uninitialized {
        /// Initial settings to use
        initial_settings: EnvironmentSettings,
        /// Initial file path
        initial_file: Option<PathBuf>,
    },
    /// Window and graphics context are ready
    Running {
        window: Arc<Window>,
        app: App,
        /// Application preferences
        preferences: AppPreferences,
    },
}

/// Main application handler implementing winit's ApplicationHandler trait
struct ImmersiveApp {
    state: AppState,
    next_redraw_at: Instant,
    last_target_fps: u32,
}

impl ImmersiveApp {
    fn new(settings: EnvironmentSettings, initial_file: Option<PathBuf>) -> Self {
        let initial_target_fps = settings.target_fps;
        Self {
            state: AppState::Uninitialized {
                initial_settings: settings,
                initial_file,
            },
            next_redraw_at: Instant::now(),
            last_target_fps: initial_target_fps,
        }
    }

    /// Handle file save action
    fn save_settings(app: &App, path: &PathBuf) -> bool {
        match app.settings.save_to_file(path) {
            Ok(_) => {
                log::info!("Saved settings to: {}", path.display());
                true
            }
            Err(e) => {
                log::error!("Failed to save settings: {}", e);
                false
            }
        }
    }

    /// Show a native file dialog for opening .immersive files
    fn show_open_dialog() -> Option<PathBuf> {
        // Use rfd for file dialogs if available, otherwise use a simple approach
        // For now, we'll use a hardcoded test path or environment variable
        if let Ok(path) = std::env::var("IMMERSIVE_OPEN_FILE") {
            return Some(PathBuf::from(path));
        }

        // Try to use native dialog via command line on macOS
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            let output = Command::new("osascript")
                .args([
                    "-e",
                    r#"POSIX path of (choose file of type {"immersive", "public.xml"} with prompt "Open Immersive Environment")"#,
                ])
                .output();

            if let Ok(output) = output {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path.is_empty() {
                        return Some(PathBuf::from(path));
                    }
                }
            }
        }

        None
    }

    /// Show a native file dialog for saving .immersive files
    fn show_save_dialog() -> Option<PathBuf> {
        if let Ok(path) = std::env::var("IMMERSIVE_SAVE_FILE") {
            return Some(PathBuf::from(path));
        }

        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            let output = Command::new("osascript")
                .args([
                    "-e",
                    r#"POSIX path of (choose file name with prompt "Save Immersive Environment" default name "environment.immersive")"#,
                ])
                .output();

            if let Ok(output) = output {
                if output.status.success() {
                    let mut path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    // Ensure .immersive extension
                    if !path.ends_with(".immersive") {
                        path.push_str(".immersive");
                    }
                    if !path.is_empty() {
                        return Some(PathBuf::from(path));
                    }
                }
            }
        }

        None
    }

    /// Show a native file dialog for opening video files
    fn show_open_video_dialog() -> Option<PathBuf> {
        if let Ok(path) = std::env::var("IMMERSIVE_OPEN_VIDEO") {
            return Some(PathBuf::from(path));
        }

        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            let output = Command::new("osascript")
                .args([
                    "-e",
                    r#"POSIX path of (choose file of type {"public.movie", "public.mpeg-4", "com.apple.quicktime-movie", "public.avi"} with prompt "Open Video File")"#,
                ])
                .output();

            if let Ok(output) = output {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path.is_empty() {
                        return Some(PathBuf::from(path));
                    }
                }
            }
        }

        None
    }
}

impl ApplicationHandler for ImmersiveApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Only initialize if we haven't already
        if let AppState::Uninitialized {
            initial_settings,
            initial_file,
        } = &self.state
        {
            log::info!("Creating window...");

            let settings = initial_settings.clone();
            let file = initial_file.clone();

            // Create window attributes
            let window_attributes = WindowAttributes::default()
                .with_title(WINDOW_TITLE)
                .with_inner_size(LogicalSize::new(
                    settings.window_width,
                    settings.window_height,
                ));

            // Create window
            let window = Arc::new(
                event_loop
                    .create_window(window_attributes)
                    .expect("Failed to create window"),
            );

            log::info!(
                "Window created: {}x{}",
                window.inner_size().width,
                window.inner_size().height
            );

            // Initialize wgpu and egui
            log::info!("Initializing wgpu and egui...");
            let mut app = pollster::block_on(App::new(window.clone(), settings));

            // Set current file if we loaded from one
            app.current_file = file;

            let preferences = AppPreferences::load();

            log::info!("Immersive Server ready!");
            log::info!("Press ESC to exit, F11 for fullscreen");

            self.state = AppState::Running {
                window,
                app,
                preferences,
            };
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // Only handle events if we're running
        let AppState::Running {
            window,
            app,
            preferences,
        } = &mut self.state
        else {
            return;
        };

        // Let egui handle the event first
        let egui_consumed = app.handle_window_event(&event);

        match event {
            // Handle close request
            WindowEvent::CloseRequested => {
                log::info!("Close requested, exiting...");
                event_loop.exit();
            }

            // Handle keyboard input (only if egui doesn't want it)
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key_code),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if !app.egui_wants_keyboard() => {
                match key_code {
                    // Escape to exit
                    KeyCode::Escape => {
                        log::info!("Escape pressed, exiting...");
                        event_loop.exit();
                    }
                    // F11 to toggle fullscreen
                    KeyCode::F11 => {
                        let fullscreen = window.fullscreen();
                        if fullscreen.is_some() {
                            window.set_fullscreen(None);
                            log::info!("Exiting fullscreen");
                        } else {
                            window.set_fullscreen(Some(
                                winit::window::Fullscreen::Borderless(None),
                            ));
                            log::info!("Entering fullscreen");
                        }
                    }
                    // Space to pause/resume video
                    KeyCode::Space => {
                        if app.has_video() {
                            app.toggle_video_pause();
                        }
                    }
                    // R to restart video
                    KeyCode::KeyR => {
                        if app.has_video() {
                            app.restart_video();
                        }
                    }
                    // + or = to zoom in
                    KeyCode::Equal | KeyCode::NumpadAdd => {
                        app.on_keyboard_zoom(true);
                    }
                    // - to zoom out
                    KeyCode::Minus | KeyCode::NumpadSubtract => {
                        app.on_keyboard_zoom(false);
                    }
                    // 0 or Home to reset viewport
                    KeyCode::Digit0 | KeyCode::Home => {
                        app.reset_viewport();
                    }
                    _ => {}
                }
            }

            // Handle window resize
            WindowEvent::Resized(physical_size) => {
                app.resize(physical_size);
            }

            // Handle mouse button events (for viewport panning)
            WindowEvent::MouseInput { state, button, .. } if !egui_consumed => {
                if button == MouseButton::Right {
                    match state {
                        ElementState::Pressed => {
                            // Use the tracked cursor position
                            let (cx, cy) = app.cursor_position();
                            app.on_right_mouse_down(cx, cy);
                        }
                        ElementState::Released => {
                            app.on_right_mouse_up();
                        }
                    }
                }
            }

            // Handle cursor movement (for viewport panning and zoom target)
            WindowEvent::CursorMoved { position, .. } => {
                app.on_mouse_move(position.x as f32, position.y as f32);
            }

            // Handle scroll wheel (for viewport zooming)
            WindowEvent::MouseWheel { delta, .. } if !egui_consumed => {
                let scroll_amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => (pos.y / 50.0) as f32,
                };
                if scroll_amount.abs() > 0.001 {
                    app.on_scroll(scroll_amount);
                }
            }

            // Handle redraw request
            WindowEvent::RedrawRequested => {
                // Check for pending file actions
                if let Some(action) = app.menu_bar.take_pending_action() {
                    use immersive_server::ui::menu_bar::FileAction;
                    match action {
                        FileAction::Open => {
                            if let Some(path) = Self::show_open_dialog() {
                                match EnvironmentSettings::load_from_file(&path) {
                                    Ok(settings) => {
                                        app.settings = settings;
                                        app.current_file = Some(path.clone());
                                        app.menu_bar.sync_from_settings(&app.settings);
                                        preferences.set_last_opened(&path);
                                        app.menu_bar.set_status(format!(
                                            "Opened: {}",
                                            path.file_name()
                                                .map(|s| s.to_string_lossy().to_string())
                                                .unwrap_or_default()
                                        ));
                                        log::info!("Loaded settings from: {}", path.display());
                                    }
                                    Err(e) => {
                                        log::error!("Failed to load settings: {}", e);
                                        app.menu_bar.set_status("Failed to open file");
                                    }
                                }
                            }
                        }
                        FileAction::Save => {
                            if let Some(path) = &app.current_file.clone() {
                                if Self::save_settings(app, path) {
                                    app.menu_bar.set_status("Saved");
                                } else {
                                    app.menu_bar.set_status("Failed to save");
                                }
                            } else {
                                // No current file, show Save As dialog
                                if let Some(path) = Self::show_save_dialog() {
                                    if Self::save_settings(app, &path) {
                                        app.current_file = Some(path.clone());
                                        preferences.set_last_opened(&path);
                                        app.menu_bar.set_status(format!(
                                            "Saved: {}",
                                            path.file_name()
                                                .map(|s| s.to_string_lossy().to_string())
                                                .unwrap_or_default()
                                        ));
                                    } else {
                                        app.menu_bar.set_status("Failed to save");
                                    }
                                }
                            }
                        }
                        FileAction::SaveAs => {
                            if let Some(path) = Self::show_save_dialog() {
                                if Self::save_settings(app, &path) {
                                    app.current_file = Some(path.clone());
                                    preferences.set_last_opened(&path);
                                    app.menu_bar.set_status(format!(
                                        "Saved: {}",
                                        path.file_name()
                                            .map(|s| s.to_string_lossy().to_string())
                                            .unwrap_or_default()
                                    ));
                                } else {
                                    app.menu_bar.set_status("Failed to save");
                                }
                            }
                        }
                        FileAction::OpenVideo => {
                            if let Some(path) = Self::show_open_video_dialog() {
                                match app.load_video(&path) {
                                    Ok(_) => {
                                        app.menu_bar.set_status(format!(
                                            "Loaded: {}",
                                            path.file_name()
                                                .map(|s| s.to_string_lossy().to_string())
                                                .unwrap_or_default()
                                        ));
                                        log::info!("Loaded video: {}", path.display());
                                    }
                                    Err(e) => {
                                        log::error!("Failed to load video: {}", e);
                                        app.menu_bar.set_status("Failed to open video");
                                    }
                                }
                            }
                        }
                    }
                }

                // Begin frame timing
                app.begin_frame();

                // Update viewport animation (rubber-band snap-back)
                app.update_viewport();

                // Update video playback (decode next frame if needed)
                app.update_video();

                // Render frame
                match app.render() {
                    Ok(settings_changed) => {
                        if settings_changed {
                            log::debug!("Settings changed");
                            // Redraw pacing reads `app.settings.target_fps` directly (see `about_to_wait`).
                        }
                    }
                    Err(wgpu::SurfaceError::Lost) => {
                        log::warn!("Surface lost, reconfiguring...");
                        app.resize(app.size());
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        log::error!("Out of GPU memory!");
                        event_loop.exit();
                    }
                    Err(e) => {
                        log::warn!("Surface error: {:?}", e);
                    }
                }

                // End frame: update stats and apply frame rate limiting
                app.end_frame();
            }

            _ => {}
        }

        // Suppress unused variable warning
        let _ = egui_consumed;
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let AppState::Running { window, app, .. } = &mut self.state else {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        };

        // Drive redraws at a stable cadence (target_fps) instead of continuous polling.
        let target_fps = app.settings.target_fps.max(1);
        if target_fps != self.last_target_fps {
            self.last_target_fps = target_fps;
            self.next_redraw_at = Instant::now();
        }

        let frame_duration = Duration::from_secs_f64(1.0 / target_fps as f64);
        let now = Instant::now();

        if now >= self.next_redraw_at {
            window.request_redraw();

            // Advance based on the expected schedule (prevents drift).
            self.next_redraw_at += frame_duration;

            // If we fell behind (e.g., system was busy), reset to avoid "catch-up spirals".
            if self.next_redraw_at < now {
                self.next_redraw_at = now + frame_duration;
            }
        }

        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_redraw_at));
    }
}

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("🎬 Immersive Server v0.1.0");

    // Load preferences and check for last opened file
    let preferences = AppPreferences::load();
    let (settings, initial_file) = if let Some(last_file) = preferences.get_last_opened() {
        log::info!("Loading last opened file: {}", last_file.display());
        match EnvironmentSettings::load_from_file(&last_file) {
            Ok(settings) => (settings, Some(last_file)),
            Err(e) => {
                log::warn!("Failed to load last file: {}", e);
                (EnvironmentSettings::default(), None)
            }
        }
    } else {
        (EnvironmentSettings::default(), None)
    };

    log::info!("Target FPS: {}", settings.target_fps);

    // Create event loop
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    // Default to sleeping; we explicitly schedule redraws in `about_to_wait`.
    event_loop.set_control_flow(ControlFlow::Wait);

    // Create and run application
    let mut app = ImmersiveApp::new(settings, initial_file);

    event_loop.run_app(&mut app).expect("Event loop error");
}
