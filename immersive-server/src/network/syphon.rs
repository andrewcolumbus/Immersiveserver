//! Syphon texture sharing implementation for macOS.
//!
//! This module provides GPU texture sharing via Syphon, allowing the environment
//! texture to be received by other applications like OBS, VDMX, Resolume, etc.
//!
//! Syphon uses Metal for GPU texture sharing on modern macOS.

#![cfg(target_os = "macos")]

use super::syphon_ffi::{is_syphon_available, SyphonMetalServerWrapper};
use super::texture_share::TextureSharer;
use metal::foreign_types::ForeignType;
use std::ffi::c_void;

/// A Send-safe wrapper for raw pointers.
///
/// Metal device and queue pointers are safe to use from any thread.
#[derive(Clone, Copy)]
struct SendPtr(*mut c_void);

// Metal devices and command queues are thread-safe
unsafe impl Send for SendPtr {}

impl SendPtr {
    fn new(ptr: *mut c_void) -> Self {
        Self(ptr)
    }

    fn get(&self) -> *mut c_void {
        self.0
    }
}

/// Syphon texture sharer for macOS.
///
/// Shares the environment texture directly on the GPU via Syphon's Metal support.
/// This provides zero-copy, zero-latency texture sharing with other applications.
pub struct SyphonSharer {
    /// The underlying Syphon Metal server (None until started)
    server: Option<SyphonMetalServerWrapper>,

    /// Server name
    name: String,

    /// Texture dimensions
    width: u32,
    height: u32,

    /// Metal device pointer (stored for creating command buffers)
    metal_device: Option<SendPtr>,

    /// Metal command queue pointer
    metal_queue: Option<SendPtr>,
}

impl SyphonSharer {
    /// Create a new Syphon sharer.
    ///
    /// # Returns
    /// A new SyphonSharer instance.
    pub fn new() -> Self {
        if is_syphon_available() {
            tracing::info!("Syphon: Framework available");
        } else {
            tracing::warn!("Syphon: Framework not available - texture sharing disabled");
        }

        Self {
            server: None,
            name: String::new(),
            width: 0,
            height: 0,
            metal_device: None,
            metal_queue: None,
        }
    }

    /// Set the texture dimensions to share.
    pub fn set_dimensions(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    /// Set the Metal device and queue pointers.
    ///
    /// This must be called before start() to enable actual Syphon functionality.
    /// The pointers should be extracted from the wgpu device.
    ///
    /// # Safety
    /// The device and queue pointers must be valid Metal objects that outlive this sharer.
    pub unsafe fn set_metal_handles(&mut self, device: *mut c_void, queue: *mut c_void) {
        self.metal_device = Some(SendPtr::new(device));
        self.metal_queue = Some(SendPtr::new(queue));
    }

    /// Publish a frame using the given Metal texture and command buffer.
    ///
    /// # Safety
    /// The texture and command_buffer must be valid Metal objects.
    pub unsafe fn publish_frame_metal(
        &self,
        texture: *mut c_void,
        command_buffer: *mut c_void,
    ) -> Result<(), String> {
        if let Some(ref server) = self.server {
            server.publish_frame(texture, command_buffer, self.width, self.height, false);
            Ok(())
        } else {
            Err("Syphon server not started".to_string())
        }
    }

    /// Publish a frame from a wgpu texture.
    ///
    /// This extracts the Metal texture from wgpu and publishes it via Syphon.
    /// The command_queue is used to create a command buffer for Syphon.
    ///
    /// # Safety
    /// The command_queue must be a valid Metal command queue.
    pub unsafe fn publish_wgpu_texture(
        &self,
        _device: &wgpu::Device,
        texture: &wgpu::Texture,
        command_queue: &metal::CommandQueue,
    ) -> Result<(), String> {
        if self.server.is_none() {
            return Err("Syphon server not started".to_string());
        }

        // Extract the Metal texture from wgpu
        let metal_texture: Option<metal::Texture> =
            texture.as_hal::<wgpu_hal::api::Metal, _, _>(|hal_texture| {
                hal_texture.map(|tex| {
                    // Clone the texture reference so we can use it outside the closure
                    tex.raw_handle().to_owned()
                })
            });

        let metal_texture = match metal_texture {
            Some(tex) => tex,
            None => {
                return Err("Failed to extract Metal texture from wgpu".to_string());
            }
        };

        // Create a texture view with linear (non-sRGB) pixel format
        // The wgpu texture is BGRA8Unorm_sRGB, but Syphon receivers expect linear
        // MTLPixelFormatBGRA8Unorm = 80
        let linear_view = metal_texture
            .new_texture_view(metal::MTLPixelFormat::BGRA8Unorm);

        // Create a command buffer for Syphon
        let command_buffer = command_queue
            .new_command_buffer()
            .to_owned();

        // Publish the frame (flipped=true to correct Metal's coordinate origin)
        if let Some(ref server) = self.server {
            server.publish_frame(
                linear_view.as_ptr() as *mut c_void,
                command_buffer.as_ptr() as *mut c_void,
                self.width,
                self.height,
                true,
            );
        }

        // Commit the command buffer
        command_buffer.commit();

        Ok(())
    }
}

impl Default for SyphonSharer {
    fn default() -> Self {
        Self::new()
    }
}

impl TextureSharer for SyphonSharer {
    fn start(&mut self, name: &str) -> Result<(), String> {
        if self.server.is_some() {
            return Ok(());
        }

        // Check if Syphon is available
        if !is_syphon_available() {
            return Err(
                "Syphon.framework not linked. Build the Metal-enabled Syphon from source."
                    .to_string(),
            );
        }

        // Check if Metal handles are set
        let device = match self.metal_device {
            Some(d) => d.get(),
            None => {
                return Err(
                    "Metal device not set. Call set_metal_handles() before start().".to_string(),
                )
            }
        };

        // Create the Syphon server
        let server = unsafe { SyphonMetalServerWrapper::new(name, device)? };

        self.name = name.to_string();
        self.server = Some(server);

        tracing::info!(
            "Syphon: Started sharing as '{}' ({}x{})",
            name,
            self.width,
            self.height
        );

        Ok(())
    }

    fn publish_frame(&mut self) -> Result<(), String> {
        // This method is called without Metal handles, so we can't publish
        // The actual publishing should be done via publish_frame_metal()
        // or by integrating directly in the render loop
        if self.server.is_none() {
            return Err("Syphon server not started".to_string());
        }

        // In the current architecture, the render loop should call publish_frame_metal()
        // with the actual Metal texture and command buffer
        Ok(())
    }

    fn has_receivers(&self) -> bool {
        if let Some(ref server) = self.server {
            server.has_clients()
        } else {
            false
        }
    }

    fn stop(&mut self) {
        if let Some(server) = self.server.take() {
            server.stop();
            tracing::info!("Syphon: Stopped sharing");
        }
        self.name.clear();
    }

    fn technology_name(&self) -> &'static str {
        "Syphon"
    }

    fn is_active(&self) -> bool {
        self.server.is_some()
    }
}

impl Drop for SyphonSharer {
    fn drop(&mut self) {
        self.stop();
    }
}
