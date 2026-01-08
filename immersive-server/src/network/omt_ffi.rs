//! FFI bindings to the official libOMT C library.
//!
//! This module provides low-level bindings to the Open Media Transport (OMT)
//! library from IntoPix. The library provides efficient video streaming with
//! the VMX1 codec for compatibility with OBS and other OMT-enabled applications.

use std::ffi::{c_char, c_float, c_int, c_void, CStr, CString};
use std::os::raw::c_longlong;
use std::ptr;

/// Maximum string length for OMT strings.
pub const OMT_MAX_STRING_LENGTH: usize = 1024;

/// Handle type for OMT sender instances.
pub type OmtSendHandle = c_longlong;

/// Handle type for OMT receiver instances.
pub type OmtReceiveHandle = c_longlong;

/// Frame type enumeration.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OMTFrameType {
    None = 0,
    Metadata = 1,
    Video = 2,
    Audio = 4,
}

/// FourCC codec constants.
pub mod codec {
    use std::ffi::c_int;

    /// VMX1 - Fast video codec (proprietary, OBS-compatible).
    pub const VMX1: c_int = 0x31584D56;
    /// BGRA - 32bpp RGBA format.
    pub const BGRA: c_int = 0x41524742;
    /// UYVY - 16bpp YUV format.
    pub const UYVY: c_int = 0x59565955;
    /// YUY2 - 16bpp YUV format YUYV pixel order.
    pub const YUY2: c_int = 0x32595559;
    /// NV12 - Planar 4:2:0 YUV format.
    pub const NV12: c_int = 0x3231564E;
    /// YV12 - Planar 4:2:0 YUV format.
    pub const YV12: c_int = 0x32315659;
}

/// Video encoding quality.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OMTQuality {
    /// Allow receivers to suggest quality.
    Default = 0,
    /// Low quality (faster encoding).
    Low = 1,
    /// Medium quality.
    Medium = 50,
    /// High quality (slower encoding).
    High = 100,
}

/// Video frame flags.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OMTVideoFlags {
    None = 0,
    Interlaced = 1,
    Alpha = 2,
    PreMultiplied = 4,
    Preview = 8,
    HighBitDepth = 16,
}

/// Color space for YUV conversions.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OMTColorSpace {
    /// Auto-detect based on resolution.
    Undefined = 0,
    /// SD color space.
    BT601 = 601,
    /// HD color space.
    BT709 = 709,
}

/// Preferred uncompressed video format for receiver.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OMTPreferredVideoFormat {
    /// UYVY only (fastest).
    UYVY = 0,
    /// BGRA only when alpha present, UYVY otherwise.
    UYVYorBGRA = 1,
    /// Always convert to BGRA.
    BGRA = 2,
    /// UYVA only when alpha present, UYVY otherwise.
    UYVYorUYVA = 3,
    /// High bit depth with alpha support.
    UYVYorUYVAorP216orPA16 = 4,
    /// P216 format.
    P216 = 5,
}

/// Receiver flags.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OMTReceiveFlags {
    /// No special flags.
    None = 0,
    /// Receive only 1/8th preview.
    Preview = 1,
    /// Include compressed VMX1 data.
    IncludeCompressed = 2,
    /// Only compressed data, no decode.
    CompressedOnly = 4,
}

/// Media frame structure for sending/receiving.
///
/// IMPORTANT: Zero this struct before use.
#[repr(C)]
#[derive(Debug)]
pub struct OMTMediaFrame {
    /// Frame type (Video, Audio, Metadata).
    pub frame_type: c_int,
    /// Timestamp where 1 second = 10,000,000. Use -1 for auto-timestamp.
    pub timestamp: c_longlong,
    /// FourCC codec identifier.
    pub codec: c_int,
    /// Video width in pixels.
    pub width: c_int,
    /// Video height in pixels.
    pub height: c_int,
    /// Stride in bytes per row.
    pub stride: c_int,
    /// Video flags (OMTVideoFlags).
    pub flags: c_int,
    /// Frame rate numerator.
    pub frame_rate_n: c_int,
    /// Frame rate denominator.
    pub frame_rate_d: c_int,
    /// Display aspect ratio (width/height).
    pub aspect_ratio: c_float,
    /// Color space (OMTColorSpace).
    pub color_space: c_int,
    /// Audio sample rate (e.g., 48000).
    pub sample_rate: c_int,
    /// Audio channels (max 32).
    pub channels: c_int,
    /// Samples per audio channel.
    pub samples_per_channel: c_int,
    /// Pointer to frame data.
    pub data: *mut c_void,
    /// Length of data in bytes.
    pub data_length: c_int,
    /// Compressed data (receive only).
    pub compressed_data: *mut c_void,
    /// Compressed data length.
    pub compressed_length: c_int,
    /// Per-frame metadata (UTF-8 XML).
    pub frame_metadata: *mut c_void,
    /// Metadata length including null terminator.
    pub frame_metadata_length: c_int,
}

impl Default for OMTMediaFrame {
    fn default() -> Self {
        // Zero-initialize as required by libOMT
        Self {
            frame_type: OMTFrameType::None as c_int,
            timestamp: -1, // Auto-timestamp
            codec: 0,
            width: 0,
            height: 0,
            stride: 0,
            flags: OMTVideoFlags::None as c_int,
            frame_rate_n: 60,
            frame_rate_d: 1,
            aspect_ratio: 0.0,
            color_space: OMTColorSpace::Undefined as c_int,
            sample_rate: 0,
            channels: 0,
            samples_per_channel: 0,
            data: ptr::null_mut(),
            data_length: 0,
            compressed_data: ptr::null_mut(),
            compressed_length: 0,
            frame_metadata: ptr::null_mut(),
            frame_metadata_length: 0,
        }
    }
}

// Link against libomt
#[link(name = "omt")]
extern "C" {
    // =========================================
    // Discovery
    // =========================================

    /// Returns a list of sources currently available on the network.
    /// The returned array is valid until the next call to this function.
    pub fn omt_discovery_getaddresses(count: *mut c_int) -> *mut *mut c_char;

    // =========================================
    // Sender
    // =========================================

    /// Create a new OMT sender instance.
    pub fn omt_send_create(name: *const c_char, quality: c_int) -> *mut OmtSendHandle;

    /// Destroy a sender instance.
    pub fn omt_send_destroy(instance: *mut OmtSendHandle);

    /// Send a frame to connected receivers.
    pub fn omt_send(instance: *mut OmtSendHandle, frame: *mut OMTMediaFrame) -> c_int;

    /// Get the discovery address in format "HOSTNAME (NAME)".
    pub fn omt_send_getaddress(
        instance: *mut OmtSendHandle,
        address: *mut c_char,
        max_length: c_int,
    ) -> c_int;

    /// Get the number of active connections.
    pub fn omt_send_connections(instance: *mut OmtSendHandle) -> c_int;

    // =========================================
    // Receiver
    // =========================================

    /// Create a new OMT receiver and begin connecting to the sender.
    pub fn omt_receive_create(
        address: *const c_char,
        frame_types: c_int,
        format: c_int,
        flags: c_int,
    ) -> *mut OmtReceiveHandle;

    /// Destroy a receiver instance.
    pub fn omt_receive_destroy(instance: *mut OmtReceiveHandle);

    /// Receive a frame with timeout.
    /// Returns null if timed out, otherwise a valid frame pointer.
    /// The frame data is valid until the next call to omt_receive.
    pub fn omt_receive(
        instance: *mut OmtReceiveHandle,
        frame_types: c_int,
        timeout_ms: c_int,
    ) -> *mut OMTMediaFrame;

    // =========================================
    // Logging
    // =========================================

    /// Set the log file path, or null to use default.
    pub fn omt_setloggingfilename(filename: *const c_char);
}

// =========================================
// Safe Rust Wrappers
// =========================================

/// Safe wrapper around libOMT sender.
pub struct LibOmtSender {
    handle: *mut OmtSendHandle,
    name: String,
}

// LibOmtSender is Send because the C library is thread-safe for sending
unsafe impl Send for LibOmtSender {}

impl LibOmtSender {
    /// Create a new OMT sender with the given name.
    pub fn new(name: &str) -> Result<Self, String> {
        let c_name = CString::new(name).map_err(|e| format!("Invalid name: {}", e))?;

        let handle = unsafe { omt_send_create(c_name.as_ptr(), OMTQuality::Default as c_int) };

        if handle.is_null() {
            return Err("Failed to create OMT sender - null handle returned".to_string());
        }

        tracing::info!("libOMT: Created sender '{}'", name);

        Ok(Self {
            handle,
            name: name.to_string(),
        })
    }

    /// Send a BGRA video frame.
    pub fn send_frame(
        &mut self,
        width: u32,
        height: u32,
        frame_rate: u32,
        bgra_data: &[u8],
    ) -> Result<(), String> {
        let expected_len = (width * height * 4) as usize;
        if bgra_data.len() != expected_len {
            return Err(format!(
                "Data length mismatch: expected {}, got {}",
                expected_len,
                bgra_data.len()
            ));
        }

        let mut frame = OMTMediaFrame {
            frame_type: OMTFrameType::Video as c_int,
            timestamp: -1, // Auto-timestamp
            codec: codec::BGRA,
            width: width as c_int,
            height: height as c_int,
            stride: (width * 4) as c_int,
            flags: OMTVideoFlags::None as c_int,
            frame_rate_n: frame_rate as c_int,
            frame_rate_d: 1,
            aspect_ratio: width as f32 / height as f32,
            color_space: OMTColorSpace::BT709 as c_int,
            data: bgra_data.as_ptr() as *mut c_void,
            data_length: bgra_data.len() as c_int,
            ..Default::default()
        };

        let result = unsafe { omt_send(self.handle, &mut frame) };

        // libOMT returns encoding time in microseconds on success, negative on error.
        // Positive values (even large ones like 640000 = 640ms) indicate success.
        if result < 0 {
            Err(format!("omt_send failed with code {}", result))
        } else {
            Ok(())
        }
    }

    /// Get the discovery address string.
    pub fn get_address(&self) -> Option<String> {
        let mut buffer = [0u8; OMT_MAX_STRING_LENGTH];
        let len = unsafe {
            omt_send_getaddress(
                self.handle,
                buffer.as_mut_ptr() as *mut c_char,
                OMT_MAX_STRING_LENGTH as c_int,
            )
        };

        if len > 0 {
            let cstr = unsafe { CStr::from_ptr(buffer.as_ptr() as *const c_char) };
            cstr.to_str().ok().map(|s| s.to_string())
        } else {
            None
        }
    }

    /// Get the number of connected receivers.
    pub fn connection_count(&self) -> i32 {
        unsafe { omt_send_connections(self.handle) }
    }

    /// Get the sender name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Drop for LibOmtSender {
    fn drop(&mut self) {
        tracing::info!("libOMT: Destroying sender '{}'", self.name);
        unsafe {
            omt_send_destroy(self.handle);
        }
    }
}

/// Safe wrapper around libOMT receiver.
pub struct LibOmtReceiver {
    handle: *mut OmtReceiveHandle,
    address: String,
}

// LibOmtReceiver is Send because the C library is thread-safe
unsafe impl Send for LibOmtReceiver {}

impl LibOmtReceiver {
    /// Create a new OMT receiver connecting to the given address.
    ///
    /// Address can be either:
    /// - Discovery format: "HOSTNAME (NAME)"
    /// - Direct URL: "omt://hostname:port"
    pub fn new(address: &str) -> Result<Self, String> {
        let c_address = CString::new(address).map_err(|e| format!("Invalid address: {}", e))?;

        let handle = unsafe {
            omt_receive_create(
                c_address.as_ptr(),
                OMTFrameType::Video as c_int, // Video frames only
                OMTPreferredVideoFormat::BGRA as c_int,
                OMTReceiveFlags::None as c_int,
            )
        };

        if handle.is_null() {
            return Err("Failed to create OMT receiver - null handle returned".to_string());
        }

        tracing::info!("libOMT: Created receiver for '{}'", address);

        Ok(Self {
            handle,
            address: address.to_string(),
        })
    }

    /// Receive a video frame with timeout.
    ///
    /// Returns None if timed out or no frame available.
    /// The returned frame data must be copied before the next call.
    pub fn receive_frame(&self, timeout_ms: i32) -> Option<ReceivedFrame> {
        let frame_ptr = unsafe {
            omt_receive(
                self.handle,
                OMTFrameType::Video as c_int,
                timeout_ms as c_int,
            )
        };

        if frame_ptr.is_null() {
            return None;
        }

        // SAFETY: libOMT guarantees the frame pointer is valid until next omt_receive call
        let frame = unsafe { &*frame_ptr };

        // Only process video frames
        if frame.frame_type != OMTFrameType::Video as c_int {
            return None;
        }

        // Check for valid dimensions
        if frame.width <= 0 || frame.height <= 0 {
            return None;
        }

        // Check for valid data
        if frame.data.is_null() || frame.data_length <= 0 {
            return None;
        }

        // Determine if this is BGRA format
        let is_bgra = frame.codec == codec::BGRA;

        Some(ReceivedFrame {
            width: frame.width as u32,
            height: frame.height as u32,
            stride: frame.stride as u32,
            data_ptr: frame.data as *const u8,
            data_len: frame.data_length as usize,
            is_bgra,
            frame_rate_n: frame.frame_rate_n,
            frame_rate_d: frame.frame_rate_d,
        })
    }

    /// Get the address this receiver is connected to.
    pub fn address(&self) -> &str {
        &self.address
    }
}

impl Drop for LibOmtReceiver {
    fn drop(&mut self) {
        tracing::info!("libOMT: Destroying receiver for '{}'", self.address);
        unsafe {
            omt_receive_destroy(self.handle);
        }
    }
}

/// Temporary reference to a received frame.
///
/// This data is only valid until the next call to `receive_frame()`.
/// Copy the data if you need to keep it.
pub struct ReceivedFrame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub data_ptr: *const u8,
    pub data_len: usize,
    pub is_bgra: bool,
    pub frame_rate_n: c_int,
    pub frame_rate_d: c_int,
}

impl ReceivedFrame {
    /// Copy the frame data to a new Vec.
    ///
    /// SAFETY: The caller must ensure this is called before the next omt_receive() call.
    pub unsafe fn copy_data(&self) -> Vec<u8> {
        if self.data_ptr.is_null() || self.data_len == 0 {
            return Vec::new();
        }
        std::slice::from_raw_parts(self.data_ptr, self.data_len).to_vec()
    }
}

/// Get list of discovered OMT sources on the network.
pub fn get_discovered_sources() -> Vec<String> {
    let mut count: c_int = 0;
    let addresses = unsafe { omt_discovery_getaddresses(&mut count) };

    if addresses.is_null() || count <= 0 {
        return Vec::new();
    }

    let mut sources = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        let addr_ptr = unsafe { *addresses.add(i) };
        if !addr_ptr.is_null() {
            if let Ok(s) = unsafe { CStr::from_ptr(addr_ptr) }.to_str() {
                sources.push(s.to_string());
            }
        }
    }

    sources
}

/// Set the OMT log file path.
pub fn set_log_file(path: Option<&str>) {
    match path {
        Some(p) => {
            if let Ok(c_path) = CString::new(p) {
                unsafe { omt_setloggingfilename(c_path.as_ptr()) };
            }
        }
        None => unsafe { omt_setloggingfilename(ptr::null()) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_default() {
        let frame = OMTMediaFrame::default();
        assert_eq!(frame.frame_type, OMTFrameType::None as c_int);
        assert_eq!(frame.timestamp, -1);
        assert!(frame.data.is_null());
    }

    #[test]
    fn test_codec_constants() {
        // Verify FourCC values match expected
        assert_eq!(codec::BGRA, 0x41524742);
        assert_eq!(codec::VMX1, 0x31584D56);
    }
}
