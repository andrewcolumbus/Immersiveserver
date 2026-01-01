//! Low-level FFI bindings to Syphon.framework for macOS.
//!
//! Syphon is a macOS framework for sharing GPU textures between applications.
//! This module provides Objective-C bindings to `SyphonMetalServer` which allows
//! publishing Metal textures that can be received by other Syphon-compatible apps.
//!
//! Reference: https://github.com/Syphon/Syphon-Framework

#![cfg(target_os = "macos")]

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool};
use objc2::msg_send;
use objc2::Encode;
use objc2_foundation::NSString;
use std::ffi::{c_void, CStr};

// CGPoint, CGSize, CGRect types for NSRect construction
// These are core graphics types used by Syphon's publishFrameTexture API
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct CGSize {
    pub width: f64,
    pub height: f64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

impl CGRect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            origin: CGPoint { x, y },
            size: CGSize { width, height },
        }
    }
}

// Encode implementations for objc2 messaging
unsafe impl objc2::Encode for CGPoint {
    const ENCODING: objc2::Encoding = objc2::Encoding::Struct("CGPoint", &[objc2::Encoding::Double, objc2::Encoding::Double]);
}

unsafe impl objc2::Encode for CGSize {
    const ENCODING: objc2::Encoding = objc2::Encoding::Struct("CGSize", &[objc2::Encoding::Double, objc2::Encoding::Double]);
}

unsafe impl objc2::Encode for CGRect {
    const ENCODING: objc2::Encoding = objc2::Encoding::Struct("CGRect", &[CGPoint::ENCODING, CGSize::ENCODING]);
}

unsafe impl objc2::RefEncode for CGPoint {
    const ENCODING_REF: objc2::Encoding = objc2::Encoding::Pointer(&Self::ENCODING);
}

unsafe impl objc2::RefEncode for CGSize {
    const ENCODING_REF: objc2::Encoding = objc2::Encoding::Pointer(&Self::ENCODING);
}

unsafe impl objc2::RefEncode for CGRect {
    const ENCODING_REF: objc2::Encoding = objc2::Encoding::Pointer(&Self::ENCODING);
}

/// Wrapper around SyphonMetalServer Objective-C class.
///
/// This provides a safe Rust interface to the Syphon Metal server functionality.
pub struct SyphonMetalServerWrapper {
    /// The underlying Objective-C SyphonMetalServer object
    server: Retained<AnyObject>,
}

// SyphonMetalServer is documented as thread-safe
unsafe impl Send for SyphonMetalServerWrapper {}

impl SyphonMetalServerWrapper {
    /// Create a new Syphon Metal server.
    ///
    /// # Arguments
    /// * `name` - The human-readable name for this server (visible to clients)
    /// * `device` - A raw pointer to the MTLDevice
    ///
    /// # Safety
    /// The `device` pointer must be a valid MTLDevice pointer.
    pub unsafe fn new(name: &str, device: *mut c_void) -> Result<Self, String> {
        // Get the SyphonMetalServer class
        let class = match get_syphon_metal_server_class() {
            Some(c) => c,
            None => {
                return Err(
                    "SyphonMetalServer class not found. Is Syphon.framework linked?".to_string(),
                )
            }
        };

        // Create NSString for the name
        let ns_name = NSString::from_str(name);

        // Call [[SyphonMetalServer alloc] initWithName:device:options:]
        let server: *mut AnyObject = msg_send![class, alloc];
        if server.is_null() {
            return Err("Failed to allocate SyphonMetalServer".to_string());
        }

        // Initialize with name, device, and nil options
        // Cast the device pointer to an Objective-C object reference
        // The device is an id<MTLDevice> which is an Objective-C object
        let device_obj: &AnyObject = &*(device as *const AnyObject);
        let server: *mut AnyObject = msg_send![
            server,
            initWithName: &*ns_name,
            device: device_obj,
            options: std::ptr::null::<AnyObject>()
        ];

        if server.is_null() {
            return Err("Failed to initialize SyphonMetalServer".to_string());
        }

        // Wrap in Retained for memory management
        let server = Retained::from_raw(server)
            .ok_or_else(|| "Failed to create Retained wrapper".to_string())?;

        Ok(Self { server })
    }

    /// Publish a frame texture to connected clients.
    ///
    /// # Arguments
    /// * `texture` - Raw pointer to the MTLTexture to publish
    /// * `command_buffer` - Raw pointer to the MTLCommandBuffer
    /// * `region` - The region of the texture to publish
    /// * `flipped` - Whether the texture is vertically flipped
    ///
    /// # Safety
    /// The texture and command_buffer pointers must be valid Metal objects.
    pub unsafe fn publish_frame(
        &self,
        texture: *mut c_void,
        command_buffer: *mut c_void,
        width: u32,
        height: u32,
        flipped: bool,
    ) {
        // Create CGRect for the image region (full texture)
        let region = CGRect::new(0.0, 0.0, width as f64, height as f64);

        // Cast pointers to Objective-C object references
        // MTLTexture and MTLCommandBuffer are Objective-C objects
        let texture_obj: &AnyObject = &*(texture as *const AnyObject);
        let cmd_buffer_obj: &AnyObject = &*(command_buffer as *const AnyObject);

        // Call publishFrameTexture:onCommandBuffer:imageRegion:flipped:
        let _: () = msg_send![
            &*self.server,
            publishFrameTexture: texture_obj,
            onCommandBuffer: cmd_buffer_obj,
            imageRegion: region,
            flipped: Bool::new(flipped)
        ];
    }

    /// Check if any clients are connected to this server.
    pub fn has_clients(&self) -> bool {
        unsafe {
            let result: Bool = msg_send![&*self.server, hasClients];
            result.as_bool()
        }
    }

    /// Stop the server and release resources.
    pub fn stop(&self) {
        unsafe {
            let _: () = msg_send![&*self.server, stop];
        }
    }

    /// Get the server name.
    pub fn name(&self) -> Option<String> {
        unsafe {
            let name: *mut NSString = msg_send![&*self.server, name];
            if name.is_null() {
                None
            } else {
                Some((*name).to_string())
            }
        }
    }
}

impl Drop for SyphonMetalServerWrapper {
    fn drop(&mut self) {
        // Stop the server when dropped
        self.stop();
    }
}

/// Get the SyphonMetalServer class, if available.
fn get_syphon_metal_server_class() -> Option<&'static AnyClass> {
    // Try to get the class - this will fail if Syphon.framework is not linked
    // SAFETY: The string is null-terminated and valid UTF-8
    let name = unsafe { CStr::from_bytes_with_nul_unchecked(b"SyphonMetalServer\0") };
    AnyClass::get(name)
}

/// Check if Syphon.framework is available.
pub fn is_syphon_available() -> bool {
    get_syphon_metal_server_class().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syphon_availability() {
        // This test checks if we can detect Syphon availability
        // The actual result depends on whether the framework is linked
        let available = is_syphon_available();
        println!("Syphon available: {}", available);
    }
}
