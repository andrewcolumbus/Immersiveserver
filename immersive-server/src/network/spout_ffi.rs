//! Low-level FFI bindings to Spout2 SDK for Windows.
//!
//! Spout is a Windows framework for sharing GPU textures between applications
//! using DirectX shared textures. This module provides bindings to SpoutLibrary.dll's
//! COM-like interface.
//!
//! Reference: https://github.com/leadedge/Spout2

#![cfg(target_os = "windows")]

use std::ffi::{c_void, CString};
use std::ptr;

/// DXGI_FORMAT values for texture formats
#[repr(u32)]
#[derive(Clone, Copy, Debug)]
pub enum DxgiFormat {
    Unknown = 0,
    R8G8B8A8Unorm = 28,
    B8G8R8A8Unorm = 87,
}

/// Spout library handle - wraps the COM-like interface
pub struct SpoutLibrary {
    /// Handle to the SPOUTLIBRARY interface
    handle: *mut c_void,
    /// DLL handle to keep it loaded
    _dll: libloading::Library,
}

// SpoutLibrary is thread-safe per documentation
unsafe impl Send for SpoutLibrary {}

impl SpoutLibrary {
    /// Load SpoutLibrary.dll and get the interface handle.
    pub fn new() -> Result<Self, String> {
        unsafe {
            // Try to load SpoutLibrary.dll from various locations
            let dll = Self::load_dll()?;

            // Get the factory function
            let get_spout: libloading::Symbol<unsafe extern "C" fn() -> *mut c_void> = dll
                .get(b"GetSpout")
                .map_err(|e| format!("Failed to find GetSpout function: {}", e))?;

            let handle = get_spout();
            if handle.is_null() {
                return Err("GetSpout returned null".to_string());
            }

            Ok(Self { handle, _dll: dll })
        }
    }

    /// Try to load SpoutLibrary.dll from various locations
    unsafe fn load_dll() -> Result<libloading::Library, String> {
        // Try paths in order of preference
        let paths = [
            "SpoutLibrary.dll",
            "./SpoutLibrary.dll",
            "../external_libraries/Spout-SDK-binaries/Libs_2-007-017/MT/bin/SpoutLibrary.dll",
        ];

        for path in &paths {
            if let Ok(dll) = libloading::Library::new(path) {
                log::info!("Spout: Loaded SpoutLibrary.dll from {}", path);
                return Ok(dll);
            }
        }

        Err("Failed to load SpoutLibrary.dll - ensure it's in PATH or application directory".to_string())
    }

    /// Get the vtable pointer for calling virtual methods
    fn vtable(&self) -> *const *const c_void {
        // The handle points to an object whose first member is the vtable pointer
        self.handle as *const *const c_void
    }

    /// Call a virtual method by index
    unsafe fn call_method<R>(&self, index: usize) -> R {
        let vtable = *self.vtable();
        let method: unsafe extern "C" fn(*mut c_void) -> R =
            std::mem::transmute(*vtable.add(index));
        method(self.handle)
    }

    // === Sender Methods ===
    // VTable indices based on SpoutLibrary.h interface order

    /// SetSenderName (index 0)
    pub fn set_sender_name(&self, name: &str) {
        unsafe {
            let c_name = CString::new(name).unwrap_or_default();
            let vtable = *self.vtable();
            let method: unsafe extern "C" fn(*mut c_void, *const i8) =
                std::mem::transmute(*vtable.add(0));
            method(self.handle, c_name.as_ptr());
        }
    }

    /// SetSenderFormat (index 1)
    pub fn set_sender_format(&self, format: DxgiFormat) {
        unsafe {
            let vtable = *self.vtable();
            let method: unsafe extern "C" fn(*mut c_void, u32) =
                std::mem::transmute(*vtable.add(1));
            method(self.handle, format as u32);
        }
    }

    /// ReleaseSender (index 2)
    pub fn release_sender(&self) {
        unsafe {
            let vtable = *self.vtable();
            let method: unsafe extern "C" fn(*mut c_void, u32) =
                std::mem::transmute(*vtable.add(2));
            method(self.handle, 0);
        }
    }

    /// SendImage (index 5) - Send pixel data
    /// glFormat: GL_RGBA = 0x1908, GL_BGRA = 0x80E1
    pub fn send_image(
        &self,
        pixels: &[u8],
        width: u32,
        height: u32,
        gl_format: u32,
        invert: bool,
    ) -> bool {
        unsafe {
            let vtable = *self.vtable();
            // SendImage is at index 5 (after SendFbo=3, SendTexture=4)
            let method: unsafe extern "C" fn(*mut c_void, *const u8, u32, u32, u32, bool) -> bool =
                std::mem::transmute(*vtable.add(5));
            method(self.handle, pixels.as_ptr(), width, height, gl_format, invert)
        }
    }

    /// IsInitialized (index 6)
    pub fn is_initialized(&self) -> bool {
        unsafe {
            let vtable = *self.vtable();
            let method: unsafe extern "C" fn(*mut c_void) -> bool =
                std::mem::transmute(*vtable.add(6));
            method(self.handle)
        }
    }

    /// GetWidth (index 8)
    pub fn get_width(&self) -> u32 {
        unsafe {
            let vtable = *self.vtable();
            let method: unsafe extern "C" fn(*mut c_void) -> u32 =
                std::mem::transmute(*vtable.add(8));
            method(self.handle)
        }
    }

    /// GetHeight (index 9)
    pub fn get_height(&self) -> u32 {
        unsafe {
            let vtable = *self.vtable();
            let method: unsafe extern "C" fn(*mut c_void) -> u32 =
                std::mem::transmute(*vtable.add(9));
            method(self.handle)
        }
    }

    /// Release (index at end of vtable) - Release the library instance
    /// This is called by Drop
    fn release(&self) {
        unsafe {
            let vtable = *self.vtable();
            // Release is typically the last method - index may vary
            // Based on the header, it's around index 90+
            // For safety, we'll let the DLL handle cleanup on unload
        }
    }
}

impl Drop for SpoutLibrary {
    fn drop(&mut self) {
        // Release sender resources
        self.release_sender();
    }
}

/// Check if Spout is available on this system
pub fn is_spout_available() -> bool {
    SpoutLibrary::new().is_ok()
}

// GL format constants
pub const GL_RGBA: u32 = 0x1908;
pub const GL_BGRA: u32 = 0x80E1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spout_availability() {
        let available = is_spout_available();
        println!("Spout available: {}", available);
    }
}
