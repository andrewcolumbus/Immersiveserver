//! Example: Decode a video file and print frame information
//!
//! Usage: cargo run --example decode_video <video_file>
//!
//! This example opens a video file, decodes the first 10 frames,
//! and prints information about each frame.

use std::env;
use std::path::Path;

use immersive_server::VideoDecoder;

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

    // Print video info
    println!("\n=== Video Information ===");
    println!("Resolution: {}x{}", decoder.width(), decoder.height());
    println!("Frame rate: {:.2} fps", decoder.frame_rate());
    println!("Duration: {:.2} seconds", decoder.duration());
    println!(
        "Estimated frames: {}",
        decoder.estimated_frame_count()
    );
    println!();

    // Decode first 10 frames
    println!("=== Decoding first 10 frames ===");
    let max_frames = 10;
    let mut decoded_count = 0;

    while decoded_count < max_frames {
        match decoder.decode_next_frame() {
            Ok(Some(frame)) => {
                println!(
                    "Frame {}: {}x{}, PTS: {:.3}s, data size: {} bytes",
                    frame.frame_index,
                    frame.width,
                    frame.height,
                    frame.pts,
                    frame.data.len()
                );
                decoded_count += 1;
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

    println!("\nDecoded {} frames successfully!", decoded_count);

    // Test reset functionality
    println!("\n=== Testing reset ===");
    if let Err(e) = decoder.reset() {
        eprintln!("Reset failed: {}", e);
    } else {
        println!("Reset successful, current frame index: {}", decoder.current_frame_index());
        
        // Decode one more frame to verify reset worked
        if let Ok(Some(frame)) = decoder.decode_next_frame() {
            println!("After reset, first frame PTS: {:.3}s", frame.pts);
        }
    }

    println!("\nDone!");
}





