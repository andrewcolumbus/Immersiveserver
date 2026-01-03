//! Spout texture sharing implementation for Windows.
//!
//! This module provides GPU texture sharing via Spout, allowing the output
//! texture to be received by other applications like OBS, Resolume, TouchDesigner, etc.
//!
//! Spout uses DirectX shared textures for GPU-accelerated texture sharing.

#![cfg(target_os = "windows")]

use super::spout_ffi::{is_spout_available, SpoutLibrary, DxgiFormat, GL_BGRA};
use super::texture_share::TextureSharer;

/// Spout texture sharer for Windows.
///
/// Shares the output texture via DirectX shared textures.
pub struct SpoutSharer {
    /// Spout library instance
    spout: Option<SpoutLibrary>,

    /// Whether sharing is currently active
    active: bool,

    /// Sender name
    name: String,

    /// Texture dimensions
    width: u32,
    height: u32,

    /// Pixel buffer for readback (reused to avoid allocations)
    pixel_buffer: Vec<u8>,
}

impl SpoutSharer {
    /// Create a new Spout sharer.
    pub fn new() -> Self {
        if is_spout_available() {
            log::info!("Spout: SpoutLibrary.dll available");
        } else {
            log::warn!("Spout: SpoutLibrary.dll not found - texture sharing disabled");
        }

        Self {
            spout: None,
            active: false,
            name: String::new(),
            width: 0,
            height: 0,
            pixel_buffer: Vec::new(),
        }
    }

    /// Set the texture dimensions to share.
    pub fn set_dimensions(&mut self, width: u32, height: u32) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.pixel_buffer.resize((width * height * 4) as usize, 0);
        }
    }

    /// Publish a frame from pixel data.
    pub fn publish_pixels(&mut self, pixels: &[u8]) -> Result<(), String> {
        if let Some(ref spout) = self.spout {
            if !self.active {
                return Err("Spout sender not started".to_string());
            }

            let expected_size = (self.width * self.height * 4) as usize;
            if pixels.len() < expected_size {
                return Err(format!(
                    "Pixel buffer too small: {} < {}",
                    pixels.len(),
                    expected_size
                ));
            }

            if spout.send_image(pixels, self.width, self.height, GL_BGRA, true) {
                Ok(())
            } else {
                Err("Spout SendImage failed".to_string())
            }
        } else {
            Err("Spout library not initialized".to_string())
        }
    }

    /// Get a mutable reference to the internal pixel buffer.
    pub fn pixel_buffer_mut(&mut self) -> &mut Vec<u8> {
        &mut self.pixel_buffer
    }
}

impl Default for SpoutSharer {
    fn default() -> Self {
        Self::new()
    }
}

impl TextureSharer for SpoutSharer {
    fn start(&mut self, name: &str) -> Result<(), String> {
        if self.active {
            return Ok(());
        }

        if self.spout.is_none() {
            match SpoutLibrary::new() {
                Ok(spout) => {
                    self.spout = Some(spout);
                }
                Err(e) => {
                    return Err(format!("Failed to initialize Spout: {}", e));
                }
            }
        }

        let spout = self.spout.as_ref().unwrap();

        spout.set_sender_name(name);
        spout.set_sender_format(DxgiFormat::B8G8R8A8Unorm);

        self.name = name.to_string();
        self.active = true;

        self.pixel_buffer
            .resize((self.width * self.height * 4) as usize, 0);

        log::info!(
            "Spout: Started sharing as '{}' ({}x{})",
            name,
            self.width,
            self.height
        );

        Ok(())
    }

    fn publish_frame(&mut self) -> Result<(), String> {
        if !self.active {
            return Err("Spout sender not started".to_string());
        }
        Ok(())
    }

    fn has_receivers(&self) -> bool {
        self.active
    }

    fn stop(&mut self) {
        if self.active {
            if let Some(ref spout) = self.spout {
                spout.release_sender();
            }
            log::info!("Spout: Stopped sharing");
        }
        self.active = false;
        self.name.clear();
    }

    fn technology_name(&self) -> &'static str {
        "Spout"
    }

    fn is_active(&self) -> bool {
        self.active
    }
}

impl Drop for SpoutSharer {
    fn drop(&mut self) {
        self.stop();
    }
}
