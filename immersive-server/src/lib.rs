//! Immersive Server Library
//!
//! A high-performance, cross-platform media server for macOS and Windows.
//! Designed for professional projection mapping, NDI/OMT streaming, and real-time web control.

pub mod app;
pub mod compositor;
pub mod converter;
pub mod layer_runtime;
pub mod network;
pub mod settings;
pub mod shaders;
pub mod ui;
pub mod video;

pub use app::App;
pub use compositor::{BlendMode, ClipCell, ClipSource, ClipTransition, DEFAULT_CLIP_SLOTS, Environment, Layer, LayerSource, Transform2D, Viewport};
pub use layer_runtime::LayerRuntime;
pub use settings::{AppPreferences, EnvironmentSettings};
pub use video::{DecodedFrame, LayerParams, VideoDecoder, VideoDecoderError, VideoInfo, VideoParams, VideoPlayer, VideoRenderer, VideoTexture};
pub use network::{DiscoveredSource, OmtFrame, OmtReceiver, OmtSender, SourceDiscovery, SourceType};

