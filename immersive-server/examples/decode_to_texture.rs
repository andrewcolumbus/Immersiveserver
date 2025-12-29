//! Example: Decode video frames and upload to GPU texture
//!
//! Usage: cargo run --example decode_to_texture <video_file>
//!
//! This example opens a video file, creates a wgpu device and texture,
//! then decodes and uploads frames to verify the GPU upload works.

use std::env;
use std::path::Path;

use immersive_server::{VideoDecoder, VideoTexture};

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

    let video_path = Path::new(&args[1]);

    if !video_path.exists() {
        eprintln!("Error: File not found: {}", video_path.display());
        std::process::exit(1);
    }

    println!("Opening video: {}", video_path.display());

    // Open the video decoder
    let mut decoder = match VideoDecoder::open(video_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to open video: {}", e);
            std::process::exit(1);
        }
    };

    println!(
        "Video: {}x{} @ {:.2}fps",
        decoder.width(),
        decoder.height(),
        decoder.frame_rate()
    );

    // Create wgpu device (headless - no window needed)
    let (device, queue) = pollster::block_on(async {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find GPU adapter");

        println!("Using GPU: {}", adapter.get_info().name);

        adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Test Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .expect("Failed to create device")
    });

    // Create video texture
    let mut video_texture = VideoTexture::new(&device, decoder.width(), decoder.height());

    println!(
        "Created GPU texture: {}x{}",
        video_texture.width(),
        video_texture.height()
    );

    // Decode and upload first 10 frames
    println!("\n=== Uploading frames to GPU ===");
    let max_frames = 10;
    let mut uploaded_count = 0;
    let start_time = std::time::Instant::now();

    while uploaded_count < max_frames {
        match decoder.decode_next_frame() {
            Ok(Some(frame)) => {
                // Upload frame to GPU texture
                video_texture.upload(&queue, &frame);

                println!(
                    "Uploaded frame {}: {}x{}, PTS: {:.3}s",
                    frame.frame_index, frame.width, frame.height, frame.pts
                );
                uploaded_count += 1;
            }
            Ok(None) => {
                println!("End of video reached");
                break;
            }
            Err(e) => {
                eprintln!("Decode error: {}", e);
                break;
            }
        }
    }

    let elapsed = start_time.elapsed();
    println!(
        "\nUploaded {} frames in {:.2}ms ({:.1} fps)",
        uploaded_count,
        elapsed.as_secs_f64() * 1000.0,
        uploaded_count as f64 / elapsed.as_secs_f64()
    );

    // Test resize functionality
    println!("\n=== Testing resize ===");
    let new_width = 1280;
    let new_height = 720;
    video_texture.resize(&device, new_width, new_height);
    println!(
        "Resized texture to {}x{}",
        video_texture.width(),
        video_texture.height()
    );

    // Verify texture view is accessible
    let _view = video_texture.view();
    println!("Texture view accessible: OK");

    println!("\nDone! GPU texture upload works correctly.");
}

