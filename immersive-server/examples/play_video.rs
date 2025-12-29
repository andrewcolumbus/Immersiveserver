//! Example: Play a video file in a window
//!
//! Usage: cargo run --example play_video <video_file>
//!
//! This example opens a video file and plays it in a window,
//! demonstrating the complete video pipeline: decode -> upload -> render.

use std::env;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use immersive_server::{VideoDecoder, VideoParams, VideoRenderer, VideoTexture};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};

struct VideoPlayer {
    // Window and GPU state
    window: Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,

    // Video state
    decoder: VideoDecoder,
    video_texture: VideoTexture,
    video_renderer: VideoRenderer,
    video_bind_group: wgpu::BindGroup,

    // Playback timing
    start_time: Instant,
    last_frame_pts: f64,
    paused: bool,
    loop_playback: bool,
}

impl VideoPlayer {
    async fn new(window: Arc<Window>, video_path: &Path) -> Self {
        // Open video decoder
        let mut decoder = VideoDecoder::open(video_path).expect("Failed to open video");
        println!(
            "Video: {}x{} @ {:.2}fps, duration: {:.2}s",
            decoder.width(),
            decoder.height(),
            decoder.frame_rate(),
            decoder.duration()
        );

        let size = window.inner_size();

        // Create wgpu instance and surface
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .expect("Failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find GPU adapter");

        println!("Using GPU: {}", adapter.get_info().name);

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Video Player Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .expect("Failed to create device");

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Create video renderer
        let mut video_renderer = VideoRenderer::new(&device, surface_format);

        // Set up aspect ratio preserving params
        let params = VideoParams::fit_aspect_ratio(
            decoder.width(),
            decoder.height(),
            size.width,
            size.height,
        );
        video_renderer.set_params(&queue, params);

        // Create video texture
        let video_texture = VideoTexture::new(&device, decoder.width(), decoder.height());

        // Decode first frame
        if let Ok(Some(frame)) = decoder.decode_next_frame() {
            video_texture.upload(&queue, &frame);
        }

        // Create bind group
        let video_bind_group = video_renderer.create_bind_group(&device, &video_texture);

        Self {
            window,
            device,
            queue,
            surface,
            surface_config,
            decoder,
            video_texture,
            video_renderer,
            video_bind_group,
            start_time: Instant::now(),
            last_frame_pts: 0.0,
            paused: false,
            loop_playback: true,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);

            // Update aspect ratio params
            let params = VideoParams::fit_aspect_ratio(
                self.decoder.width(),
                self.decoder.height(),
                width,
                height,
            );
            self.video_renderer.set_params(&self.queue, params);
        }
    }

    fn update(&mut self) {
        if self.paused {
            return;
        }

        // Calculate current playback time
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let frame_duration = 1.0 / self.decoder.frame_rate();

        // Check if we need a new frame
        if elapsed >= self.last_frame_pts + frame_duration {
            match self.decoder.decode_next_frame() {
                Ok(Some(frame)) => {
                    self.video_texture.upload(&self.queue, &frame);
                    self.last_frame_pts = frame.pts;
                }
                Ok(None) => {
                    // End of video
                    if self.loop_playback {
                        self.decoder.reset().ok();
                        self.start_time = Instant::now();
                        self.last_frame_pts = 0.0;
                        println!("Looping video...");
                    }
                }
                Err(e) => {
                    eprintln!("Decode error: {}", e);
                }
            }
        }
    }

    fn render(&self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Render video to screen
        self.video_renderer
            .render(&mut encoder, &view, &self.video_bind_group, true);

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        if !self.paused {
            // Reset timing on unpause
            self.start_time = Instant::now() - Duration::from_secs_f64(self.last_frame_pts);
        }
        println!("Playback: {}", if self.paused { "PAUSED" } else { "PLAYING" });
    }
}

struct App {
    video_path: String,
    player: Option<VideoPlayer>,
}

impl App {
    fn new(video_path: String) -> Self {
        Self {
            video_path,
            player: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.player.is_some() {
            return;
        }

        let video_path = Path::new(&self.video_path);

        // Create window with video title
        let title = format!(
            "Video Player - {}",
            video_path.file_name().unwrap_or_default().to_string_lossy()
        );

        let window_attributes = WindowAttributes::default()
            .with_title(title)
            .with_inner_size(LogicalSize::new(1280, 720));

        let window = Arc::new(
            event_loop
                .create_window(window_attributes)
                .expect("Failed to create window"),
        );

        let player = pollster::block_on(VideoPlayer::new(window, video_path));
        self.player = Some(player);

        println!("\nControls:");
        println!("  SPACE - Pause/Resume");
        println!("  R     - Restart");
        println!("  ESC   - Quit");
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(player) = &mut self.player else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => {
                            event_loop.exit();
                        }
                        PhysicalKey::Code(KeyCode::Space) => {
                            player.toggle_pause();
                        }
                        PhysicalKey::Code(KeyCode::KeyR) => {
                            player.decoder.reset().ok();
                            player.start_time = Instant::now();
                            player.last_frame_pts = 0.0;
                            println!("Restarted");
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::Resized(size) => {
                player.resize(size.width, size.height);
            }
            WindowEvent::RedrawRequested => {
                player.update();
                match player.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => {
                        player.resize(player.surface_config.width, player.surface_config.height);
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        event_loop.exit();
                    }
                    Err(e) => eprintln!("Render error: {:?}", e),
                }
                player.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(player) = &self.player {
            player.window.request_redraw();
        }
    }
}

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Get video file path from command line
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <video_file>", args[0]);
        eprintln!("Example: {} test.mp4", args[0]);
        std::process::exit(1);
    }

    let video_path = &args[1];
    if !Path::new(video_path).exists() {
        eprintln!("Error: File not found: {}", video_path);
        std::process::exit(1);
    }

    println!("Opening: {}", video_path);

    // Create event loop and run
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new(video_path.clone());
    event_loop.run_app(&mut app).expect("Event loop error");
}

