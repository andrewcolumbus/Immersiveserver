//! Video decoder using FFmpeg
//!
//! Provides video file decoding to RGBA frames using the ffmpeg-next crate.

use std::path::Path;

use super::DecodedFrame;

/// Errors that can occur during video decoding
#[derive(Debug)]
pub enum VideoDecoderError {
    /// Failed to open the video file
    OpenFailed(String),
    /// No video stream found in the file
    NoVideoStream,
    /// Failed to create decoder
    DecoderCreationFailed(String),
    /// Failed to create scaler
    ScalerCreationFailed(String),
    /// Decoding error
    DecodeFailed(String),
    /// FFmpeg error
    Ffmpeg(ffmpeg_next::Error),
}

impl std::fmt::Display for VideoDecoderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VideoDecoderError::OpenFailed(path) => write!(f, "Failed to open video file: {}", path),
            VideoDecoderError::NoVideoStream => write!(f, "No video stream found in file"),
            VideoDecoderError::DecoderCreationFailed(msg) => {
                write!(f, "Failed to create decoder: {}", msg)
            }
            VideoDecoderError::ScalerCreationFailed(msg) => {
                write!(f, "Failed to create scaler: {}", msg)
            }
            VideoDecoderError::DecodeFailed(msg) => write!(f, "Decoding failed: {}", msg),
            VideoDecoderError::Ffmpeg(e) => write!(f, "FFmpeg error: {}", e),
        }
    }
}

impl std::error::Error for VideoDecoderError {}

impl From<ffmpeg_next::Error> for VideoDecoderError {
    fn from(e: ffmpeg_next::Error) -> Self {
        VideoDecoderError::Ffmpeg(e)
    }
}

/// Video decoder that reads frames from a video file
pub struct VideoDecoder {
    /// The input format context
    input: ffmpeg_next::format::context::Input,
    /// Index of the video stream
    video_stream_index: usize,
    /// Video decoder
    decoder: ffmpeg_next::decoder::Video,
    /// Scaler for converting to RGBA
    scaler: ffmpeg_next::software::scaling::Context,
    /// Video width
    width: u32,
    /// Video height
    height: u32,
    /// Frame rate (fps)
    frame_rate: f64,
    /// Video duration in seconds
    duration: f64,
    /// Time base for PTS conversion
    time_base: f64,
    /// Current frame index
    frame_index: u64,
    /// Whether we've reached end of file
    eof: bool,
}

impl VideoDecoder {
    /// Open a video file for decoding
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, VideoDecoderError> {
        // Initialize FFmpeg (safe to call multiple times)
        ffmpeg_next::init()?;

        let path = path.as_ref();
        let path_str = path.to_string_lossy().to_string();

        // Open input file
        let input = ffmpeg_next::format::input(&path)
            .map_err(|_| VideoDecoderError::OpenFailed(path_str.clone()))?;

        // Find best video stream
        let video_stream = input
            .streams()
            .best(ffmpeg_next::media::Type::Video)
            .ok_or(VideoDecoderError::NoVideoStream)?;

        let video_stream_index = video_stream.index();

        // Get stream parameters
        let time_base = video_stream.time_base();
        let time_base_f64 = time_base.numerator() as f64 / time_base.denominator() as f64;

        let frame_rate = video_stream.avg_frame_rate();
        let frame_rate_f64 = if frame_rate.denominator() > 0 {
            frame_rate.numerator() as f64 / frame_rate.denominator() as f64
        } else {
            30.0 // Default fallback
        };

        let duration = if video_stream.duration() > 0 {
            video_stream.duration() as f64 * time_base_f64
        } else if input.duration() > 0 {
            input.duration() as f64 / ffmpeg_next::ffi::AV_TIME_BASE as f64
        } else {
            0.0
        };

        // Create decoder
        let context =
            ffmpeg_next::codec::context::Context::from_parameters(video_stream.parameters())?;
        let decoder = context.decoder().video().map_err(|e| {
            VideoDecoderError::DecoderCreationFailed(format!("Failed to create video decoder: {}", e))
        })?;

        let width = decoder.width();
        let height = decoder.height();

        log::info!(
            "Opened video: {}x{} @ {:.2}fps, duration: {:.2}s",
            width,
            height,
            frame_rate_f64,
            duration
        );

        // Create scaler to convert to RGBA
        let scaler = ffmpeg_next::software::scaling::Context::get(
            decoder.format(),
            width,
            height,
            ffmpeg_next::format::Pixel::RGBA,
            width,
            height,
            ffmpeg_next::software::scaling::Flags::BILINEAR,
        )
        .map_err(|e| VideoDecoderError::ScalerCreationFailed(e.to_string()))?;

        Ok(Self {
            input,
            video_stream_index,
            decoder,
            scaler,
            width,
            height,
            frame_rate: frame_rate_f64,
            duration,
            time_base: time_base_f64,
            frame_index: 0,
            eof: false,
        })
    }

    /// Decode the next frame, returning None if end of file
    pub fn decode_next_frame(&mut self) -> Result<Option<DecodedFrame>, VideoDecoderError> {
        if self.eof {
            return Ok(None);
        }

        // Try to receive a decoded frame
        let mut decoded_frame = ffmpeg_next::frame::Video::empty();

        loop {
            // First, try to receive any pending frames from the decoder
            match self.decoder.receive_frame(&mut decoded_frame) {
                Ok(()) => {
                    // Got a frame, convert to RGBA
                    let mut rgba_frame = ffmpeg_next::frame::Video::empty();
                    self.scaler.run(&decoded_frame, &mut rgba_frame)?;

                    // Calculate PTS
                    let pts = decoded_frame.pts().unwrap_or(0) as f64 * self.time_base;

                    // Copy data to Vec
                    let data = rgba_frame.data(0);
                    let stride = rgba_frame.stride(0);
                    let expected_stride = (self.width as usize) * 4;

                    // Handle stride vs width mismatch
                    let rgba_data = if stride == expected_stride as usize {
                        data[..DecodedFrame::expected_size(self.width, self.height)].to_vec()
                    } else {
                        // Need to copy row by row
                        let mut output =
                            Vec::with_capacity(DecodedFrame::expected_size(self.width, self.height));
                        for y in 0..self.height as usize {
                            let row_start = y * stride;
                            let row_end = row_start + expected_stride;
                            output.extend_from_slice(&data[row_start..row_end]);
                        }
                        output
                    };

                    let frame = DecodedFrame::new(
                        rgba_data,
                        self.width,
                        self.height,
                        pts,
                        self.frame_index,
                    );
                    self.frame_index += 1;

                    return Ok(Some(frame));
                }
                Err(ffmpeg_next::Error::Other {
                    errno: ffmpeg_next::error::EAGAIN,
                }) => {
                    // Need more input - read next packet
                }
                Err(ffmpeg_next::Error::Eof) => {
                    self.eof = true;
                    return Ok(None);
                }
                Err(e) => {
                    return Err(VideoDecoderError::DecodeFailed(e.to_string()));
                }
            }

            // Read next packet
            loop {
                match self.input.packets().next() {
                    Some((stream, packet)) => {
                        if stream.index() == self.video_stream_index {
                            // Send packet to decoder
                            self.decoder.send_packet(&packet)?;
                            break;
                        }
                        // Skip non-video packets
                    }
                    None => {
                        // End of file - flush decoder
                        self.decoder.send_eof()?;
                        self.eof = true;
                        break;
                    }
                }
            }
        }
    }

    /// Reset the decoder to the beginning of the file
    pub fn reset(&mut self) -> Result<(), VideoDecoderError> {
        // Seek to beginning
        self.input.seek(0, ..)?;
        self.decoder.flush();
        self.frame_index = 0;
        self.eof = false;
        Ok(())
    }

    /// Get the video width in pixels
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get the video height in pixels
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get the video frame rate (fps)
    pub fn frame_rate(&self) -> f64 {
        self.frame_rate
    }

    /// Get the video duration in seconds
    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// Get the estimated total frame count
    pub fn estimated_frame_count(&self) -> u64 {
        (self.duration * self.frame_rate) as u64
    }

    /// Get the current frame index
    pub fn current_frame_index(&self) -> u64 {
        self.frame_index
    }

    /// Check if we've reached end of file
    pub fn is_eof(&self) -> bool {
        self.eof
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_error_display() {
        let err = VideoDecoderError::NoVideoStream;
        assert_eq!(err.to_string(), "No video stream found in file");
    }
}

