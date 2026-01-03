//! Network module for OMT (Open Media Transport), Syphon/Spout, and NDI integration.
//!
//! This module provides:
//! - OMT input/output via official libOMT (C library for OBS compatibility)
//! - OMT capture for streaming compositor output
//! - NDI input/output via official NDI SDK
//! - Syphon (macOS) / Spout (Windows) GPU texture sharing
//! - Source discovery on the local network

pub mod discovery;
pub mod ndi;
pub mod ndi_capture;
pub mod ndi_ffi;
pub mod omt;
pub mod omt_capture;
pub mod omt_ffi;
pub mod texture_share;

#[cfg(target_os = "macos")]
pub mod syphon;
#[cfg(target_os = "macos")]
pub mod syphon_ffi;

#[cfg(target_os = "windows")]
pub mod spout;
#[cfg(target_os = "windows")]
pub mod spout_ffi;
#[cfg(target_os = "windows")]
pub mod spout_capture;

pub use discovery::{DiscoveredSource, SourceDiscovery, SourceType};
pub use ndi::{NdiFrame, NdiReceiver, NdiSender};
pub use ndi_capture::NdiCapture;
pub use omt::{OmtFrame, OmtReceiver, OmtSender};
pub use omt_capture::{CapturedFrame, OmtCapture};
pub use texture_share::{is_available as texture_share_available, platform_technology_name, TextureSharer};

#[cfg(target_os = "macos")]
pub use syphon::SyphonSharer;

#[cfg(target_os = "windows")]
pub use spout::SpoutSharer;
#[cfg(target_os = "windows")]
pub use spout_capture::SpoutCapture;

