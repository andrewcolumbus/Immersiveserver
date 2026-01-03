//! FFI bindings to the NDI SDK (libndi).
//!
//! This module provides low-level bindings to the NDI (Network Device Interface)
//! SDK from Vizrt. NDI enables high-quality, low-latency video streaming over
//! standard networks.
//!
//! SDK Location: /Library/NDI SDK for Apple/
//! Library: /Library/NDI SDK for Apple/lib/macOS/libndi.dylib

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

use std::ffi::{c_char, c_int, c_void, CStr};
use std::ptr;

// =============================================================================
// Opaque Handle Types
// =============================================================================

/// Opaque handle to an NDI finder instance.
pub type NDIlib_find_instance_t = *mut c_void;

/// Opaque handle to an NDI receiver instance.
pub type NDIlib_recv_instance_t = *mut c_void;

/// Opaque handle to an NDI sender instance.
pub type NDIlib_send_instance_t = *mut c_void;

// =============================================================================
// Constants
// =============================================================================

/// Timecode value that tells NDI to synthesize the timecode.
pub const NDILIB_SEND_TIMECODE_SYNTHESIZE: i64 = i64::MAX;

/// Timestamp value indicating the timestamp is undefined.
pub const NDILIB_RECV_TIMESTAMP_UNDEFINED: i64 = i64::MAX;

// =============================================================================
// Enumerations
// =============================================================================

/// Frame type returned by capture functions.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NDIlib_frame_type_e {
    /// No frame available.
    None = 0,
    /// Video frame.
    Video = 1,
    /// Audio frame.
    Audio = 2,
    /// Metadata frame.
    Metadata = 3,
    /// Error occurred (connection lost).
    Error = 4,
    /// Settings on the input have changed.
    StatusChange = 100,
    /// Source has changed.
    SourceChange = 101,
}

/// FourCC video format codes.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NDIlib_FourCC_video_type_e {
    /// YCbCr 4:2:2 (UYVY ordering).
    UYVY = 0x59565955, // 'UYVY'
    /// YCbCr + Alpha 4:2:2:4.
    UYVA = 0x41565955, // 'UYVA'
    /// YCbCr 4:2:2 in 16bpp (P216).
    P216 = 0x36313250, // 'P216'
    /// YCbCr + Alpha 4:2:2:4 in 16bpp (PA16).
    PA16 = 0x36314150, // 'PA16'
    /// Planar YV12.
    YV12 = 0x32315659, // 'YV12'
    /// Planar I420.
    I420 = 0x30323449, // 'I420'
    /// Planar NV12.
    NV12 = 0x3231564E, // 'NV12'
    /// BGRA 8-bit.
    BGRA = 0x41524742, // 'BGRA'
    /// BGRX 8-bit (alpha = 255).
    BGRX = 0x58524742, // 'BGRX'
    /// RGBA 8-bit.
    RGBA = 0x41424752, // 'RGBA'
    /// RGBX 8-bit (alpha = 255).
    RGBX = 0x58424752, // 'RGBX'
}

/// Receiver bandwidth modes.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NDIlib_recv_bandwidth_e {
    /// Receive metadata only.
    MetadataOnly = -10,
    /// Receive metadata and audio only.
    AudioOnly = 10,
    /// Receive at lower bandwidth and resolution.
    Lowest = 0,
    /// Receive at full resolution.
    Highest = 100,
}

/// Receiver color format preferences.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NDIlib_recv_color_format_e {
    /// No alpha: BGRX, with alpha: BGRA.
    BGRX_BGRA = 0,
    /// No alpha: UYVY, with alpha: BGRA.
    UYVY_BGRA = 1,
    /// No alpha: RGBX, with alpha: RGBA.
    RGBX_RGBA = 2,
    /// No alpha: UYVY, with alpha: RGBA.
    UYVY_RGBA = 3,
    /// Fastest available format.
    Fastest = 100,
    /// Best quality format.
    Best = 101,
}

/// Frame format types.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NDIlib_frame_format_type_e {
    /// Progressive frame.
    Progressive = 1,
    /// Interlaced frame.
    Interlaced = 0,
    /// Field 0.
    Field0 = 2,
    /// Field 1.
    Field1 = 3,
}

// =============================================================================
// Structures
// =============================================================================

/// NDI source descriptor.
#[repr(C)]
#[derive(Debug)]
pub struct NDIlib_source_t {
    /// UTF-8 source name in format "MACHINE_NAME (NDI_SOURCE_NAME)".
    pub p_ndi_name: *const c_char,
    /// URL address for direct connection (may be NULL).
    pub p_url_address: *const c_char,
}

impl Default for NDIlib_source_t {
    fn default() -> Self {
        Self {
            p_ndi_name: ptr::null(),
            p_url_address: ptr::null(),
        }
    }
}

/// Finder creation settings.
#[repr(C)]
#[derive(Debug)]
pub struct NDIlib_find_create_t {
    /// Include local sources in the list.
    pub show_local_sources: bool,
    /// Groups to search (NULL for default).
    pub p_groups: *const c_char,
    /// Extra IP addresses to query (comma-separated).
    pub p_extra_ips: *const c_char,
}

impl Default for NDIlib_find_create_t {
    fn default() -> Self {
        Self {
            show_local_sources: true,
            p_groups: ptr::null(),
            p_extra_ips: ptr::null(),
        }
    }
}

/// Receiver creation settings (v3).
#[repr(C)]
#[derive(Debug)]
pub struct NDIlib_recv_create_v3_t {
    /// Source to connect to.
    pub source_to_connect_to: NDIlib_source_t,
    /// Preferred color format.
    pub color_format: NDIlib_recv_color_format_e,
    /// Bandwidth setting.
    pub bandwidth: NDIlib_recv_bandwidth_e,
    /// Allow fielded video (false = always progressive).
    pub allow_video_fields: bool,
    /// Receiver name (NULL for auto).
    pub p_ndi_recv_name: *const c_char,
}

impl Default for NDIlib_recv_create_v3_t {
    fn default() -> Self {
        Self {
            source_to_connect_to: NDIlib_source_t::default(),
            color_format: NDIlib_recv_color_format_e::BGRX_BGRA,
            bandwidth: NDIlib_recv_bandwidth_e::Highest,
            allow_video_fields: false,
            p_ndi_recv_name: ptr::null(),
        }
    }
}

/// Sender creation settings.
#[repr(C)]
#[derive(Debug)]
pub struct NDIlib_send_create_t {
    /// Name of the NDI source to create.
    pub p_ndi_name: *const c_char,
    /// Groups to join (NULL for default).
    pub p_groups: *const c_char,
    /// Clock video to frame rate.
    pub clock_video: bool,
    /// Clock audio to sample rate.
    pub clock_audio: bool,
}

impl Default for NDIlib_send_create_t {
    fn default() -> Self {
        Self {
            p_ndi_name: ptr::null(),
            p_groups: ptr::null(),
            clock_video: true,
            clock_audio: true,
        }
    }
}

/// Video frame structure (v2).
#[repr(C)]
#[derive(Debug)]
pub struct NDIlib_video_frame_v2_t {
    /// Horizontal resolution.
    pub xres: c_int,
    /// Vertical resolution.
    pub yres: c_int,
    /// FourCC pixel format.
    pub FourCC: NDIlib_FourCC_video_type_e,
    /// Frame rate numerator.
    pub frame_rate_N: c_int,
    /// Frame rate denominator.
    pub frame_rate_D: c_int,
    /// Picture aspect ratio (0 = square pixels).
    pub picture_aspect_ratio: f32,
    /// Frame format (progressive/interlaced).
    pub frame_format_type: NDIlib_frame_format_type_e,
    /// Timecode in 100ns intervals.
    pub timecode: i64,
    /// Pointer to pixel data.
    pub p_data: *mut u8,
    /// Line stride in bytes (0 = default).
    pub line_stride_in_bytes: c_int,
    /// Per-frame metadata (UTF-8 XML, may be NULL).
    pub p_metadata: *const c_char,
    /// Timestamp in 100ns intervals.
    pub timestamp: i64,
}

impl Default for NDIlib_video_frame_v2_t {
    fn default() -> Self {
        Self {
            xres: 0,
            yres: 0,
            FourCC: NDIlib_FourCC_video_type_e::BGRA,
            frame_rate_N: 60000,
            frame_rate_D: 1001,
            picture_aspect_ratio: 0.0,
            frame_format_type: NDIlib_frame_format_type_e::Progressive,
            timecode: NDILIB_SEND_TIMECODE_SYNTHESIZE,
            p_data: ptr::null_mut(),
            line_stride_in_bytes: 0,
            p_metadata: ptr::null(),
            timestamp: 0,
        }
    }
}

/// Audio frame structure (v2).
#[repr(C)]
#[derive(Debug)]
pub struct NDIlib_audio_frame_v2_t {
    /// Sample rate (e.g., 48000).
    pub sample_rate: c_int,
    /// Number of audio channels.
    pub no_channels: c_int,
    /// Number of samples per channel.
    pub no_samples: c_int,
    /// Timecode in 100ns intervals.
    pub timecode: i64,
    /// Pointer to audio data (32-bit float planar).
    pub p_data: *mut f32,
    /// Channel stride in bytes.
    pub channel_stride_in_bytes: c_int,
    /// Per-frame metadata (UTF-8 XML, may be NULL).
    pub p_metadata: *const c_char,
    /// Timestamp in 100ns intervals.
    pub timestamp: i64,
}

impl Default for NDIlib_audio_frame_v2_t {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            no_channels: 2,
            no_samples: 0,
            timecode: NDILIB_SEND_TIMECODE_SYNTHESIZE,
            p_data: ptr::null_mut(),
            channel_stride_in_bytes: 0,
            p_metadata: ptr::null(),
            timestamp: 0,
        }
    }
}

/// Metadata frame structure.
#[repr(C)]
#[derive(Debug)]
pub struct NDIlib_metadata_frame_t {
    /// Length of string in UTF-8 characters (0 = use strlen).
    pub length: c_int,
    /// Timecode in 100ns intervals.
    pub timecode: i64,
    /// Metadata as UTF-8 XML string.
    pub p_data: *mut c_char,
}

impl Default for NDIlib_metadata_frame_t {
    fn default() -> Self {
        Self {
            length: 0,
            timecode: NDILIB_SEND_TIMECODE_SYNTHESIZE,
            p_data: ptr::null_mut(),
        }
    }
}

/// Tally state structure.
#[repr(C)]
#[derive(Debug, Default)]
pub struct NDIlib_tally_t {
    /// Is this source on program output.
    pub on_program: bool,
    /// Is this source on preview output.
    pub on_preview: bool,
}

/// Receiver performance statistics.
#[repr(C)]
#[derive(Debug, Default)]
pub struct NDIlib_recv_performance_t {
    /// Number of video frames.
    pub video_frames: i64,
    /// Number of audio frames.
    pub audio_frames: i64,
    /// Number of metadata frames.
    pub metadata_frames: i64,
}

/// Receiver queue depths.
#[repr(C)]
#[derive(Debug, Default)]
pub struct NDIlib_recv_queue_t {
    /// Number of video frames queued.
    pub video_frames: c_int,
    /// Number of audio frames queued.
    pub audio_frames: c_int,
    /// Number of metadata frames queued.
    pub metadata_frames: c_int,
}

// =============================================================================
// FFI Function Declarations
// =============================================================================

#[link(name = "ndi")]
extern "C" {
    // =========================================================================
    // Library Lifecycle
    // =========================================================================

    /// Initialize NDI library. Returns false if CPU is not supported.
    pub fn NDIlib_initialize() -> bool;

    /// Destroy NDI library and free resources.
    pub fn NDIlib_destroy();

    /// Get NDI library version string.
    pub fn NDIlib_version() -> *const c_char;

    /// Check if current CPU supports NDI (requires SSE4.2).
    pub fn NDIlib_is_supported_CPU() -> bool;

    // =========================================================================
    // Finder (Discovery)
    // =========================================================================

    /// Create a new finder instance.
    pub fn NDIlib_find_create_v2(
        p_create_settings: *const NDIlib_find_create_t,
    ) -> NDIlib_find_instance_t;

    /// Destroy a finder instance.
    pub fn NDIlib_find_destroy(p_instance: NDIlib_find_instance_t);

    /// Get current list of sources. Valid until next call or destroy.
    pub fn NDIlib_find_get_current_sources(
        p_instance: NDIlib_find_instance_t,
        p_no_sources: *mut u32,
    ) -> *const NDIlib_source_t;

    /// Wait for source list to change.
    pub fn NDIlib_find_wait_for_sources(
        p_instance: NDIlib_find_instance_t,
        timeout_in_ms: u32,
    ) -> bool;

    // =========================================================================
    // Receiver
    // =========================================================================

    /// Create a new receiver instance.
    pub fn NDIlib_recv_create_v3(
        p_create_settings: *const NDIlib_recv_create_v3_t,
    ) -> NDIlib_recv_instance_t;

    /// Destroy a receiver instance.
    pub fn NDIlib_recv_destroy(p_instance: NDIlib_recv_instance_t);

    /// Change the source being received (NULL to disconnect).
    pub fn NDIlib_recv_connect(
        p_instance: NDIlib_recv_instance_t,
        p_src: *const NDIlib_source_t,
    );

    /// Capture video, audio, and/or metadata frames.
    pub fn NDIlib_recv_capture_v2(
        p_instance: NDIlib_recv_instance_t,
        p_video_data: *mut NDIlib_video_frame_v2_t,
        p_audio_data: *mut NDIlib_audio_frame_v2_t,
        p_metadata: *mut NDIlib_metadata_frame_t,
        timeout_in_ms: u32,
    ) -> NDIlib_frame_type_e;

    /// Free a captured video frame.
    pub fn NDIlib_recv_free_video_v2(
        p_instance: NDIlib_recv_instance_t,
        p_video_data: *const NDIlib_video_frame_v2_t,
    );

    /// Free a captured audio frame.
    pub fn NDIlib_recv_free_audio_v2(
        p_instance: NDIlib_recv_instance_t,
        p_audio_data: *const NDIlib_audio_frame_v2_t,
    );

    /// Free a captured metadata frame.
    pub fn NDIlib_recv_free_metadata(
        p_instance: NDIlib_recv_instance_t,
        p_metadata: *const NDIlib_metadata_frame_t,
    );

    /// Free a string allocated by the receiver.
    pub fn NDIlib_recv_free_string(
        p_instance: NDIlib_recv_instance_t,
        p_string: *const c_char,
    );

    /// Send metadata to the connected source.
    pub fn NDIlib_recv_send_metadata(
        p_instance: NDIlib_recv_instance_t,
        p_metadata: *const NDIlib_metadata_frame_t,
    ) -> bool;

    /// Set tally state for the connected source.
    pub fn NDIlib_recv_set_tally(
        p_instance: NDIlib_recv_instance_t,
        p_tally: *const NDIlib_tally_t,
    ) -> bool;

    /// Get performance statistics.
    pub fn NDIlib_recv_get_performance(
        p_instance: NDIlib_recv_instance_t,
        p_total: *mut NDIlib_recv_performance_t,
        p_dropped: *mut NDIlib_recv_performance_t,
    );

    /// Get current queue depths.
    pub fn NDIlib_recv_get_queue(
        p_instance: NDIlib_recv_instance_t,
        p_total: *mut NDIlib_recv_queue_t,
    );

    /// Get number of connections (0 or 1).
    pub fn NDIlib_recv_get_no_connections(p_instance: NDIlib_recv_instance_t) -> c_int;

    // =========================================================================
    // Sender
    // =========================================================================

    /// Create a new sender instance.
    pub fn NDIlib_send_create(
        p_create_settings: *const NDIlib_send_create_t,
    ) -> NDIlib_send_instance_t;

    /// Destroy a sender instance.
    pub fn NDIlib_send_destroy(p_instance: NDIlib_send_instance_t);

    /// Send a video frame (synchronous).
    pub fn NDIlib_send_send_video_v2(
        p_instance: NDIlib_send_instance_t,
        p_video_data: *const NDIlib_video_frame_v2_t,
    );

    /// Send a video frame (asynchronous).
    pub fn NDIlib_send_send_video_async_v2(
        p_instance: NDIlib_send_instance_t,
        p_video_data: *const NDIlib_video_frame_v2_t,
    );

    /// Send an audio frame.
    pub fn NDIlib_send_send_audio_v2(
        p_instance: NDIlib_send_instance_t,
        p_audio_data: *const NDIlib_audio_frame_v2_t,
    );

    /// Send a metadata frame.
    pub fn NDIlib_send_send_metadata(
        p_instance: NDIlib_send_instance_t,
        p_metadata: *const NDIlib_metadata_frame_t,
    );

    /// Get tally state from connected receivers.
    pub fn NDIlib_send_get_tally(
        p_instance: NDIlib_send_instance_t,
        p_tally: *mut NDIlib_tally_t,
        timeout_in_ms: u32,
    ) -> bool;

    /// Get number of connected receivers.
    pub fn NDIlib_send_get_no_connections(
        p_instance: NDIlib_send_instance_t,
        timeout_in_ms: u32,
    ) -> c_int;

    /// Clear connection metadata.
    pub fn NDIlib_send_clear_connection_metadata(p_instance: NDIlib_send_instance_t);

    /// Add connection metadata.
    pub fn NDIlib_send_add_connection_metadata(
        p_instance: NDIlib_send_instance_t,
        p_metadata: *const NDIlib_metadata_frame_t,
    );

    /// Get source info for the sender.
    pub fn NDIlib_send_get_source_name(
        p_instance: NDIlib_send_instance_t,
    ) -> *const NDIlib_source_t;
}

// =============================================================================
// Safe Rust Wrappers
// =============================================================================

/// Initialize NDI library. Call once at application startup.
/// Returns true if initialization succeeded.
pub fn initialize() -> bool {
    unsafe { NDIlib_initialize() }
}

/// Destroy NDI library. Call at application shutdown.
pub fn destroy() {
    unsafe { NDIlib_destroy() }
}

/// Get NDI library version string.
pub fn version() -> String {
    let version_ptr = unsafe { NDIlib_version() };
    if version_ptr.is_null() {
        return "unknown".to_string();
    }
    unsafe { CStr::from_ptr(version_ptr) }
        .to_str()
        .unwrap_or("unknown")
        .to_string()
}

/// Check if current CPU supports NDI.
pub fn is_supported_cpu() -> bool {
    unsafe { NDIlib_is_supported_CPU() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enum_values() {
        // Verify FourCC values match expected
        assert_eq!(NDIlib_FourCC_video_type_e::BGRA as u32, 0x41524742);
        assert_eq!(NDIlib_FourCC_video_type_e::RGBA as u32, 0x41424752);
        assert_eq!(NDIlib_FourCC_video_type_e::UYVY as u32, 0x59565955);
    }

    #[test]
    fn test_frame_type_values() {
        assert_eq!(NDIlib_frame_type_e::None as i32, 0);
        assert_eq!(NDIlib_frame_type_e::Video as i32, 1);
        assert_eq!(NDIlib_frame_type_e::Error as i32, 4);
    }

    #[test]
    fn test_default_structs() {
        let source = NDIlib_source_t::default();
        assert!(source.p_ndi_name.is_null());

        let video_frame = NDIlib_video_frame_v2_t::default();
        assert_eq!(video_frame.xres, 0);
        assert!(video_frame.p_data.is_null());
    }
}
