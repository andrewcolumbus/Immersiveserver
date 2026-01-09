//! Immersive Server - Main Entry Point
//!
//! A high-performance, cross-platform media server for macOS and Windows.
//! Designed for professional projection mapping, NDI/OMT streaming, and real-time web control.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use immersive_server::output::DisplayManager;
use immersive_server::settings::{AppPreferences, EnvironmentSettings};
use immersive_server::ui::{activate_macos_app, focus_window_on_click, is_native_menu_supported, DockAction, NativeMenu, NativeMenuEvent, WindowRegistry};
use immersive_server::App;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, Modifiers, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId, WindowLevel};

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
        /// Manager for connected displays (for multi-output)
        display_manager: DisplayManager,
    },
}

/// Main application handler implementing winit's ApplicationHandler trait
struct ImmersiveApp {
    state: AppState,
    next_redraw_at: Instant,
    last_target_fps: u32,
    /// Current modifier key state
    modifiers: Modifiers,
    /// Last time we checked for display hot-plug events
    last_display_check: Instant,
}

/// How often to check for display hot-plug events (in seconds)
const DISPLAY_CHECK_INTERVAL_SECS: u64 = 1;

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
            last_display_check: Instant::now(),
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

    /// Handle dock actions (create/destroy panel windows)
    fn handle_dock_actions(
        event_loop: &ActiveEventLoop,
        app: &mut App,
        window_registry: &mut WindowRegistry,
        gpu: &Arc<immersive_server::GpuContext>,
    ) {
        let actions = app.dock_manager.take_pending_actions();
        for action in actions {
            match action {
                DockAction::UndockPanel { panel_id, position, size } => {
                    tracing::info!("Creating window for undocked panel: {}", panel_id);

                    // Get panel title for window
                    let title = app.dock_manager
                        .get_panel(&panel_id)
                        .map(|p| p.title.clone())
                        .unwrap_or_else(|| panel_id.clone());

                    // Create the window
                    let window_attrs = WindowAttributes::default()
                        .with_title(title)
                        .with_inner_size(LogicalSize::new(size.0 as f64, size.1 as f64))
                        .with_position(winit::dpi::LogicalPosition::new(position.0 as f64, position.1 as f64));

                    match event_loop.create_window(window_attrs) {
                        Ok(window) => {
                            let window = Arc::new(window);
                            // Register in window registry
                            // Note: For UI-only panels, we don't need GPU rendering
                            // but we still need egui state for the panel content
                            window_registry.register_panel_window(
                                window.clone(),
                                panel_id.clone(),
                                gpu,
                            );
                            tracing::info!("Panel window created: {}", panel_id);
                        }
                        Err(e) => {
                            tracing::error!("Failed to create panel window: {}", e);
                            // Revert the undock
                            app.dock_manager.redock_panel(&panel_id);
                        }
                    }
                }
                DockAction::RedockPanel { panel_id } => {
                    tracing::info!("Closing window for re-docked panel: {}", panel_id);
                    // The window will be closed when it receives CloseRequested
                    // or we can close it programmatically here
                    if let Some(window_id) = window_registry.get_panel_window_id(&panel_id) {
                        window_registry.mark_closed(window_id);
                    }
                }
                DockAction::ClosePanel { panel_id } => {
                    tracing::info!("Closing panel: {}", panel_id);
                    // Close the window if it's undocked
                    if let Some(window_id) = window_registry.get_panel_window_id(&panel_id) {
                        window_registry.mark_closed(window_id);
                    }
                }
                DockAction::BreakoutEnvironment { position, size } => {
                    tracing::info!("Breaking out environment viewport to separate window");

                    // Create the window for the environment viewport
                    let window_attrs = WindowAttributes::default()
                        .with_title("Immersive Environment")
                        .with_inner_size(winit::dpi::LogicalSize::new(size.0 as f64, size.1 as f64))
                        .with_position(winit::dpi::LogicalPosition::new(position.0 as f64, position.1 as f64));

                    match event_loop.create_window(window_attrs) {
                        Ok(window) => {
                            let window = Arc::new(window);
                            window_registry.register_environment_window(window.clone(), gpu);
                            app.environment_broken_out = true;
                            tracing::info!("Environment viewport window created");
                        }
                        Err(e) => {
                            tracing::error!("Failed to create environment viewport window: {}", e);
                        }
                    }
                }
                DockAction::RedockEnvironment => {
                    tracing::info!("Returning environment viewport to main window");
                    // Close the environment window if it exists
                    if let Some(window_id) = window_registry.environment_window_id() {
                        window_registry.mark_closed(window_id);
                    }
                    app.environment_broken_out = false;
                }
            }
        }
    }

    /// Handle display window creation/destruction for screens with Display output devices
    fn handle_display_windows(
        event_loop: &ActiveEventLoop,
        app: &mut App,
        window_registry: &mut WindowRegistry,
        display_manager: &DisplayManager,
        gpu: &Arc<immersive_server::GpuContext>,
    ) {
        // Collect stale and pending windows info first (immutable borrow)
        let (stale_screens, pending) = {
            let Some(output_manager) = app.output_manager() else {
                return;
            };
            (
                output_manager.stale_display_windows(),
                output_manager.pending_display_windows(),
            )
        };

        // Handle stale windows (mutable borrow)
        if !stale_screens.is_empty() {
            if let Some(output_manager) = app.output_manager_mut() {
                for screen_id in stale_screens {
                    if let Some(window_id) = output_manager.remove_window_for_screen(screen_id) {
                        tracing::info!("Closing stale display window for screen {:?}", screen_id);
                        window_registry.mark_closed(window_id);
                    }
                }
            }
        }

        // Handle pending windows - collect info we need
        struct PendingWindow {
            screen_id: immersive_server::output::ScreenId,
            display_name: String,
            monitor_handle: winit::monitor::MonitorHandle,
            width: u32,
            height: u32,
        }

        let mut windows_to_create: Vec<PendingWindow> = Vec::new();

        for (screen_id, display_id) in pending {
            // Get display info
            let Some(display_info) = display_manager.get(display_id) else {
                tracing::warn!(
                    "Screen {:?} references display {} which is not connected",
                    screen_id,
                    display_id
                );
                continue;
            };

            // Get screen dimensions (immutable borrow)
            let Some(output_manager) = app.output_manager() else {
                continue;
            };
            let Some(screen) = output_manager.get_screen(screen_id) else {
                continue;
            };

            windows_to_create.push(PendingWindow {
                screen_id,
                display_name: display_info.name.clone(),
                monitor_handle: display_info.monitor_handle().clone(),
                width: screen.width,
                height: screen.height,
            });
        }

        // Now create the windows
        for pending_window in windows_to_create {
            tracing::info!(
                "Creating display window for screen {:?} on {} ({}x{})",
                pending_window.screen_id,
                pending_window.display_name,
                pending_window.width,
                pending_window.height
            );

            // Create fullscreen window on this display
            let window_attrs = WindowAttributes::default()
                .with_title(format!("Immersive Output - {}", pending_window.display_name))
                .with_fullscreen(Some(winit::window::Fullscreen::Borderless(Some(
                    pending_window.monitor_handle,
                ))));

            match event_loop.create_window(window_attrs) {
                Ok(window) => {
                    let window = Arc::new(window);
                    let window_id = window.id();

                    // Register in window registry
                    window_registry.register_monitor_window(
                        window.clone(),
                        pending_window.screen_id.0, // Use screen ID as output_id
                        gpu,
                    );

                    // Associate window with screen in OutputManager
                    if let Some(output_manager) = app.output_manager_mut() {
                        output_manager.set_window_for_screen(pending_window.screen_id, window_id);
                    }

                    tracing::info!(
                        "Display window created for screen {:?} on {}",
                        pending_window.screen_id,
                        pending_window.display_name
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to create display window for screen {:?}: {}",
                        pending_window.screen_id,
                        e
                    );
                }
            }
        }
    }

    /// Handle display hot-plug events (connect/disconnect)
    ///
    /// Checks for display changes and handles:
    /// - Disconnected displays: closes monitor windows, falls back to Virtual
    /// - Connected displays: syncs to app for UI updates
    fn handle_display_hotplug(
        event_loop: &ActiveEventLoop,
        app: &mut App,
        window_registry: &mut WindowRegistry,
        display_manager: &mut DisplayManager,
    ) {
        use immersive_server::output::{DisplayEvent, OutputDevice, ScreenId};

        // Check for display changes
        let events = display_manager.check_connections(event_loop);

        if events.is_empty() {
            return;
        }

        // Update available displays in App for UI
        app.set_available_displays(display_manager.displays().cloned().collect());

        // Process each event
        for event in events {
            match event {
                DisplayEvent::Connected(info) => {
                    tracing::info!(
                        "Display connected: {} ({}x{}) id={}",
                        info.name,
                        info.size.0,
                        info.size.1,
                        info.id
                    );
                    // Note: We don't auto-activate screens on reconnect
                    // User must manually select the display again in UI
                }
                DisplayEvent::Disconnected(display_id) => {
                    tracing::warn!("Display disconnected: id={}", display_id);

                    // Find all screens using this display and fall back to Virtual
                    if let Some(output_manager) = app.output_manager_mut() {
                        let screens_to_update: Vec<ScreenId> = output_manager
                            .screens()
                            .filter_map(|s| {
                                if let OutputDevice::Display { display_id: id } = &s.device {
                                    if *id == display_id {
                                        return Some(s.id);
                                    }
                                }
                                None
                            })
                            .collect();

                        for screen_id in screens_to_update {
                            // Close any monitor window for this screen
                            if let Some(window_id) = output_manager.remove_window_for_screen(screen_id) {
                                tracing::info!(
                                    "Closing monitor window for screen {:?} (display disconnected)",
                                    screen_id
                                );
                                window_registry.mark_closed(window_id);
                            }

                            // Fall back to Virtual
                            if let Some(screen) = output_manager.get_screen_mut(screen_id) {
                                screen.device = OutputDevice::Virtual;
                                tracing::info!(
                                    "Screen {:?} '{}' fell back to Virtual (display disconnected)",
                                    screen_id,
                                    screen.name
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// Render content into an undocked panel window
    fn render_panel_window(
        window_id: winit::window::WindowId,
        window_registry: &mut WindowRegistry,
        app: &mut App,
    ) {
        // First, extract what we need from the entry before borrowing gpu_context mutably
        let Some(entry) = window_registry.get(window_id) else {
            return;
        };

        let Some(panel_id) = entry.panel_id().map(|s| s.to_string()) else {
            return;
        };

        if entry.gpu_context.is_none() {
            return;
        }

        let size = entry.window.inner_size();

        // Now get mutable access
        let entry = window_registry.get_mut(window_id).unwrap();
        let gpu_context = entry.gpu_context.as_mut().unwrap();

        let gpu = app.gpu_context();

        // Get surface texture
        let surface_texture = match gpu_context.surface.get_current_texture() {
            Ok(tex) => tex,
            Err(e) => {
                tracing::warn!("Failed to get panel window surface: {:?}", e);
                return;
            }
        };

        let view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Use the persistent egui context for this panel window
        let egui_ctx = &entry.egui_ctx;

        // Begin egui pass for this window using the persistent context
        // Note: Don't call set_pixels_per_point() - let egui_winit handle it via raw_input.pixels_per_point
        // (calling it would cause zoom_factor to be derived as ppp/native, leading to double-scaling)
        let raw_input = entry.egui_state.take_egui_input(&entry.window);
        egui_ctx.begin_pass(raw_input);

        // For Preview Monitor panel, register the preview texture with this window's egui_renderer
        // since egui texture IDs are renderer-specific and not transferable between windows
        use immersive_server::ui::dock::panel_ids;
        let preview_texture_id = if panel_id == panel_ids::PREVIEW_MONITOR {
            app.preview_texture_view().map(|view| {
                gpu_context.egui_renderer.register_native_texture(
                    &gpu.device,
                    view,
                    wgpu::FilterMode::Linear,
                )
            })
        } else {
            None
        };

        // Render panel content using App's public method
        // Use a frame with no margins that fills the entire window
        let mut should_redock = false;
        let mut should_close = false;
        let panel_frame = egui::Frame::NONE
            .fill(egui::Color32::from_gray(30))
            .inner_margin(egui::Margin::same(8));
        egui::CentralPanel::default()
            .frame(panel_frame)
            .show(egui_ctx, |ui| {
                (should_redock, should_close) = app.render_undocked_panel(&panel_id, ui, preview_texture_id);
            });

        // Clean up the temporary texture registration for Preview Monitor
        if let Some(tex_id) = preview_texture_id {
            gpu_context.egui_renderer.free_texture(&tex_id);
        }

        let full_output = egui_ctx.end_pass();

        // Handle platform output
        entry.egui_state.handle_platform_output(&entry.window, full_output.platform_output);

        // Tessellate and render
        // Use full_output.pixels_per_point to match what egui_winit uses for input coordinate conversion
        // (egui_winit includes zoom_factor, not just scale_factor)
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [size.width, size.height],
            pixels_per_point: full_output.pixels_per_point,
        };

        let clipped_primitives = egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);

        // Upload textures
        for (id, delta) in &full_output.textures_delta.set {
            gpu_context.egui_renderer.update_texture(&gpu.device, &gpu.queue, *id, delta);
        }

        // Create encoder and render
        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Panel Window Encoder"),
        });

        gpu_context.egui_renderer.update_buffers(
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            &clipped_primitives,
            &screen_descriptor,
        );

        // Render pass in its own scope so it's dropped before encoder.finish()
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Panel Window Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // SAFETY: The render_pass is used only within this block and dropped
            // before the encoder is finished.
            let render_pass_static: &mut wgpu::RenderPass<'static> =
                unsafe { std::mem::transmute(&mut render_pass) };

            gpu_context.egui_renderer.render(render_pass_static, &clipped_primitives, &screen_descriptor);
        }

        gpu.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();

        // Free textures
        for id in &full_output.textures_delta.free {
            gpu_context.egui_renderer.free_texture(id);
        }

        // Handle redock request after rendering
        if should_redock {
            app.dock_manager.request_redock(&panel_id);
        }

        // Handle close request
        if should_close {
            app.dock_manager.request_close(&panel_id);
        }
    }

    /// Render content into a monitor output window
    fn render_monitor_window(
        window_id: winit::window::WindowId,
        window_registry: &mut WindowRegistry,
        app: &mut App,
    ) {
        use immersive_server::output::ScreenId;

        // Get the window entry
        let Some(entry) = window_registry.get(window_id) else {
            return;
        };

        // Get the output_id (which is screen_id.0)
        let Some(output_id) = entry.output_id() else {
            return;
        };
        let screen_id = ScreenId(output_id);

        // Get GPU context reference
        let Some(gpu_context) = entry.gpu_context.as_ref() else {
            return;
        };
        let surface_format = gpu_context.config.format;

        // Get the surface texture
        let surface_texture = match gpu_context.surface.get_current_texture() {
            Ok(tex) => tex,
            Err(e) => {
                tracing::warn!("Failed to get monitor window surface: {:?}", e);
                return;
            }
        };

        let surface_view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Get GPU and OutputManager
        let gpu = app.gpu_context();

        // Create command encoder
        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Monitor Window Encoder"),
        });

        // Use OutputManager to blit screen content to surface (mutable for delay buffer)
        let presented = if let Some(output_manager) = app.output_manager_mut() {
            output_manager.present_to_surface(
                &gpu.device,
                &mut encoder,
                screen_id,
                &surface_view,
                surface_format,
            )
        } else {
            false
        };

        if !presented {
            // No content to present - clear to black
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Monitor Window Clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        // Submit and present
        gpu.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
    }

    /// Render the environment viewport to its own window
    fn render_environment_window(
        window_id: winit::window::WindowId,
        window_registry: &mut WindowRegistry,
        app: &mut App,
    ) {
        // Get the window entry
        let Some(entry) = window_registry.get(window_id) else {
            return;
        };

        // Get GPU context reference
        let Some(gpu_context) = entry.gpu_context.as_ref() else {
            return;
        };

        // Get the surface texture
        let surface_texture = match gpu_context.surface.get_current_texture() {
            Ok(tex) => tex,
            Err(e) => {
                tracing::warn!("Failed to get environment window surface: {:?}", e);
                return;
            }
        };

        let surface_view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Get the window size for viewport calculations
        let size = entry.window.inner_size();

        // Render the environment to this window using App's copy pipeline
        app.render_environment_to_surface(&surface_view, size.width, size.height);

        // Present
        surface_texture.present();
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

            // Restore output manager screens from settings
            app.sync_output_manager_from_settings();

            // Sync OMT broadcast state from settings
            app.sync_omt_broadcast_from_settings();

            // Start API server if enabled
            if app.settings.api_server_enabled {
                app.start_api_server();
            }

            let preferences = AppPreferences::load();

            // Sync sources panel state with discovery settings
            app.sources_panel.set_omt_discovery_enabled(app.settings.omt_discovery_enabled);
            app.sources_panel.set_ndi_discovery_enabled(app.settings.ndi_discovery_enabled);

            // Refresh OMT source list for UI (if discovery is enabled)
            if app.settings.omt_discovery_enabled {
                app.refresh_omt_sources();
            }

            // Start NDI discovery if enabled in settings
            if app.settings.ndi_discovery_enabled {
                app.start_ndi_discovery();
            }

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

            // Initialize display manager and enumerate connected displays
            let mut display_manager = DisplayManager::new();
            display_manager.refresh(event_loop);
            tracing::info!(
                "Enumerated {} connected displays",
                display_manager.count()
            );

            // Sync available displays to App for UI
            app.set_available_displays(display_manager.displays().cloned().collect());

            self.state = AppState::Running {
                window,
                app,
                preferences,
                file_dialogs: AsyncFileDialogs::new(),
                native_menu,
                has_activated: false,
                window_registry,
                display_manager,
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
            display_manager,
        } = &mut self.state
        else {
            return;
        };

        // Check if this is the main window or a panel window
        let is_main_window = window.id() == window_id;

        // Handle non-main window events (panel windows, monitor windows, environment viewport)
        if !is_main_window {
            // Determine what kind of window this is
            let is_monitor = window_registry.get(window_id).map(|e| e.is_monitor()).unwrap_or(false);
            let is_environment_viewport = window_registry.get(window_id).map(|e| e.is_environment_viewport()).unwrap_or(false);

            if is_environment_viewport {
                // Handle environment viewport window events
                match event {
                    WindowEvent::CloseRequested => {
                        // Environment window closed - return to main window
                        tracing::info!("Environment viewport window closed, returning to main window");
                        app.environment_broken_out = false;
                        window_registry.mark_closed(window_id);
                    }
                    WindowEvent::Resized(new_size) => {
                        // Update environment window GPU context
                        let gpu = app.gpu_context();
                        if let Some(entry) = window_registry.get_mut(window_id) {
                            entry.resize(&gpu, new_size);
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        // Render the environment viewport
                        Self::render_environment_window(window_id, window_registry, app);
                    }
                    WindowEvent::KeyboardInput { event, .. } => {
                        // Handle Escape to close environment window
                        if event.state == ElementState::Pressed {
                            if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
                                tracing::info!("Environment viewport window closed via Escape");
                                app.environment_broken_out = false;
                                window_registry.mark_closed(window_id);
                            }
                        }
                    }
                    // Handle mouse input for viewport panning
                    WindowEvent::MouseInput { state, button, .. } => {
                        if button == MouseButton::Right {
                            // Get cursor position from window entry
                            let cursor_pos = window_registry
                                .get(window_id)
                                .map(|e| e.cursor_position())
                                .unwrap_or((0.0, 0.0));

                            match state {
                                ElementState::Pressed => {
                                    tracing::debug!(
                                        target: "viewport",
                                        context = "undocked_env",
                                        x = cursor_pos.0,
                                        y = cursor_pos.1,
                                        "right_mouse_down"
                                    );
                                    app.on_right_mouse_down(cursor_pos.0, cursor_pos.1);
                                }
                                ElementState::Released => {
                                    tracing::debug!(target: "viewport", context = "undocked_env", "right_mouse_up");
                                    app.on_right_mouse_up();
                                }
                            }
                            // Request redraw to show pan changes
                            if let Some(entry) = window_registry.get_mut(window_id) {
                                entry.window.request_redraw();
                            }
                        }
                    }
                    // Handle cursor movement for pan tracking
                    WindowEvent::CursorMoved { position, .. } => {
                        // Update cursor position in window entry
                        if let Some(entry) = window_registry.get_mut(window_id) {
                            entry.set_cursor_position(position.x as f32, position.y as f32);
                        }

                        tracing::trace!(
                            target: "viewport",
                            context = "undocked_env",
                            x = position.x,
                            y = position.y,
                            "cursor_moved"
                        );

                        // Use the same approach as main window - on_mouse_move handles dragging internally
                        app.on_mouse_move(position.x as f32, position.y as f32);

                        // Request redraw to update the viewport
                        if let Some(entry) = window_registry.get_mut(window_id) {
                            entry.window.request_redraw();
                        }
                    }
                    // Handle scroll wheel for zooming
                    WindowEvent::MouseWheel { delta, .. } => {
                        let scroll_amount = match delta {
                            MouseScrollDelta::LineDelta(_, y) => y,
                            MouseScrollDelta::PixelDelta(pos) => (pos.y / 50.0) as f32,
                        };

                        if scroll_amount.abs() > 0.001 {
                            tracing::debug!(
                                target: "viewport",
                                context = "undocked_env",
                                delta = scroll_amount,
                                "scroll"
                            );
                            app.on_scroll(scroll_amount);
                            // Request redraw
                            if let Some(entry) = window_registry.get_mut(window_id) {
                                entry.window.request_redraw();
                            }
                        }
                    }
                    _ => {
                        // Forward other events to egui for viewport controls
                        if let Some(entry) = window_registry.get_mut(window_id) {
                            let _ = entry.egui_state.on_window_event(&entry.window, &event);
                        }
                    }
                }
            } else if is_monitor {
                // Handle monitor window events
                match event {
                    WindowEvent::CloseRequested => {
                        use immersive_server::output::ScreenId;
                        // Monitor window closed - update screen output device
                        if let Some(entry) = window_registry.get(window_id) {
                            if let Some(output_id) = entry.output_id() {
                                let screen_id = ScreenId(output_id);
                                tracing::info!("Monitor window closed for screen {:?}", screen_id);
                                // Remove the window association
                                if let Some(output_manager) = app.output_manager_mut() {
                                    output_manager.remove_window_for_screen(screen_id);
                                    // Set screen device to Virtual
                                    if let Some(screen) = output_manager.get_screen_mut(screen_id) {
                                        screen.device = immersive_server::output::OutputDevice::Virtual;
                                    }
                                }
                            }
                        }
                        window_registry.mark_closed(window_id);
                    }
                    WindowEvent::Resized(new_size) => {
                        // Update monitor window GPU context
                        let gpu = app.gpu_context();
                        if let Some(entry) = window_registry.get_mut(window_id) {
                            entry.resize(&gpu, new_size);
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        // Render the screen content
                        Self::render_monitor_window(window_id, window_registry, app);
                    }
                    WindowEvent::KeyboardInput { event, .. } => {
                        // Handle Escape to close monitor window
                        if event.state == ElementState::Pressed {
                            if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
                                use immersive_server::output::ScreenId;
                                // Same logic as CloseRequested - close window and fall back to Virtual
                                if let Some(entry) = window_registry.get(window_id) {
                                    if let Some(output_id) = entry.output_id() {
                                        let screen_id = ScreenId(output_id);
                                        tracing::info!("Monitor window closed via Escape for screen {:?}", screen_id);
                                        if let Some(output_manager) = app.output_manager_mut() {
                                            output_manager.remove_window_for_screen(screen_id);
                                            if let Some(screen) = output_manager.get_screen_mut(screen_id) {
                                                screen.device = immersive_server::output::OutputDevice::Virtual;
                                            }
                                        }
                                    }
                                }
                                window_registry.mark_closed(window_id);
                            }
                        }
                    }
                    _ => {
                        // Monitor windows don't need other event handling
                    }
                }
            } else {
                // Handle panel window events
                match event {
                    WindowEvent::CloseRequested => {
                        // Panel window closed - trigger re-docking
                        if let Some(entry) = window_registry.get(window_id) {
                            if let Some(panel_id) = entry.panel_id() {
                                tracing::info!("Panel window closed, re-docking: {}", panel_id);
                                app.dock_manager.redock_panel(panel_id);
                            }
                        }
                        window_registry.mark_closed(window_id);
                    }
                    WindowEvent::Resized(new_size) => {
                        // Update panel window GPU context
                        let gpu = app.gpu_context();
                        if let Some(entry) = window_registry.get_mut(window_id) {
                            entry.resize(&gpu, new_size);
                            // Also forward to egui_state so it knows the new window size
                            let _ = entry.egui_state.on_window_event(&entry.window, &event);
                            // Update panel geometry
                            if let Some(panel_id) = entry.panel_id().map(|s| s.to_string()) {
                                if let Some(panel) = app.dock_manager.get_panel_mut(&panel_id) {
                                    panel.undocked_geometry.set_size(new_size.width as f32, new_size.height as f32);
                                }
                            }
                        }
                    }
                    WindowEvent::Focused(gained) => {
                        // Track which window is focused
                        if gained {
                            window_registry.set_focused(window_id);
                        }
                        // Forward to egui
                        if let Some(entry) = window_registry.get_mut(window_id) {
                            let _ = entry.egui_state.on_window_event(&entry.window, &event);
                        }
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        // Track cursor position for this window
                        if let Some(entry) = window_registry.get_mut(window_id) {
                            entry.cursor_pos = Some((position.x as f32, position.y as f32));
                            // Forward to egui
                            let _ = entry.egui_state.on_window_event(&entry.window, &event);
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        // Render the undocked panel
                        Self::render_panel_window(window_id, window_registry, app);
                    }
                    _ => {
                        // Forward other events to egui for the panel window
                        if let Some(entry) = window_registry.get_mut(window_id) {
                            let _ = entry.egui_state.on_window_event(&entry.window, &event);
                        }
                    }
                }
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

            // Handle Cmd/Ctrl+Shift+A to toggle Advanced Output window and window level
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::KeyA),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if (self.modifiers.state().super_key() || self.modifiers.state().control_key())
                && self.modifiers.state().shift_key() =>
            {
                app.advanced_output_window.toggle();
                // Set window level based on Advanced Output visibility
                if app.advanced_output_window.open {
                    window.set_window_level(WindowLevel::AlwaysOnTop);
                } else {
                    window.set_window_level(WindowLevel::Normal);
                }
            }

            // Handle Cmd/Ctrl+Shift+L to toggle test pattern
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::KeyL),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if (self.modifiers.state().super_key() || self.modifiers.state().control_key())
                && self.modifiers.state().shift_key() =>
            {
                app.toggle_test_pattern();
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

            // Handle mouse button events
            WindowEvent::MouseInput { state, button, .. } => {
                // On macOS, ensure window gets focus when clicked (always, even if egui consumes)
                if state == ElementState::Pressed {
                    focus_window_on_click(&window);
                }

                // Handle right-click for viewport panning (only when egui doesn't consume)
                // This prevents double-handling when panning on egui panels like preview monitor
                if button == MouseButton::Right && !egui_consumed {
                    match state {
                        ElementState::Pressed => {
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

            // Handle scroll wheel for viewport zooming (only when egui doesn't consume it)
            // This prevents double-handling when scrolling over egui panels like preview monitor
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
                            NativeMenuEvent::ShowBpmToggled(show) => {
                                app.settings.show_bpm = show;
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
                                        app.sync_output_manager_from_settings();
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

                // Track Advanced Output window state to detect close via X button
                let was_advanced_output_open = app.advanced_output_window.open;

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

                // Check if Advanced Output was closed via X button
                if was_advanced_output_open && !app.advanced_output_window.open {
                    window.set_window_level(WindowLevel::Normal);
                }

                // End frame: update stats and apply frame rate limiting
                app.end_frame();

                // Sync native menu states with app state
                if let Some(menu) = native_menu {
                    let panel_states = app.get_panel_states();
                    menu.update_panel_states(&panel_states);
                    menu.update_show_fps(app.settings.show_fps);
                }

                // Handle dock actions (create/destroy panel windows)
                let gpu = app.gpu_context();
                Self::handle_dock_actions(event_loop, app, window_registry, &gpu);

                // Handle display window creation (for screens with Display output devices)
                Self::handle_display_windows(
                    event_loop,
                    app,
                    window_registry,
                    display_manager,
                    &gpu,
                );
            }

            _ => {}
        }

        // Suppress unused variable warning
        let _ = egui_consumed;
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let AppState::Running { window, app, window_registry, display_manager, .. } = &mut self.state else {
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

        // Check for display hot-plug events periodically
        let now = Instant::now();
        if now.duration_since(self.last_display_check) >= Duration::from_secs(DISPLAY_CHECK_INTERVAL_SECS) {
            self.last_display_check = now;
            Self::handle_display_hotplug(event_loop, app, window_registry, display_manager);
        }

        // VSYNC mode: let the display control timing via Fifo present mode
        if app.settings.vsync_enabled {
            window.request_redraw();
            for (_, entry) in window_registry.iter() {
                entry.window.request_redraw();
            }
            event_loop.set_control_flow(ControlFlow::Poll);
            return;
        }

        // Manual FPS control mode: use precise frame timing
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

        // Also request redraws for all panel windows
        for (_, entry) in window_registry.iter() {
            entry.window.request_redraw();
        }

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
    let log_config = LogConfig {
        console_enabled: true,
        file_enabled: false,
        file_path: None,
        json_format: false,
        default_level: "info".to_string(),
    };
    // Keep the guard alive for the program duration
    let _log_guard = match init_logging(&log_config) {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Failed to initialize logging: {}", e);
            None
        }
    };

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
