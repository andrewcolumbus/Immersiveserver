//! Immersive Server - Main Entry Point
//!
//! A high-performance, cross-platform media server for macOS and Windows.
//! Designed for professional projection mapping, NDI/OMT streaming, and real-time web control.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use immersive_server::settings::{AppPreferences, EnvironmentSettings};
use immersive_server::ui::{activate_macos_app, focus_window_on_click, is_native_menu_supported, NativeMenu, NativeMenuEvent, WindowRegistry};
use immersive_server::App;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, Modifiers, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};

const WINDOW_TITLE: &str = "Immersive Server";

/// Types of file dialogs we can spawn
#[derive(Debug, Clone, Copy)]
enum FileDialogType {
    OpenEnvironment,
    SaveEnvironment,
    OpenVideo,
}

/// Result from an async file dialog
struct FileDialogResult {
    dialog_type: FileDialogType,
    path: Option<PathBuf>,
}

/// Manages async file dialogs that run on background threads
struct AsyncFileDialogs {
    /// Receiver for completed dialogs
    receiver: Receiver<FileDialogResult>,
    /// Sender to pass to spawned threads
    sender: Sender<FileDialogResult>,
    /// Whether a dialog is currently open
    dialog_open: bool,
}

impl AsyncFileDialogs {
    fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            receiver,
            sender,
            dialog_open: false,
        }
    }

    /// Spawn an open environment dialog on a background thread
    fn spawn_open_environment(&mut self) {
        if self.dialog_open {
            return;
        }
        self.dialog_open = true;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let path = Self::run_open_environment_dialog();
            let _ = sender.send(FileDialogResult {
                dialog_type: FileDialogType::OpenEnvironment,
                path,
            });
        });
    }

    /// Spawn a save environment dialog on a background thread
    fn spawn_save_environment(&mut self) {
        if self.dialog_open {
            return;
        }
        self.dialog_open = true;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let path = Self::run_save_environment_dialog();
            let _ = sender.send(FileDialogResult {
                dialog_type: FileDialogType::SaveEnvironment,
                path,
            });
        });
    }

    /// Spawn an open video dialog on a background thread
    fn spawn_open_video(&mut self) {
        if self.dialog_open {
            return;
        }
        self.dialog_open = true;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let path = Self::run_open_video_dialog();
            let _ = sender.send(FileDialogResult {
                dialog_type: FileDialogType::OpenVideo,
                path,
            });
        });
    }

    /// Poll for completed dialogs (non-blocking)
    fn poll(&mut self) -> Option<FileDialogResult> {
        match self.receiver.try_recv() {
            Ok(result) => {
                self.dialog_open = false;
                Some(result)
            }
            Err(_) => None,
        }
    }

    /// Check if a dialog is currently open
    fn is_dialog_open(&self) -> bool {
        self.dialog_open
    }

    #[cfg(target_os = "macos")]
    fn run_open_environment_dialog() -> Option<PathBuf> {
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
        None
    }

    #[cfg(not(target_os = "macos"))]
    fn run_open_environment_dialog() -> Option<PathBuf> {
        None
    }

    #[cfg(target_os = "macos")]
    fn run_save_environment_dialog() -> Option<PathBuf> {
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
                if !path.ends_with(".immersive") {
                    path.push_str(".immersive");
                }
                if !path.is_empty() {
                    return Some(PathBuf::from(path));
                }
            }
        }
        None
    }

    #[cfg(not(target_os = "macos"))]
    fn run_save_environment_dialog() -> Option<PathBuf> {
        None
    }

    #[cfg(target_os = "macos")]
    fn run_open_video_dialog() -> Option<PathBuf> {
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
        None
    }

    #[cfg(not(target_os = "macos"))]
    fn run_open_video_dialog() -> Option<PathBuf> {
        None
    }
}

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
        /// Async file dialogs manager
        file_dialogs: AsyncFileDialogs,
        /// Native OS menu (macOS/Windows only)
        native_menu: Option<NativeMenu>,
        /// Whether we've activated the app (macOS focus fix)
        has_activated: bool,
        /// Registry for tracking all windows (main + panel windows)
        window_registry: WindowRegistry,
    },
}

/// Main application handler implementing winit's ApplicationHandler trait
struct ImmersiveApp {
    state: AppState,
    next_redraw_at: Instant,
    last_target_fps: u32,
    /// Current modifier key state
    modifiers: Modifiers,
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
            modifiers: Modifiers::default(),
        }
    }

    /// Handle file save action
    fn save_settings(app: &mut App, path: &PathBuf) -> bool {
        // Sync layers from environment to settings before saving
        app.sync_layers_to_settings();
        
        match app.settings.save_to_file(path) {
            Ok(_) => {
                tracing::info!("Saved settings to: {}", path.display());
                true
            }
            Err(e) => {
                tracing::error!("Failed to save settings: {}", e);
                false
            }
        }
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
            tracing::info!("Creating window...");

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

            // Explicitly request focus on the window
            window.focus_window();

            tracing::info!(
                "Window created: {}x{}",
                window.inner_size().width,
                window.inner_size().height
            );

            // Initialize wgpu and egui
            tracing::info!("Initializing wgpu and egui...");
            let mut app = pollster::block_on(App::new(window.clone(), settings));

            // Set current file if we loaded from one
            app.current_file = file;

            // Restore layers from settings (if any were saved)
            if !app.settings.layers.is_empty() {
                app.restore_layers_from_settings();
            }

            // Sync OMT broadcast state from settings
            app.sync_omt_broadcast_from_settings();

            // Start API server if enabled
            if app.settings.api_server_enabled {
                app.start_api_server();
            }

            let preferences = AppPreferences::load();

            // Refresh OMT source list for UI
            app.refresh_omt_sources();

            // Initialize native menu on supported platforms
            let native_menu = if is_native_menu_supported() {
                tracing::info!("Initializing native OS menu bar");
                let menu = NativeMenu::new();
                // On Windows, attach menu to window
                menu.attach_to_window(&window);
                // Sync initial show_fps state
                menu.update_show_fps(app.settings.show_fps);
                // Tell app to skip egui menu bar rendering
                app.use_native_menu = true;
                Some(menu)
            } else {
                None
            };

            tracing::info!("Immersive Server ready!");
            tracing::info!("Press ESC to exit, F11 for fullscreen");

            // Initialize window registry and register the main window
            let window_registry = WindowRegistry::new();
            // Note: We don't use the registry's egui state for the main window
            // because App manages its own egui context. This registration is for
            // consistent window tracking when panel windows are added later.
            // For now, just track that we have a main window.

            self.state = AppState::Running {
                window,
                app,
                preferences,
                file_dialogs: AsyncFileDialogs::new(),
                native_menu,
                has_activated: false,
                window_registry,
            };
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        // Only handle events if we're running
        let AppState::Running {
            window,
            app,
            preferences,
            file_dialogs,
            native_menu,
            has_activated,
            window_registry,
        } = &mut self.state
        else {
            return;
        };

        // Check if this is the main window or a panel window
        let is_main_window = window.id() == window_id;

        // For now, only handle events for the main window
        // Panel window event handling will be added in Phase E
        if !is_main_window {
            // Future: Route to panel window handler
            // For now, handle basic panel window events
            if let WindowEvent::CloseRequested = event {
                // Panel window closed - mark for cleanup
                // In Phase E, this will trigger re-docking
                window_registry.mark_closed(window_id);
                tracing::info!("Panel window closed");
            }
            return;
        }

        // Let egui handle the event first
        let egui_consumed = app.handle_window_event(&event);

        match event {
            // Handle close request
            WindowEvent::CloseRequested => {
                tracing::info!("Close requested, exiting...");
                event_loop.exit();
            }

            // Track modifier keys
            WindowEvent::ModifiersChanged(new_modifiers) => {
                self.modifiers = new_modifiers;
            }

            // Handle Cmd/Ctrl+S for save (always, even when egui wants keyboard)
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::KeyS),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if self.modifiers.state().super_key() || self.modifiers.state().control_key() => {
                // Trigger save action
                app.menu_bar.pending_action = Some(immersive_server::ui::menu_bar::FileAction::Save);
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
                        tracing::info!("Escape pressed, exiting...");
                        event_loop.exit();
                    }
                    // F11 to toggle fullscreen
                    KeyCode::F11 => {
                        let fullscreen = window.fullscreen();
                        if fullscreen.is_some() {
                            window.set_fullscreen(None);
                            tracing::info!("Exiting fullscreen");
                        } else {
                            window.set_fullscreen(Some(
                                winit::window::Fullscreen::Borderless(None),
                            ));
                            tracing::info!("Entering fullscreen");
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

            // On macOS, ensure window gets focus when clicked (always, even if egui consumes)
            WindowEvent::MouseInput { state: ElementState::Pressed, .. } => {
                focus_window_on_click(&window);
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
                // On first frame, activate the app to steal focus from terminal (macOS)
                if !*has_activated {
                    *has_activated = true;
                    activate_macos_app();
                    focus_window_on_click(window);
                }

                // Poll native menu events (macOS/Windows)
                if let Some(menu) = native_menu {
                    loop {
                        match menu.poll_events() {
                            NativeMenuEvent::FileAction(action) => {
                                app.menu_bar.pending_action = Some(action);
                            }
                            NativeMenuEvent::MenuAction(action) => {
                                app.menu_bar.pending_menu_action = Some(action);
                            }
                            NativeMenuEvent::ShowFpsToggled(show) => {
                                app.settings.show_fps = show;
                            }
                            NativeMenuEvent::OpenPreferences => {
                                app.preferences_window.open = true;
                            }
                            NativeMenuEvent::Exit => {
                                tracing::info!("Exit requested via menu");
                                event_loop.exit();
                                return;
                            }
                            NativeMenuEvent::None => break,
                        }
                    }
                }

                // Check for pending file actions - spawn async dialogs
                if let Some(action) = app.menu_bar.take_pending_action() {
                    use immersive_server::ui::menu_bar::FileAction;
                    match action {
                        FileAction::Open => {
                            if !file_dialogs.is_dialog_open() {
                                file_dialogs.spawn_open_environment();
                            }
                        }
                        FileAction::Save => {
                            if let Some(path) = &app.current_file.clone() {
                                // Save directly to existing file (no dialog needed)
                                if Self::save_settings(app, path) {
                                    app.menu_bar.set_status("Saved");
                                } else {
                                    app.menu_bar.set_status("Failed to save");
                                }
                            } else {
                                // No current file, show Save As dialog
                                if !file_dialogs.is_dialog_open() {
                                    file_dialogs.spawn_save_environment();
                                }
                            }
                        }
                        FileAction::SaveAs => {
                            if !file_dialogs.is_dialog_open() {
                                file_dialogs.spawn_save_environment();
                            }
                        }
                        FileAction::OpenVideo => {
                            if !file_dialogs.is_dialog_open() {
                                file_dialogs.spawn_open_video();
                            }
                        }
                    }
                }

                // Poll for completed async file dialogs
                if let Some(result) = file_dialogs.poll() {
                    match result.dialog_type {
                        FileDialogType::OpenEnvironment => {
                            if let Some(path) = result.path {
                                match EnvironmentSettings::load_from_file(&path) {
                                    Ok(settings) => {
                                        app.settings = settings;
                                        app.current_file = Some(path.clone());
                                        app.restore_layers_from_settings();
                                        app.sync_omt_broadcast_from_settings();
                                        preferences.set_last_opened(&path);
                                        app.menu_bar.set_status(format!(
                                            "Opened: {}",
                                            path.file_name()
                                                .map(|s| s.to_string_lossy().to_string())
                                                .unwrap_or_default()
                                        ));
                                        tracing::info!("Loaded settings from: {}", path.display());
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to load settings: {}", e);
                                        app.menu_bar.set_status("Failed to open file");
                                    }
                                }
                            }
                        }
                        FileDialogType::SaveEnvironment => {
                            if let Some(path) = result.path {
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
                        FileDialogType::OpenVideo => {
                            if let Some(path) = result.path {
                                app.complete_clip_assignment(path);
                            } else {
                                // User cancelled - clear any pending assignment
                                app.clip_grid_panel.pending_clip_assignment = None;
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
                            tracing::debug!("Settings changed");
                            // Redraw pacing reads `app.settings.target_fps` directly (see `about_to_wait`).
                        }
                    }
                    Err(wgpu::SurfaceError::Lost) => {
                        tracing::warn!("Surface lost, reconfiguring...");
                        app.resize(app.size());
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        tracing::error!("Out of GPU memory!");
                        event_loop.exit();
                    }
                    Err(e) => {
                        tracing::warn!("Surface error: {:?}", e);
                    }
                }

                // End frame: update stats and apply frame rate limiting
                app.end_frame();

                // Sync native menu states with app state
                if let Some(menu) = native_menu {
                    let panel_states = app.get_panel_states();
                    menu.update_panel_states(&panel_states);
                    menu.update_show_fps(app.settings.show_fps);
                }
            }

            _ => {}
        }

        // Suppress unused variable warning
        let _ = egui_consumed;
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let AppState::Running { window, app, window_registry, .. } = &mut self.state else {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        };

        // Clean up any closed panel windows
        let closed_windows = window_registry.cleanup_closed_windows();
        for closed in closed_windows {
            if let Some(panel_id) = closed.panel_id() {
                tracing::info!("Cleaned up panel window: {}", panel_id);
                // Future: Trigger re-docking logic here
            }
        }

        let target_fps = app.settings.target_fps.max(1);
        if target_fps != self.last_target_fps {
            self.last_target_fps = target_fps;
            self.next_redraw_at = Instant::now();
        }

        // Integer nanoseconds to eliminate floating-point drift
        let frame_nanos = 1_000_000_000u64 / target_fps as u64;
        let frame_duration = Duration::from_nanos(frame_nanos);

        let now = Instant::now();

        // Check if we're within 2ms of target - if so, spin-wait for precision
        let spin_threshold = Duration::from_micros(2000);
        if now < self.next_redraw_at {
            if self.next_redraw_at.duration_since(now) <= spin_threshold {
                // Spin-wait the final microseconds
                while Instant::now() < self.next_redraw_at {
                    std::hint::spin_loop();
                }
            } else {
                // Still waiting - wake 1ms early next time
                let wake_at = self.next_redraw_at
                    .checked_sub(Duration::from_micros(1000))
                    .unwrap_or(self.next_redraw_at);
                event_loop.set_control_flow(ControlFlow::WaitUntil(wake_at));
                return;
            }
        }

        // Time to render
        window.request_redraw();
        self.next_redraw_at += frame_duration;

        // Reset if more than 2 frames behind
        if Instant::now() > self.next_redraw_at + frame_duration * 2 {
            self.next_redraw_at = Instant::now() + frame_duration;
        }

        // Schedule next wake 1ms early
        let wake_at = self.next_redraw_at
            .checked_sub(Duration::from_micros(1000))
            .unwrap_or(self.next_redraw_at);
        event_loop.set_control_flow(ControlFlow::WaitUntil(wake_at));
    }
}

fn main() {
    // Initialize logging with tracing
    use immersive_server::telemetry::{init_logging, LogConfig};
    if let Err(e) = init_logging(&LogConfig::default()) {
        eprintln!("Failed to initialize logging: {}", e);
    }

    tracing::info!("Immersive Server v0.1.0");

    // Load preferences and check for last opened file
    let preferences = AppPreferences::load();
    let (settings, initial_file) = if let Some(last_file) = preferences.get_last_opened() {
        tracing::info!("Loading last opened file: {}", last_file.display());
        match EnvironmentSettings::load_from_file(&last_file) {
            Ok(settings) => (settings, Some(last_file)),
            Err(e) => {
                tracing::warn!("Failed to load last file: {}", e);
                (EnvironmentSettings::default(), None)
            }
        }
    } else {
        (EnvironmentSettings::default(), None)
    };

    tracing::info!("Target FPS: {}", settings.target_fps);

    // Create event loop
    #[cfg(target_os = "macos")]
    let event_loop = {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        let mut builder = EventLoop::builder();
        // Disable winit's default macOS menu so muda can take over
        builder.with_default_menu(false);
        // Ensure the app activates and takes focus
        builder.with_activate_ignoring_other_apps(true);
        builder.build().expect("Failed to create event loop")
    };
    #[cfg(not(target_os = "macos"))]
    let event_loop = EventLoop::new().expect("Failed to create event loop");

    // Default to sleeping; we explicitly schedule redraws in `about_to_wait`.
    event_loop.set_control_flow(ControlFlow::Wait);

    // Create and run application
    let mut app = ImmersiveApp::new(settings, initial_file);

    event_loop.run_app(&mut app).expect("Event loop error");
}
