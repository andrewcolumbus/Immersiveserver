//! Network module for OMT (Open Media Transport) and NDI integration.
//!
//! This module provides:
//! - OMT input/output via official libOMT (C library for OBS compatibility)
//! - OMT capture for streaming compositor output
//! - Source discovery on the local network
//! - Future: NDI input/output support

pub mod omt_ffi;
pub mod omt;
pub mod omt_capture;
pub mod discovery;

pub use omt::{OmtReceiver, OmtSender, OmtFrame};
pub use omt_capture::{OmtCapture, CapturedFrame};
pub use discovery::{SourceDiscovery, DiscoveredSource, SourceType};

