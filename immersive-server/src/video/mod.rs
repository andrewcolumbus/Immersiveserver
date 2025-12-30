//! Video decoding, GPU texture, and rendering module
//!
//! Provides video file decoding using FFmpeg via the `ffmpeg-next` crate.
//! Decoded frames are returned as RGBA pixel buffers ready for GPU upload.
//! Also provides GPU texture management and rendering for displaying video frames.
//!
//! Hardware acceleration is supported via:
//! - VideoToolbox (macOS)
//! - D3D11VA / NVDEC (Windows)
//!
//! HAP codec support allows direct GPU texture upload without CPU decompression.

mod decoder;
mod frame;
mod hap;
mod player;
mod renderer;
mod texture;

pub use decoder::{HwAccelMethod, VideoDecoder, VideoDecoderError};
pub use frame::DecodedFrame;
pub use hap::{HapDecoder, HapFormat, HapFrame};
pub use player::{VideoInfo, VideoPlayer};
pub use renderer::{LayerParams, VideoParams, VideoRenderer};
pub use texture::VideoTexture;

