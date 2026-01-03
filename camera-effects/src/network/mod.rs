//! Network module for Syphon/Spout texture sharing
//!
//! Provides GPU texture sharing with other applications via:
//! - Syphon (macOS)
//! - Spout (Windows)

pub mod texture_share;

#[cfg(target_os = "macos")]
pub mod syphon;
#[cfg(target_os = "macos")]
pub mod syphon_ffi;

#[cfg(target_os = "windows")]
pub mod spout;
#[cfg(target_os = "windows")]
pub mod spout_ffi;

#[cfg(target_os = "macos")]
pub use syphon::SyphonSharer;

#[cfg(target_os = "windows")]
pub use spout::SpoutSharer;

pub use texture_share::TextureSharer;
