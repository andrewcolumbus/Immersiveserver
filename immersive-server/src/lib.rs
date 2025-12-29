//! Immersive Server Library
//!
//! A high-performance, cross-platform media server for macOS and Windows.
//! Designed for professional projection mapping, NDI/OMT streaming, and real-time web control.

pub mod app;
pub mod compositor;
pub mod settings;
pub mod ui;
pub mod video;

pub use app::App;
pub use compositor::{Environment, Viewport};
pub use settings::{AppPreferences, EnvironmentSettings};
pub use video::{DecodedFrame, VideoDecoder, VideoDecoderError, VideoInfo, VideoParams, VideoPlayer, VideoRenderer, VideoTexture};

