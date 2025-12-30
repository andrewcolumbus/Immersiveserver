//! Network module for OMT (Open Media Transport) and NDI integration.
//!
//! This module provides:
//! - OMT input/output via Aqueduct (Rust-native implementation)
//! - Source discovery on the local network
//! - Future: NDI input/output support

pub mod omt;
pub mod discovery;

pub use omt::{OmtReceiver, OmtSender, OmtFrame};
pub use discovery::{SourceDiscovery, DiscoveredSource, SourceType};

