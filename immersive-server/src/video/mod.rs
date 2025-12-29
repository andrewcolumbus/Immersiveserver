//! Video decoding, GPU texture, and rendering module
//!
//! Provides video file decoding using FFmpeg via the `ffmpeg-next` crate.
//! Decoded frames are returned as RGBA pixel buffers ready for GPU upload.
//! Also provides GPU texture management and rendering for displaying video frames.

mod decoder;
mod frame;
mod player;
mod renderer;
mod texture;

pub use decoder::{VideoDecoder, VideoDecoderError};
pub use frame::DecodedFrame;
pub use player::{VideoInfo, VideoPlayer};
pub use renderer::{LayerParams, VideoParams, VideoRenderer};
pub use texture::VideoTexture;

