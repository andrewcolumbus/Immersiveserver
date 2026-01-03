//! Camera Effects - Main Entry Point
//!
//! A cross-platform camera effects application with ML-powered visual effects
//! and Syphon/Spout output for integration with immersive-server.

use std::sync::Arc;
use std::time::{Duration, Instant};

use camera_effects::App;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};

const WINDOW_TITLE: &str = "Camera Effects";
const DEFAULT_WIDTH: u32 = 1280;
const DEFAULT_HEIGHT: u32 = 720;
const TARGET_FPS: u32 = 60;

/// Application state machine
enum AppState {
    /// Initial state before window is created
    Uninitialized,
    /// Window and graphics context are ready
    Running { window: Arc<Window>, app: App },
}

/// Main application handler implementing winit's ApplicationHandler trait
struct CameraEffectsApp {
    state: AppState,
    next_redraw_at: Instant,
}

impl CameraEffectsApp {
    fn new() -> Self {
        Self {
            state: AppState::Uninitialized,
            next_redraw_at: Instant::now(),
        }
    }
}

impl ApplicationHandler for CameraEffectsApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Only initialize if we haven't already
        if let AppState::Uninitialized = &self.state {
            log::info!("Creating window...");

            // Create window attributes
            let window_attributes = WindowAttributes::default()
                .with_title(WINDOW_TITLE)
                .with_inner_size(LogicalSize::new(DEFAULT_WIDTH, DEFAULT_HEIGHT));

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
            let app = pollster::block_on(App::new(window.clone()));

            log::info!("Camera Effects ready!");
            log::info!("Press ESC to exit, F11 for fullscreen");

            self.state = AppState::Running { window, app };
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // Only handle events if we're running
        let AppState::Running { window, app } = &mut self.state else {
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
            } if !egui_consumed => {
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
                    // Space to toggle effect
                    KeyCode::Space => {
                        app.toggle_effect();
                    }
                    // 1-3 to select effects
                    KeyCode::Digit1 => app.select_effect(0),
                    KeyCode::Digit2 => app.select_effect(1),
                    KeyCode::Digit3 => app.select_effect(2),
                    // C to connect to camera 0
                    KeyCode::KeyC => app.connect_camera(0),
                    // D to disconnect camera
                    KeyCode::KeyD => app.disconnect_camera(),
                    // T to spawn test particles
                    KeyCode::KeyT => app.spawn_test_particles(100),
                    // M to initialize ML
                    KeyCode::KeyM => app.init_ml(),
                    _ => {}
                }
            }

            // Handle window resize
            WindowEvent::Resized(physical_size) => {
                app.resize(physical_size);
            }

            // Handle cursor movement
            WindowEvent::CursorMoved { position, .. } => {
                app.on_mouse_move(position.x as f32, position.y as f32);
            }

            // Handle redraw request
            WindowEvent::RedrawRequested => {
                // Calculate delta time
                let now = std::time::Instant::now();
                let delta = 1.0 / 60.0; // Fixed timestep for now

                // Update camera frame
                app.update_camera();

                // Update ML inference results
                app.update_ml();

                // Update effects
                app.update_effects(delta);

                // Render frame
                match app.render() {
                    Ok(_) => {}
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
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let AppState::Running { window, .. } = &mut self.state else {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        };

        // Drive redraws at target FPS
        let frame_duration = Duration::from_nanos(1_000_000_000u64 / TARGET_FPS as u64);
        let wake_early = Duration::from_micros(1000);
        let wake_at = self
            .next_redraw_at
            .checked_sub(wake_early)
            .unwrap_or(self.next_redraw_at);
        let now = Instant::now();

        if now >= wake_at {
            // Spin-wait for precise timing
            while Instant::now() < self.next_redraw_at {
                std::hint::spin_loop();
            }

            window.request_redraw();
            self.next_redraw_at += frame_duration;

            // Reset if too far behind
            let max_behind = frame_duration * 2;
            let now_after = Instant::now();
            if now_after > self.next_redraw_at + max_behind {
                self.next_redraw_at = now_after + frame_duration;
            }
        }

        event_loop.set_control_flow(ControlFlow::WaitUntil(wake_at));
    }
}

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Camera Effects v0.1.0");

    // Create event loop
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);

    // Create and run application
    let mut app = CameraEffectsApp::new();
    event_loop.run_app(&mut app).expect("Event loop error");
}
