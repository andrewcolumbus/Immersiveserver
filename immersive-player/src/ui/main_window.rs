//! Main window UI components
//!
//! Transport controls, media browser, and properties panel.
//! Enhanced with Resolume-style property controls.

#![allow(dead_code)]

use super::widgets::{defaults, resettable_drag_value, property_row, spinbox_u32, FlipState, flip_buttons};
use crate::output::{DeviceType, OutputDevice, Screen, Slice, DisplayInfo, enumerate_displays};
use crate::video::{format_time, VideoPlayer};
use eframe::egui::{self, Color32, RichText, Vec2};

/// Main window UI state
#[derive(Default)]
pub struct MainWindow {
    /// Current directory for file browser
    current_dir: Option<std::path::PathBuf>,
    /// List of video files in current directory
    video_files: Vec<std::path::PathBuf>,
    /// Cached display list
    displays: Vec<DisplayInfo>,
    /// Input guide image path
    guide_path: Option<std::path::PathBuf>,
    /// Current flip state for selected slice
    flip_state: FlipState,
}

impl MainWindow {
    /// Show transport controls (play/pause/stop, timeline, etc.)
    pub fn show_transport(&mut self, ui: &mut egui::Ui, player: &mut VideoPlayer) {
        ui.horizontal(|ui| {
            // Playback controls
            let play_icon = if player.is_playing() { "⏸" } else { "▶" };
            if ui.button(play_icon).clicked() {
                player.toggle_play();
            }
            
            if ui.button("⏹").clicked() {
                player.stop();
            }

            // Skip controls
            if ui.button("⏪").clicked() {
                let t = (player.current_time() - 5.0).max(0.0);
                player.seek(t);
            }
            if ui.button("⏩").clicked() {
                let t = (player.current_time() + 5.0).min(player.duration());
                player.seek(t);
            }

            ui.separator();

            // Timeline
            let duration = player.duration();
            let mut progress = player.progress();
            
            let timeline = ui.add(
                egui::Slider::new(&mut progress, 0.0..=1.0)
                    .show_value(false)
                    .min_decimals(2)
            );
            
            if timeline.changed() {
                let t = progress * player.duration();
                player.seek(t);
            }

            // Time display
            let current_time = player.current_time();
            ui.label(format!(
                "{} / {}",
                format_time(current_time),
                format_time(duration)
            ));

            ui.separator();

            // Loop mode
            ui.label("Loop:");
            ui.horizontal(|ui| {
                if ui.selectable_label(player.loop_mode == crate::video::LoopMode::None, "Off").clicked() {
                    player.loop_mode = crate::video::LoopMode::None;
                }
                if ui.selectable_label(player.loop_mode == crate::video::LoopMode::Loop, "🔁").clicked() {
                    player.loop_mode = crate::video::LoopMode::Loop;
                }
                if ui.selectable_label(player.loop_mode == crate::video::LoopMode::PingPong, "🔀").clicked() {
                    player.loop_mode = crate::video::LoopMode::PingPong;
                }
            });

            ui.separator();

            // Speed control (right-click to reset)
            ui.label("Speed:");
            resettable_drag_value(ui, &mut player.speed, 0.1, 0.1..=4.0, defaults::PLAYBACK_SPEED);
        });
    }

    /// Show media browser
    pub fn show_media_browser(&mut self, ui: &mut egui::Ui, player: &mut VideoPlayer) {
        // Open file button
        if ui.button("📂 Open File...").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("HAP Video", &["mov", "avi", "mp4"])
                .add_filter("All Files", &["*"])
                .pick_file()
            {
                if let Err(e) = player.load(&path) {
                    log::error!("Failed to load video: {}", e);
                }
            }
        }

        ui.separator();

        // Current file info
        if player.is_loaded() {
            ui.heading("Current Video");
            
            ui.label("File: Video Loaded");
            
            if let Some((w, h)) = player.dimensions() {
                ui.label(format!("Resolution: {}×{}", w, h));
            }
            
            ui.label(format!("Frame Rate: {:.2} fps", player.frame_rate()));
            ui.label(format!("Duration: {}", format_time(player.duration())));
            
            ui.separator();
            
            let status = if player.is_playing() { "Playing" } else { "Stopped" };
            ui.label(format!("Status: {}", status));
            
            ui.label(format!("Performance: {:.1} fps", player.current_fps()));
        } else {
            ui.label("No video loaded");
            ui.label("Click 'Open File' to load a HAP video");
        }
    }

    /// Show properties panel for selected screen (Resolume-style)
    /// 
    /// The `displays` parameter should come from `OutputManager.displays` to ensure
    /// the display list stays in sync when "Detect Displays" is clicked.
    pub fn show_properties(&mut self, ui: &mut egui::Ui, screen: &mut Screen, displays: &[DisplayInfo]) {
        // Update local cache from OutputManager displays
        if !displays.is_empty() {
            self.displays = displays.to_vec();
        } else if self.displays.is_empty() {
            // Fallback: enumerate if nothing provided
            self.displays = enumerate_displays();
        }

        // Screen name header
        ui.heading(RichText::new(&screen.name).size(14.0));
        
        ui.add_space(8.0);
        
        // Device selector
        ui.horizontal(|ui| {
            ui.label(RichText::new("Device").size(11.0).color(Color32::from_gray(150)));
            
            let current_type = DeviceType::from(&screen.device);
            let mut selected = current_type;
            
            egui::ComboBox::from_id_source("device_selector")
                .selected_text(screen.device.type_name())
                .width(160.0)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(selected == DeviceType::Virtual, "Virtual Output").clicked() {
                        selected = DeviceType::Virtual;
                    }
                    
                    ui.separator();
                    ui.label(RichText::new("Fullscreen").size(10.0).color(Color32::from_gray(120)));
                    
                    for display in &self.displays {
                        let label = format!("  {}", display.name);
                        if ui.selectable_label(
                            matches!(&screen.device, OutputDevice::Fullscreen { display_id, .. } if *display_id == display.id),
                            &label
                        ).clicked() {
                            screen.device = OutputDevice::new_fullscreen(display.id, display.name.clone());
                            screen.resolution = display.resolution;
                            // TODO: Implement actual fullscreen window creation
                            // This requires winit/wgpu window management to create a fullscreen
                            // window on the selected display. Currently only stores the intent.
                            log::info!(
                                "Set screen '{}' to fullscreen on display '{}' ({}×{})",
                                screen.name,
                                display.name,
                                display.resolution.0,
                                display.resolution.1
                            );
                        }
                    }
                    
                    ui.separator();
                    
                    if ui.selectable_label(selected == DeviceType::Aqueduct, "Aqueduct").clicked() {
                        selected = DeviceType::Aqueduct;
                    }
                    
                    ui.separator();
                    
                    ui.add_enabled_ui(false, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("NDI");
                            ui.label(RichText::new("COMING SOON").size(9.0).color(Color32::from_rgb(200, 120, 0)));
                        });
                    });
                });
            
            // Update device if type changed
            if selected != current_type {
                match selected {
                    DeviceType::Virtual => {
                        screen.device = OutputDevice::new_virtual(screen.resolution.0, screen.resolution.1);
                    }
                    DeviceType::Aqueduct => {
                        screen.device = OutputDevice::new_aqueduct(screen.name.clone(), 9000);
                    }
                    _ => {}
                }
            }
        });
        
        ui.add_space(4.0);
        
        // Resolution with +/- spinboxes (Resolume-style)
        ui.horizontal(|ui| {
            ui.label(RichText::new("Width").size(11.0).color(Color32::from_gray(150)));
            let mut w = screen.resolution.0;
            spinbox_u32(ui, "", &mut w, 1, 7680);
            screen.resolution.0 = w;
        });
        
        ui.horizontal(|ui| {
            ui.label(RichText::new("Height").size(11.0).color(Color32::from_gray(150)));
            let mut h = screen.resolution.1;
            spinbox_u32(ui, "", &mut h, 1, 4320);
            screen.resolution.1 = h;
        });
        
        ui.add_space(8.0);
        
        // Opacity with percentage display and color bar
        property_row(
            ui,
            "Opacity",
            &mut screen.opacity,
            0.0..=1.0,
            0.01,
            Color32::from_gray(200),
        );
        
        // Brightness
        property_row(
            ui,
            "Brightness",
            &mut screen.brightness,
            -1.0..=1.0,
            0.01,
            Color32::from_gray(150),
        );
        
        // Contrast
        property_row(
            ui,
            "Contrast",
            &mut screen.contrast,
            0.0..=2.0,
            0.01,
            Color32::from_gray(150),
        );
        
        ui.add_space(4.0);
        
        // RGB channels with colored bars
        property_row(
            ui,
            "Red",
            &mut screen.rgb_adjust.red,
            0.0..=2.0,
            0.01,
            Color32::from_rgb(220, 80, 80),
        );
        
        property_row(
            ui,
            "Green",
            &mut screen.rgb_adjust.green,
            0.0..=2.0,
            0.01,
            Color32::from_rgb(80, 180, 80),
        );
        
        property_row(
            ui,
            "Blue",
            &mut screen.rgb_adjust.blue,
            0.0..=2.0,
            0.01,
            Color32::from_rgb(80, 120, 220),
        );
        
        ui.add_space(8.0);
        ui.separator();
        
        // Input Guide section
        ui.collapsing(RichText::new("Input Guide").size(11.0), |ui| {
            ui.horizontal(|ui| {
                if ui.add(
                    egui::Button::new("Load...")
                        .min_size(Vec2::new(50.0, 20.0))
                ).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg", "bmp"])
                        .pick_file()
                    {
                        self.guide_path = Some(path);
                    }
                }
                
                if self.guide_path.is_some() {
                    if ui.small_button("✕").clicked() {
                        self.guide_path = None;
                    }
                    ui.label(RichText::new("Image loaded").size(10.0).color(Color32::from_gray(150)));
                } else {
                    ui.label(RichText::new("Drop image file here.").size(10.0).color(Color32::from_gray(100)));
                }
            });
        });
    }

    /// Show properties panel for a selected slice (Resolume-style)
    pub fn show_slice_properties(&mut self, ui: &mut egui::Ui, slice: &mut Slice) {
        // Slice name header
        ui.heading(RichText::new(&slice.name).size(14.0));
        
        ui.add_space(8.0);
        
        // Input Source dropdown
        ui.horizontal(|ui| {
            ui.label(RichText::new("Input Source").size(11.0).color(Color32::from_gray(150)));
            
            egui::ComboBox::from_id_source("input_source_selector")
                .selected_text("Composition")
                .width(120.0)
                .show_ui(ui, |ui| {
                    let _ = ui.selectable_label(true, "Composition");
                    let _ = ui.selectable_label(false, "Layer 1");
                    let _ = ui.selectable_label(false, "Layer 2");
                    let _ = ui.selectable_label(false, "Preview");
                });
        });
        
        ui.add_space(4.0);
        
        // Position
        ui.horizontal(|ui| {
            ui.label(RichText::new("X").size(11.0).color(Color32::from_gray(150)));
            let mut x = slice.input_rect.x as u32;
            spinbox_u32(ui, "", &mut x, 0, 12000);
            slice.input_rect.x = x as f32;
        });
        
        ui.horizontal(|ui| {
            ui.label(RichText::new("Y").size(11.0).color(Color32::from_gray(150)));
            let mut y = slice.input_rect.y as u32;
            spinbox_u32(ui, "", &mut y, 0, 12000);
            slice.input_rect.y = y as f32;
        });
        
        ui.add_space(4.0);
        
        // Size
        ui.horizontal(|ui| {
            ui.label(RichText::new("Width").size(11.0).color(Color32::from_gray(150)));
            let mut w = slice.input_rect.width as u32;
            spinbox_u32(ui, "", &mut w, 1, 12000);
            slice.input_rect.width = w as f32;
        });
        
        ui.horizontal(|ui| {
            ui.label(RichText::new("Height").size(11.0).color(Color32::from_gray(150)));
            let mut h = slice.input_rect.height as u32;
            spinbox_u32(ui, "", &mut h, 1, 12000);
            slice.input_rect.height = h as f32;
        });
        
        ui.add_space(4.0);
        
        // Rotation
        ui.horizontal(|ui| {
            ui.label(RichText::new("Rotation").size(11.0).color(Color32::from_gray(150)));
            let mut rotation = 0u32;
            spinbox_u32(ui, "", &mut rotation, 0, 360);
        });
        
        ui.add_space(8.0);
        
        // Flip buttons
        ui.horizontal(|ui| {
            ui.label(RichText::new("Flip").size(11.0).color(Color32::from_gray(150)));
            flip_buttons(ui, &mut self.flip_state);
        });
        
        ui.add_space(8.0);
        
        // Checkboxes
        ui.horizontal(|ui| {
            ui.label(RichText::new("Is Key").size(11.0).color(Color32::from_gray(150)));
            let mut is_key = false;
            ui.checkbox(&mut is_key, "");
        });
        
        ui.horizontal(|ui| {
            ui.label(RichText::new("Black BG").size(11.0).color(Color32::from_gray(150)));
            let mut black_bg = false;
            ui.checkbox(&mut black_bg, "");
        });
        
        ui.add_space(8.0);
        
        // Brightness/Contrast for slice
        property_row(
            ui,
            "Brightness",
            &mut 0.0,
            -1.0..=1.0,
            0.01,
            Color32::from_gray(150),
        );
        
        property_row(
            ui,
            "Contrast",
            &mut 1.0,
            0.0..=2.0,
            0.01,
            Color32::from_gray(150),
        );
        
        ui.add_space(8.0);
        ui.separator();
        
        // Soft Edge section
        ui.collapsing(RichText::new("Soft Edge").size(11.0), |ui| {
            ui.checkbox(&mut slice.enabled, "Enable Soft Edge");
            // Soft edge controls would go here
            ui.label(RichText::new("Configure soft edge blending").size(10.0).color(Color32::from_gray(100)));
        });
    }
}

