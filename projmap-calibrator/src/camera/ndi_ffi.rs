//! FFI bindings to the NDI SDK (libndi).
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

pub type NDIlib_find_instance_t = *mut c_void;
pub type NDIlib_recv_instance_t = *mut c_void;

// =============================================================================
// Constants
// =============================================================================

pub const NDILIB_RECV_TIMESTAMP_UNDEFINED: i64 = i64::MAX;

// =============================================================================
// Enumerations
// =============================================================================

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NDIlib_frame_type_e {
    None = 0,
    Video = 1,
    Audio = 2,
    Metadata = 3,
    Error = 4,
    StatusChange = 100,
    SourceChange = 101,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NDIlib_FourCC_video_type_e {
    UYVY = 0x59565955,
    BGRA = 0x41524742,
    BGRX = 0x58524742,
    RGBA = 0x41424752,
    RGBX = 0x58424752,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NDIlib_recv_bandwidth_e {
    MetadataOnly = -10,
    AudioOnly = 10,
    Lowest = 0,
    Highest = 100,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NDIlib_recv_color_format_e {
    BGRX_BGRA = 0,
    UYVY_BGRA = 1,
    RGBX_RGBA = 2,
    UYVY_RGBA = 3,
    Fastest = 100,
    Best = 101,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NDIlib_frame_format_type_e {
    Progressive = 1,
    Interlaced = 0,
    Field0 = 2,
    Field1 = 3,
}

// =============================================================================
// Structures
// =============================================================================

#[repr(C)]
#[derive(Debug)]
pub struct NDIlib_source_t {
    pub p_ndi_name: *const c_char,
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

#[repr(C)]
#[derive(Debug)]
pub struct NDIlib_find_create_t {
    pub show_local_sources: bool,
    pub p_groups: *const c_char,
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

#[repr(C)]
#[derive(Debug)]
pub struct NDIlib_recv_create_v3_t {
    pub source_to_connect_to: NDIlib_source_t,
    pub color_format: NDIlib_recv_color_format_e,
    pub bandwidth: NDIlib_recv_bandwidth_e,
    pub allow_video_fields: bool,
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

#[repr(C)]
#[derive(Debug)]
pub struct NDIlib_video_frame_v2_t {
    pub xres: c_int,
    pub yres: c_int,
    pub FourCC: NDIlib_FourCC_video_type_e,
    pub frame_rate_N: c_int,
    pub frame_rate_D: c_int,
    pub picture_aspect_ratio: f32,
    pub frame_format_type: NDIlib_frame_format_type_e,
    pub timecode: i64,
    pub p_data: *mut u8,
    pub line_stride_in_bytes: c_int,
    pub p_metadata: *const c_char,
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
            timecode: 0,
            p_data: ptr::null_mut(),
            line_stride_in_bytes: 0,
            p_metadata: ptr::null(),
            timestamp: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct NDIlib_audio_frame_v2_t {
    pub sample_rate: c_int,
    pub no_channels: c_int,
    pub no_samples: c_int,
    pub timecode: i64,
    pub p_data: *mut f32,
    pub channel_stride_in_bytes: c_int,
    pub p_metadata: *const c_char,
    pub timestamp: i64,
}

impl Default for NDIlib_audio_frame_v2_t {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            no_channels: 2,
            no_samples: 0,
            timecode: 0,
            p_data: ptr::null_mut(),
            channel_stride_in_bytes: 0,
            p_metadata: ptr::null(),
            timestamp: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct NDIlib_metadata_frame_t {
    pub length: c_int,
    pub timecode: i64,
    pub p_data: *mut c_char,
}

impl Default for NDIlib_metadata_frame_t {
    fn default() -> Self {
        Self {
            length: 0,
            timecode: 0,
            p_data: ptr::null_mut(),
        }
    }
}

// =============================================================================
// FFI Function Declarations
// =============================================================================

#[link(name = "ndi")]
extern "C" {
    pub fn NDIlib_initialize() -> bool;
    pub fn NDIlib_destroy();
    pub fn NDIlib_version() -> *const c_char;
    pub fn NDIlib_is_supported_CPU() -> bool;

    // Finder
    pub fn NDIlib_find_create_v2(
        p_create_settings: *const NDIlib_find_create_t,
    ) -> NDIlib_find_instance_t;
    pub fn NDIlib_find_destroy(p_instance: NDIlib_find_instance_t);
    pub fn NDIlib_find_get_current_sources(
        p_instance: NDIlib_find_instance_t,
        p_no_sources: *mut u32,
    ) -> *const NDIlib_source_t;
    pub fn NDIlib_find_wait_for_sources(
        p_instance: NDIlib_find_instance_t,
        timeout_in_ms: u32,
    ) -> bool;

    // Receiver
    pub fn NDIlib_recv_create_v3(
        p_create_settings: *const NDIlib_recv_create_v3_t,
    ) -> NDIlib_recv_instance_t;
    pub fn NDIlib_recv_destroy(p_instance: NDIlib_recv_instance_t);
    pub fn NDIlib_recv_connect(
        p_instance: NDIlib_recv_instance_t,
        p_src: *const NDIlib_source_t,
    );
    pub fn NDIlib_recv_capture_v2(
        p_instance: NDIlib_recv_instance_t,
        p_video_data: *mut NDIlib_video_frame_v2_t,
        p_audio_data: *mut NDIlib_audio_frame_v2_t,
        p_metadata: *mut NDIlib_metadata_frame_t,
        timeout_in_ms: u32,
    ) -> NDIlib_frame_type_e;
    pub fn NDIlib_recv_free_video_v2(
        p_instance: NDIlib_recv_instance_t,
        p_video_data: *const NDIlib_video_frame_v2_t,
    );
    pub fn NDIlib_recv_get_no_connections(p_instance: NDIlib_recv_instance_t) -> c_int;
}

// =============================================================================
// Safe Rust Wrappers
// =============================================================================

pub fn initialize() -> bool {
    unsafe { NDIlib_initialize() }
}

pub fn destroy() {
    unsafe { NDIlib_destroy() }
}

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

pub fn is_supported_cpu() -> bool {
    unsafe { NDIlib_is_supported_CPU() }
}
