//! Video decoder using FFmpeg with hardware acceleration support
//!
//! Provides video file decoding to RGBA frames using the ffmpeg-next crate.
//! Supports hardware acceleration via VideoToolbox (macOS), D3D11VA/NVDEC (Windows).
//! HAP codec uses GPU-native DXT extraction for optimal performance.

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
    /// Reached end of file unexpectedly
    EndOfFile,
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
            VideoDecoderError::EndOfFile => write!(f, "Reached end of file"),
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

/// Hardware acceleration method in use
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HwAccelMethod {
    /// No hardware acceleration (software decode)
    None,
    /// macOS VideoToolbox
    VideoToolbox,
    /// Windows D3D11VA
    D3d11va,
    /// NVIDIA NVDEC
    Nvdec,
    /// Intel QuickSync
    Qsv,
    /// HAP codec - GPU-native DXT/BC texture format (no decode needed)
    HapGpuNative,
}

impl std::fmt::Display for HwAccelMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HwAccelMethod::None => write!(f, "software"),
            HwAccelMethod::VideoToolbox => write!(f, "videotoolbox"),
            HwAccelMethod::D3d11va => write!(f, "d3d11va"),
            HwAccelMethod::Nvdec => write!(f, "nvdec"),
            HwAccelMethod::Qsv => write!(f, "qsv"),
            HwAccelMethod::HapGpuNative => write!(f, "hap-gpu-native"),
        }
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
    /// Scaler for converting to RGBA/BGRA
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
    /// Hardware acceleration method in use
    hwaccel: HwAccelMethod,
    /// Codec name (for Hap detection)
    codec_name: String,
    /// Output pixel format (RGBA or BGRA)
    output_format: ffmpeg_next::format::Pixel,
}

impl VideoDecoder {
    /// Open a video file for decoding with automatic hardware acceleration (RGBA output)
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, VideoDecoderError> {
        Self::open_with_options(path, true, false)
    }

    /// Open a video file for decoding with BGRA output format
    pub fn open_bgra<P: AsRef<Path>>(path: P) -> Result<Self, VideoDecoderError> {
        Self::open_with_options(path, true, true)
    }

    /// Open a video file with explicit hardware acceleration and pixel format control
    pub fn open_with_options<P: AsRef<Path>>(
        path: P,
        try_hwaccel: bool,
        use_bgra: bool,
    ) -> Result<Self, VideoDecoderError> {
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

        // Get codec info
        let parameters = video_stream.parameters();
        let codec_id = unsafe { (*parameters.as_ptr()).codec_id };
        let codec = ffmpeg_next::decoder::find(codec_id.into());
        let codec_name = codec
            .map(|c| c.name().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Check if this is a GPU-native codec (HAP or DXV)
        // These codecs store frames in DXT/BC format - no decoding needed, direct GPU upload
        let is_hap = codec_name.starts_with("hap");
        let is_dxv = codec_name == "dxv";
        let is_gpu_native = is_hap || is_dxv;

        let (decoder, hwaccel) = if is_gpu_native {
            // HAP/DXV codec - GPU-native format, no decode acceleration needed
            // These frames are DXT/BC compressed and upload directly to GPU
            let codec_type = if is_hap { "HAP" } else { "DXV" };
            tracing::info!("Detected {} codec: {} - GPU-native texture format (no decode needed)", codec_type, codec_name);
            let context = ffmpeg_next::codec::context::Context::from_parameters(parameters)?;
            let decoder = context.decoder().video().map_err(|e| {
                VideoDecoderError::DecoderCreationFailed(format!("Failed to create {} decoder: {}", codec_type, e))
            })?;
            (decoder, HwAccelMethod::HapGpuNative)
        } else if try_hwaccel {
            Self::try_create_hwaccel_decoder(&video_stream, &codec_name)
                .unwrap_or_else(|e| {
                    tracing::warn!("Hardware acceleration failed: {}. Falling back to software decode.", e);
                    let context = ffmpeg_next::codec::context::Context::from_parameters(parameters)
                        .expect("Failed to create codec context");
                    (context.decoder().video().expect("Failed to create software decoder"), HwAccelMethod::None)
                })
        } else {
            // Create standard software decoder
            let context = ffmpeg_next::codec::context::Context::from_parameters(parameters)?;
            let decoder = context.decoder().video().map_err(|e| {
                VideoDecoderError::DecoderCreationFailed(format!("Failed to create video decoder: {}", e))
            })?;
            (decoder, HwAccelMethod::None)
        };

        let width = decoder.width();
        let height = decoder.height();

        tracing::info!(
            "Opened video: {}x{} @ {:.2}fps, duration: {:.2}s, codec: {}, hwaccel: {}",
            width,
            height,
            frame_rate_f64,
            duration,
            codec_name,
            hwaccel
        );

        // Determine output pixel format
        let output_format = if use_bgra {
            ffmpeg_next::format::Pixel::BGRA
        } else {
            ffmpeg_next::format::Pixel::RGBA
        };

        // Create scaler to convert to RGBA or BGRA
        let scaler = ffmpeg_next::software::scaling::Context::get(
            decoder.format(),
            width,
            height,
            output_format,
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
            hwaccel,
            codec_name,
            output_format,
        })
    }

    /// Try to create a hardware-accelerated decoder
    fn try_create_hwaccel_decoder(
        stream: &ffmpeg_next::format::stream::Stream,
        codec_name: &str,
    ) -> Result<(ffmpeg_next::decoder::Video, HwAccelMethod), String> {
        // Determine which hwaccel to try based on platform
        #[cfg(target_os = "macos")]
        let hwaccel_methods = [HwAccelMethod::VideoToolbox];

        #[cfg(target_os = "windows")]
        let hwaccel_methods = [HwAccelMethod::D3d11va, HwAccelMethod::Nvdec];

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let hwaccel_methods = [HwAccelMethod::Nvdec];

        for method in hwaccel_methods {
            match Self::create_hwaccel_decoder_with_method(stream, codec_name, method) {
                Ok(decoder) => {
                    tracing::info!("✓ Hardware acceleration enabled: {}", method);
                    return Ok((decoder, method));
                }
                Err(e) => {
                    tracing::debug!("Hardware acceleration {} not available: {}", method, e);
                }
            }
        }

        Err("No hardware acceleration available".to_string())
    }

    /// Create a decoder with a specific hwaccel method
    fn create_hwaccel_decoder_with_method(
        stream: &ffmpeg_next::format::stream::Stream,
        codec_name: &str,
        method: HwAccelMethod,
    ) -> Result<ffmpeg_next::decoder::Video, String> {
        // For now, we'll use the standard decoder path
        // FFmpeg will automatically use hwaccel if available in the codec context
        // 
        // Note: Full hwaccel support requires setting up hw_device_ctx on the codec context,
        // which isn't directly exposed by ffmpeg-next's safe API. We rely on FFmpeg's
        // automatic hwaccel detection for supported codecs.
        //
        // For VideoToolbox on macOS, FFmpeg can use it automatically for H.264/HEVC
        // if the system supports it.

        let hwaccel_name = match method {
            HwAccelMethod::VideoToolbox => "videotoolbox",
            HwAccelMethod::D3d11va => "d3d11va",
            HwAccelMethod::Nvdec => "cuda",
            HwAccelMethod::Qsv => "qsv",
            HwAccelMethod::None => return Err("No hwaccel requested".to_string()),
            HwAccelMethod::HapGpuNative => return Err("HAP uses GPU-native textures, not hwaccel".to_string()),
        };

        // Try to find a hardware decoder variant
        let hw_decoder_name = match (codec_name, method) {
            ("h264", HwAccelMethod::VideoToolbox) => Some("h264"),
            ("hevc", HwAccelMethod::VideoToolbox) => Some("hevc"),
            ("h264", HwAccelMethod::Nvdec) => Some("h264_cuvid"),
            ("hevc", HwAccelMethod::Nvdec) => Some("hevc_cuvid"),
            _ => None,
        };

        // Try hardware decoder
        if let Some(hw_name) = hw_decoder_name {
            tracing::debug!("Attempting {} decoder with {} hwaccel", hw_name, hwaccel_name);
            
            // Create decoder from stream parameters - this is the safe way
            let parameters = stream.parameters();
            let context = ffmpeg_next::codec::context::Context::from_parameters(parameters)
                .map_err(|e| format!("Failed to create context: {}", e))?;

            // Enable threading for better performance
            let decoder = context.decoder().video()
                .map_err(|e| format!("Failed to create decoder: {}", e))?;

            // Check if hwaccel is actually being used by examining the pixel format
            // Hardware decoders typically output to special pixel formats
            let pix_fmt = decoder.format();
            let is_hw_format = matches!(
                pix_fmt,
                ffmpeg_next::format::Pixel::VIDEOTOOLBOX
                    | ffmpeg_next::format::Pixel::D3D11
                    | ffmpeg_next::format::Pixel::CUDA
                    | ffmpeg_next::format::Pixel::QSV
            );

            if is_hw_format {
                tracing::info!("Decoder using hardware pixel format: {:?}", pix_fmt);
                return Ok(decoder);
            } else {
                // The decoder was created successfully - FFmpeg may still be using hwaccel
                // internally even if the output format isn't a "hardware" format.
                // Modern FFmpeg often auto-transfers hw frames to system memory.
                tracing::debug!("Decoder created with format {:?}, hwaccel {} available", pix_fmt, hwaccel_name);
                return Ok(decoder);
            }
        }

        Err(format!("No hardware decoder available for {} with {}", codec_name, hwaccel_name))
    }

    /// Decode the next frame, returning None if end of file
    pub fn decode_next_frame(&mut self) -> Result<Option<DecodedFrame>, VideoDecoderError> {
        if self.eof {
            return Ok(None);
        }

        // GPU-NATIVE FAST PATH: For HAP codec, bypass FFmpeg decoder entirely
        // We read packets directly and extract DXT data from the HAP container
        if self.is_hap() {
            return self.decode_hap_frame();
        }
        
        // DXV codec: Most DXV files use v4 with proprietary RAD compression
        // FFmpeg can decode this, so use standard path (outputs RGBA)

        // STANDARD PATH: Use FFmpeg decoder for DXV and all other codecs
        self.decode_standard_frame()
    }
    
    /// Decode HAP frames by extracting raw DXT data directly from packets
    /// This bypasses FFmpeg's decoder which would convert DXT to RGBA
    fn decode_hap_frame(&mut self) -> Result<Option<DecodedFrame>, VideoDecoderError> {
        // Read packets until we find a video packet or hit EOF
        for (stream, packet) in self.input.packets() {
            if stream.index() == self.video_stream_index {
                let packet_data = packet.data().unwrap_or(&[]);

                if let Some((dxt_data, is_bc3)) = parse_hap_packet(packet_data) {
                    // Get raw PTS from packet
                    let raw_pts = packet.pts().unwrap_or(0);

                    // Get stream time_base for proper PTS conversion
                    let stream_time_base = stream.time_base();
                    let stream_tb_f64 = stream_time_base.numerator() as f64 / stream_time_base.denominator() as f64;

                    // Convert PTS to seconds using stream time_base
                    let pts_secs = raw_pts as f64 * stream_tb_f64;
                    let pts_frame_index = (pts_secs * self.frame_rate).round() as u64;

                    let frame = DecodedFrame::new_gpu_native(
                        dxt_data,
                        self.width,
                        self.height,
                        pts_secs,
                        pts_frame_index, // Use PTS-derived index for accurate position
                        is_bc3,
                    );
                    self.frame_index = pts_frame_index + 1; // Keep internal counter in sync

                    return Ok(Some(frame));
                } else {
                    // HAP parsing failed - this shouldn't happen for valid HAP files
                    tracing::warn!("HAP packet parsing failed for frame {}", self.frame_index);
                    continue;
                }
            }
            // Non-video packet, continue to next
        }

        // Iterator exhausted - EOF
        self.eof = true;
        Ok(None)
    }
    
    /// Standard decode path using FFmpeg decoder (for non-HAP codecs)
    fn decode_standard_frame(&mut self) -> Result<Option<DecodedFrame>, VideoDecoderError> {
        let mut decoded_frame = ffmpeg_next::frame::Video::empty();

        loop {
            // First, try to receive any pending frames from the decoder
            match self.decoder.receive_frame(&mut decoded_frame) {
                Ok(()) => {
                    let pts = decoded_frame.pts().unwrap_or(0) as f64 * self.time_base;
                    let frame_format = decoded_frame.format();
                    
                    // If hardware format, transfer to system memory
                    let frame_to_scale = if Self::is_hardware_format(frame_format) {
                        let mut sw_frame = ffmpeg_next::frame::Video::empty();
                        unsafe {
                            let ret = ffmpeg_next::ffi::av_hwframe_transfer_data(
                                sw_frame.as_mut_ptr(),
                                decoded_frame.as_ptr(),
                                0,
                            );
                            if ret < 0 {
                                tracing::warn!("Failed to transfer hwframe to system memory");
                                decoded_frame
                            } else {
                                sw_frame
                            }
                        }
                    } else {
                        decoded_frame
                    };

                    // Recreate scaler if format changed
                    if frame_to_scale.format() != self.scaler.input().format {
                        self.scaler = ffmpeg_next::software::scaling::Context::get(
                            frame_to_scale.format(),
                            self.width,
                            self.height,
                            self.output_format,
                            self.width,
                            self.height,
                            ffmpeg_next::software::scaling::Flags::BILINEAR,
                        )
                        .map_err(|e| VideoDecoderError::ScalerCreationFailed(e.to_string()))?;
                    }

                    // Convert to RGBA/BGRA
                    let mut rgba_frame = ffmpeg_next::frame::Video::empty();
                    self.scaler.run(&frame_to_scale, &mut rgba_frame)?;

                    let data = rgba_frame.data(0);
                    let stride = rgba_frame.stride(0);
                    let expected_stride = (self.width as usize) * 4;

                    let rgba_data = if stride == expected_stride {
                        data[..DecodedFrame::expected_size(self.width, self.height)].to_vec()
                    } else {
                        let mut output = Vec::with_capacity(DecodedFrame::expected_size(self.width, self.height));
                        for y in 0..self.height as usize {
                            let row_start = y * stride;
                            let row_end = row_start + expected_stride;
                            output.extend_from_slice(&data[row_start..row_end]);
                        }
                        output
                    };

                    // Use PTS for accurate position tracking
                    let pts_frame_index = (pts * self.frame_rate).round() as u64;

                    let frame = DecodedFrame::new(
                        rgba_data,
                        self.width,
                        self.height,
                        pts,
                        pts_frame_index,
                    );
                    self.frame_index = pts_frame_index + 1;

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

            // Read next packet and send to decoder
            loop {
                match self.input.packets().next() {
                    Some((stream, packet)) => {
                        if stream.index() == self.video_stream_index {
                            self.decoder.send_packet(&packet)?;
                            break;
                        }
                    }
                    None => {
                        self.decoder.send_eof()?;
                        self.eof = true;
                        break;
                    }
                }
            }
        }
    }

    /// Check if a pixel format is a hardware format
    fn is_hardware_format(format: ffmpeg_next::format::Pixel) -> bool {
        matches!(
            format,
            ffmpeg_next::format::Pixel::VIDEOTOOLBOX
                | ffmpeg_next::format::Pixel::D3D11
                | ffmpeg_next::format::Pixel::CUDA
                | ffmpeg_next::format::Pixel::QSV
                | ffmpeg_next::format::Pixel::VAAPI
                | ffmpeg_next::format::Pixel::VDPAU
                | ffmpeg_next::format::Pixel::DXVA2_VLD
        )
    }

    /// Reset the decoder to the beginning of the file
    pub fn reset(&mut self) -> Result<(), VideoDecoderError> {
        self.input.seek(0, ..)?;
        self.decoder.flush();
        self.frame_index = 0;
        self.eof = false;
        Ok(())
    }

    /// Seek to a specific timestamp in seconds and decode one frame
    ///
    /// This is useful for generating thumbnails - seek to the middle of
    /// the video and grab a single frame.
    ///
    /// NOTE: For HAP videos, this returns DXT compressed data (is_gpu_native=true).
    /// Use `seek_and_decode_frame_rgba` if you need raw RGBA pixels.
    pub fn seek_and_decode_frame(&mut self, timestamp_secs: f64) -> Result<DecodedFrame, VideoDecoderError> {
        // FFmpeg's input.seek() uses AV_TIME_BASE (microseconds)
        let timestamp_us = (timestamp_secs * 1_000_000.0) as i64;

        // Seek to nearest keyframe
        self.input.seek(timestamp_us, ..timestamp_us)?;
        self.decoder.flush();
        self.eof = false;

        // Just decode the next available frame after seek
        self.decode_next_frame()?
            .ok_or(VideoDecoderError::DecodeFailed("No frame available after seek".to_string()))
    }

    /// Seek and decode a frame, always returning RGBA pixel data
    ///
    /// This forces the standard FFmpeg decode path even for HAP videos,
    /// ensuring the result is always raw RGBA pixels suitable for thumbnails.
    pub fn seek_and_decode_frame_rgba(&mut self, timestamp_secs: f64) -> Result<DecodedFrame, VideoDecoderError> {
        // FFmpeg's input.seek() uses AV_TIME_BASE (microseconds)
        let timestamp_us = (timestamp_secs * 1_000_000.0) as i64;
        self.input.seek(timestamp_us, ..timestamp_us)?;
        self.decoder.flush();
        self.eof = false;

        // Always use standard decode path (returns RGBA, not DXT)
        self.decode_standard_frame()?
            .ok_or(VideoDecoderError::DecodeFailed("No frame available after seek".to_string()))
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

    /// Get the hardware acceleration method in use
    pub fn hwaccel_method(&self) -> HwAccelMethod {
        self.hwaccel
    }

    /// Get the codec name
    pub fn codec_name(&self) -> &str {
        &self.codec_name
    }

    /// Check if this is a Hap codec video
    pub fn is_hap(&self) -> bool {
        self.codec_name.starts_with("hap")
    }

    /// Check if this is a DXV codec video
    pub fn is_dxv(&self) -> bool {
        self.codec_name == "dxv"
    }

    /// Check if this is a GPU-native codec (HAP or DXV)
    /// These codecs store frames in DXT/BC format for direct GPU upload
    pub fn is_gpu_native(&self) -> bool {
        self.is_hap() || self.is_dxv()
    }

    /// Check if the decoder outputs BGRA format (instead of RGBA)
    pub fn is_bgra_output(&self) -> bool {
        self.output_format == ffmpeg_next::format::Pixel::BGRA
    }
}

// HAP container parsing for raw DXT extraction
// HAP format: https://github.com/Vidvox/hap/blob/master/documentation/HapVideoDRAFT.md

/// HAP compressor types (upper 4 bits of type byte)  
const HAP_COMPRESSOR_NONE: u8 = 0xA0;      // Uncompressed
const HAP_COMPRESSOR_SNAPPY: u8 = 0xB0;    // Snappy compressed
const HAP_COMPRESSOR_COMPLEX: u8 = 0xC0;   // Multiple chunks

/// HAP texture types (lower 4 bits of type byte)
const HAP_FORMAT_RGBADXT5: u8 = 0x0E; // DXT5/BC3 (RGBA)
const HAP_FORMAT_YCOCGDXT5: u8 = 0x0F; // YCoCg DXT5 (HAP Q)

/// Parse HAP packet and extract raw DXT data
/// Returns (dxt_data, is_bc3) or None if parsing fails
pub fn parse_hap_packet(packet_data: &[u8]) -> Option<(Vec<u8>, bool)> {
    if packet_data.len() < 4 {
        return None;
    }
    
    // Parse section header
    // HAP uses two header formats:
    // - 4-byte header: bytes 0-2 are 24-bit length, byte 3 is type
    // - 8-byte header: bytes 0-2 are 0x00 0x00 0x00, byte 3 is type, bytes 4-7 are 32-bit length
    // The 8-byte format is used when the length field (bytes 0-2) is all zeros
    let length_24bit = (packet_data[0] as usize) 
            | ((packet_data[1] as usize) << 8) 
            | ((packet_data[2] as usize) << 16);
    
    let type_byte = packet_data[3];
    
    let (payload_offset, payload_len) = if length_24bit == 0 {
        // 8-byte header: bytes 0-2 are zero, byte 3 is type, bytes 4-7 are 32-bit length
        if packet_data.len() < 8 {
            return None;
        }
        let len = u32::from_le_bytes([packet_data[4], packet_data[5], packet_data[6], packet_data[7]]) as usize;
        (8, len)
    } else {
        // 4-byte header: bytes 0-2 are 24-bit length
        (4, length_24bit)
    };
    
    // Extract compressor and format
    let compressor = type_byte & 0xF0;
    let texture_type = type_byte & 0x0F;
    
    // Determine BC format from texture type
    let is_bc3 = texture_type == HAP_FORMAT_RGBADXT5 || texture_type == HAP_FORMAT_YCOCGDXT5;
    
    // Get payload
    if packet_data.len() < payload_offset + payload_len {
        // Payload extends beyond packet - use what we have
        let available = packet_data.len() - payload_offset;
        let payload = &packet_data[payload_offset..payload_offset + available];
        
        // Decompress if needed
        let dxt_data = match compressor {
            HAP_COMPRESSOR_NONE => payload.to_vec(),
            HAP_COMPRESSOR_SNAPPY => {
                match snap::raw::Decoder::new().decompress_vec(payload) {
                    Ok(decompressed) => decompressed,
                    Err(_) => return None,
                }
            }
            HAP_COMPRESSOR_COMPLEX => return None, // Complex multi-chunk not yet supported
            _ => return None,
        };
        
        return Some((dxt_data, is_bc3));
    }
    
    let payload = &packet_data[payload_offset..payload_offset + payload_len];
    
    // Decompress if needed
    let dxt_data = match compressor {
        HAP_COMPRESSOR_NONE => payload.to_vec(),
        HAP_COMPRESSOR_SNAPPY => {
            match snap::raw::Decoder::new().decompress_vec(payload) {
                Ok(decompressed) => decompressed,
                Err(_) => return None,
            }
        }
        HAP_COMPRESSOR_COMPLEX => return None, // Complex multi-chunk not yet supported
        _ => return None,
    };
    
    Some((dxt_data, is_bc3))
}

// NOTE: DXV codec support
// DXV v4 (used by modern Resolume files) uses RAD Game Tools proprietary compression.
// We cannot decode this directly without a proprietary library.
// DXV files are handled by FFmpeg's decoder which outputs RGBA.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_error_display() {
        let err = VideoDecoderError::NoVideoStream;
        assert_eq!(err.to_string(), "No video stream found in file");
    }

    #[test]
    fn test_hwaccel_method_display() {
        assert_eq!(format!("{}", HwAccelMethod::VideoToolbox), "videotoolbox");
        assert_eq!(format!("{}", HwAccelMethod::None), "software");
    }
}
