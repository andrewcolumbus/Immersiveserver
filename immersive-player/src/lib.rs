//! Immersive Player Library
//!
//! A cross-platform HAP video player with projection blending capabilities.

pub mod api;
pub mod app;
pub mod composition;
pub mod converter;
pub mod output;
pub mod project;
pub mod render;
pub mod ui;
pub mod video;

// Re-export commonly used types
pub use app::ImmersivePlayerApp;
pub use composition::{BlendMode, Clip, ClipSlot, Composition, CompositionSettings, Layer};
pub use converter::ConverterWindow;
pub use output::{BlendConfig, OutputManager, Screen, Slice};
pub use project::ProjectPreset;
pub use render::Compositor;
pub use video::{HapDecoder, VideoPlayer};
