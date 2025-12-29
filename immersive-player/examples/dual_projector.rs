//! Dual Projector Example
//!
//! Demonstrates setting up a dual-projector configuration with edge blending.

use immersive_player::output::{EdgeBlend, OutputManager, Screen, Slice};
use immersive_player::project::ProjectPreset;

fn main() {
    // Initialize logging
    env_logger::init();
    
    println!("Dual Projector Setup Example");
    println!("============================\n");

    // Create output manager
    let mut output_manager = OutputManager::new();
    
    // Create dual projector setup with 200px overlap
    println!("Creating dual projector setup with 200px blend overlap...\n");
    
    // Left projector - blends on the right edge
    let mut left_screen = Screen::new("Left Projector".to_string(), 0, (1920, 1080));
    left_screen.blend_config.right = Some(EdgeBlend {
        width: 200,
        power: 2.2,
        gamma: 1.0,
        black_level: 0.02,
    });
    
    // Add a full-screen slice that samples the left portion of the composition
    let mut left_slice = Slice::full_screen(1920, 1080);
    left_slice.name = "Left Region".to_string();
    // Sample from x=0 to x=1820 (1920 - 100 for half the overlap)
    left_slice.input_rect.width = 1820.0;
    left_screen.add_slice(left_slice);
    
    // Right projector - blends on the left edge
    let mut right_screen = Screen::new("Right Projector".to_string(), 1, (1920, 1080));
    right_screen.blend_config.left = Some(EdgeBlend {
        width: 200,
        power: 2.2,
        gamma: 1.0,
        black_level: 0.02,
    });
    
    // Add a full-screen slice that samples the right portion of the composition
    let mut right_slice = Slice::full_screen(1920, 1080);
    right_slice.name = "Right Region".to_string();
    // Sample from x=1720 (offset by composition width - slice width + half overlap)
    right_slice.input_rect.x = 1720.0;
    right_slice.input_rect.width = 1820.0;
    right_screen.add_slice(right_slice);
    
    // Add screens to manager
    let left_id = output_manager.add_screen(left_screen);
    let right_id = output_manager.add_screen(right_screen);
    
    println!("Created screens:");
    println!("  - Left Projector (ID: {})", left_id);
    println!("  - Right Projector (ID: {})", right_id);
    
    // Total canvas dimensions
    let total_width = 1920 + 1920 - 200; // 3640 pixels
    println!("\nEffective canvas size: {}x1080 pixels", total_width);
    
    // Save as a preset
    let mut preset = ProjectPreset::new("Dual Projector Setup");
    preset.description = "Two 1080p projectors with 200px edge blend overlap".to_string();
    
    for screen in &output_manager.screens {
        preset.add_screen(screen);
    }
    
    // Print the preset as RON
    println!("\nPreset configuration (RON format):");
    println!("-----------------------------------");
    let ron_string = ron::ser::to_string_pretty(&preset, ron::ser::PrettyConfig::default()).unwrap();
    println!("{}", ron_string);
    
    println!("\n-----------------------------------");
    println!("To use this setup:");
    println!("1. Run the main application");
    println!("2. Load or create a HAP video with resolution >= {}x1080", total_width);
    println!("3. Connect two projectors to displays 0 and 1");
    println!("4. Load this preset file to restore the configuration");
}



