//! HAP Video Converter Module
//!
//! Converts various video formats to HAP using FFmpeg.

mod ffmpeg;
mod formats;
mod job;
mod queue;
mod window;

pub use window::ConverterWindow;



