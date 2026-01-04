//! Background-threaded video player
//!
//! Decodes video frames on a background thread while the main thread
//! picks up decoded frames for GPU upload, allowing UI to run at full
//! display refresh rate.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::{DecodedFrame, VideoDecoder, VideoDecoderError};

/// Shared state between decode thread and main thread
struct SharedState {
    /// The latest decoded frame (if any)
    current_frame: Mutex<Option<DecodedFrame>>,
    /// Whether a new frame is available for pickup
    new_frame_available: AtomicBool,
    /// Whether the player is running
    running: AtomicBool,
    /// Whether playback is paused
    paused: AtomicBool,
    /// Signal to restart from beginning
    restart_requested: AtomicBool,
    /// Signal to seek to a specific time
    seek_requested: AtomicBool,
    /// Target seek time in seconds (stored as bits for atomic ops)
    seek_target_bits: AtomicU64,
    /// Current frame index for tracking
    frame_index: AtomicU64,
    /// Loop mode: 0=Loop, 1=PlayOnce
    loop_mode: AtomicU8,
}

impl SharedState {
    fn new() -> Self {
        Self {
            current_frame: Mutex::new(None),
            new_frame_available: AtomicBool::new(false),
            running: AtomicBool::new(true),
            paused: AtomicBool::new(false),
            restart_requested: AtomicBool::new(false),
            seek_requested: AtomicBool::new(false),
            seek_target_bits: AtomicU64::new(0),
            frame_index: AtomicU64::new(0),
            loop_mode: AtomicU8::new(0), // Default: Loop
        }
    }
}

/// Video metadata available on the main thread
#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub frame_rate: f64,
    pub duration: f64,
    /// Whether this is a GPU-native codec (HAP/DXV)
    pub is_gpu_native: bool,
    /// For GPU-native codecs: true = BC3/DXT5, false = BC1/DXT1
    pub is_bc3: bool,
    /// Whether this is specifically a HAP codec (not DXV)
    pub is_hap: bool,
}

/// Background-threaded video player
///
/// Decodes video on a background thread at the video's native frame rate.
/// Main thread can pick up frames without blocking.
pub struct VideoPlayer {
    /// Shared state with decode thread
    state: Arc<SharedState>,
    /// Decode thread handle
    thread_handle: Option<JoinHandle<()>>,
    /// Video metadata (cached on main thread)
    info: VideoInfo,
    /// Path to the video file
    path: std::path::PathBuf,
}

impl VideoPlayer {
    /// Open a video file and start background decoding
    pub fn open(path: &Path) -> Result<Self, VideoDecoderError> {
        // Open decoder to get video info
        let decoder = VideoDecoder::open(path)?;
        
        // Determine BC3 vs BC1 for GPU-native codecs
        let is_gpu_native = decoder.is_gpu_native();
        let is_hap = decoder.is_hap();
        let codec_name = decoder.codec_name();
        let is_bc3 = codec_name.contains("alpha") || codec_name.contains("_q") || codec_name.contains("hapq");
        
        let info = VideoInfo {
            width: decoder.width(),
            height: decoder.height(),
            frame_rate: decoder.frame_rate(),
            duration: decoder.duration(),
            is_gpu_native,
            is_bc3,
            is_hap,
        };
        
        tracing::info!(
            "VideoPlayer: {}x{} @ {:.2}fps, duration: {:.2}s, gpu_native: {}, codec: {}",
            info.width, info.height, info.frame_rate, info.duration, is_gpu_native, codec_name
        );
        
        let state = Arc::new(SharedState::new());
        let state_clone = Arc::clone(&state);
        let path_clone = path.to_path_buf();
        
        // Start decode thread
        let thread_handle = thread::spawn(move || {
            Self::decode_loop(state_clone, path_clone);
        });
        
        Ok(Self {
            state,
            thread_handle: Some(thread_handle),
            info,
            path: path.to_path_buf(),
        })
    }
    
    /// Background decode loop
    fn decode_loop(state: Arc<SharedState>, path: std::path::PathBuf) {
        // Open decoder in this thread
        let mut decoder = match VideoDecoder::open(&path) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!("Failed to open video in decode thread: {}", e);
                return;
            }
        };

        let frame_duration = Duration::from_secs_f64(1.0 / decoder.frame_rate());
        let mut next_frame_time = Instant::now();

        // Decode first frame immediately
        if let Ok(Some(frame)) = decoder.decode_next_frame() {
            if let Ok(mut current) = state.current_frame.lock() {
                *current = Some(frame);
                state.new_frame_available.store(true, Ordering::Release);
            }
        }

        while state.running.load(Ordering::Acquire) {
            // Check for restart request
            if state.restart_requested.swap(false, Ordering::AcqRel) {
                if let Err(e) = decoder.reset() {
                    tracing::warn!("Failed to reset decoder: {}", e);
                }
                next_frame_time = Instant::now();
                state.frame_index.store(0, Ordering::Release);
                tracing::debug!("VideoPlayer: restarted");
            }

            // Check for seek request
            if state.seek_requested.swap(false, Ordering::AcqRel) {
                let target_bits = state.seek_target_bits.load(Ordering::Acquire);
                let target_secs = f64::from_bits(target_bits);

                match decoder.seek_and_decode_frame(target_secs) {
                    Ok(frame) => {
                        let frame_idx = frame.frame_index;
                        if let Ok(mut current) = state.current_frame.lock() {
                            *current = Some(frame);
                            state.new_frame_available.store(true, Ordering::Release);
                            state.frame_index.store(frame_idx, Ordering::Release);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to seek: {}", e);
                    }
                }
                next_frame_time = Instant::now();
            }

            // Check if paused
            if state.paused.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(10));
                next_frame_time = Instant::now();
                continue;
            }

            // Wait until next frame time (only if we're ahead of schedule)
            let now = Instant::now();
            if now < next_frame_time {
                let sleep_time = next_frame_time - now;
                if sleep_time > Duration::from_micros(500) {
                    thread::sleep(sleep_time - Duration::from_micros(500));
                }
                while Instant::now() < next_frame_time {
                    std::hint::spin_loop();
                }
            }

            // Decode next frame
            match decoder.decode_next_frame() {
                Ok(Some(frame)) => {
                    let frame_idx = frame.frame_index;

                    // Store frame for main thread pickup
                    if let Ok(mut current) = state.current_frame.lock() {
                        *current = Some(frame);
                        state.new_frame_available.store(true, Ordering::Release);
                        state.frame_index.store(frame_idx, Ordering::Release);
                    }
                }
                Ok(None) => {
                    // End of video - check loop mode
                    let mode = state.loop_mode.load(Ordering::Acquire);
                    if mode == 0 {
                        // Loop: restart from beginning
                        if let Err(e) = decoder.reset() {
                            tracing::warn!("Failed to reset decoder for loop: {}", e);
                        }
                        next_frame_time = Instant::now();
                        state.frame_index.store(0, Ordering::Release);
                    } else {
                        // PlayOnce: stay paused on last frame
                        state.paused.store(true, Ordering::Release);
                    }
                }
                Err(e) => {
                    tracing::error!("Decode error: {}", e);
                }
            }

            // Schedule next frame
            next_frame_time += frame_duration;

            // If we fell behind, reset to now (don't try to catch up)
            let now = Instant::now();
            if next_frame_time < now {
                next_frame_time = now;
            }
        }

        tracing::debug!("VideoPlayer decode thread stopped");
    }
    
    /// Take the latest decoded frame if available (non-blocking)
    ///
    /// Returns `Some(frame)` if a new frame is ready, `None` otherwise.
    /// This is very fast - just an atomic check and mutex lock.
    pub fn take_frame(&self) -> Option<DecodedFrame> {
        if self.state.new_frame_available.swap(false, Ordering::AcqRel) {
            if let Ok(mut current) = self.state.current_frame.lock() {
                return current.take();
            }
        }
        None
    }
    
    /// Check if a new frame is available (without taking it)
    pub fn has_new_frame(&self) -> bool {
        self.state.new_frame_available.load(Ordering::Acquire)
    }
    
    /// Pause playback
    pub fn pause(&self) {
        self.state.paused.store(true, Ordering::Release);
        tracing::info!("VideoPlayer: paused");
    }
    
    /// Resume playback
    pub fn resume(&self) {
        self.state.paused.store(false, Ordering::Release);
        tracing::info!("VideoPlayer: resumed");
    }
    
    /// Toggle pause state
    pub fn toggle_pause(&self) {
        let was_paused = self.state.paused.fetch_xor(true, Ordering::AcqRel);
        tracing::info!("VideoPlayer: {}", if was_paused { "resumed" } else { "paused" });
    }
    
    /// Check if paused
    pub fn is_paused(&self) -> bool {
        self.state.paused.load(Ordering::Acquire)
    }
    
    /// Restart from beginning
    pub fn restart(&self) {
        self.state.restart_requested.store(true, Ordering::Release);
    }

    /// Seek to a specific time in seconds
    pub fn seek(&self, time_secs: f64) {
        // Store target time as bits (atomic f64 workaround)
        let bits = time_secs.to_bits();
        self.state.seek_target_bits.store(bits, Ordering::Release);
        self.state.seek_requested.store(true, Ordering::Release);
    }

    /// Get current frame index
    pub fn frame_index(&self) -> u64 {
        self.state.frame_index.load(Ordering::Acquire)
    }
    
    /// Get video width
    pub fn width(&self) -> u32 {
        self.info.width
    }
    
    /// Get video height
    pub fn height(&self) -> u32 {
        self.info.height
    }
    
    /// Get video frame rate
    pub fn frame_rate(&self) -> f64 {
        self.info.frame_rate
    }
    
    /// Get video duration in seconds
    pub fn duration(&self) -> f64 {
        self.info.duration
    }
    
    /// Get video info
    pub fn info(&self) -> &VideoInfo {
        &self.info
    }
    
    /// Get path to video file
    pub fn path(&self) -> &Path {
        &self.path
    }
    
    /// Check if this is a GPU-native codec (HAP/DXV)
    pub fn is_gpu_native(&self) -> bool {
        self.info.is_gpu_native
    }
    
    /// For GPU-native codecs: true = BC3/DXT5, false = BC1/DXT1
    pub fn is_bc3(&self) -> bool {
        self.info.is_bc3
    }
    
    /// Check if this is specifically a HAP codec (not DXV)
    pub fn is_hap(&self) -> bool {
        self.info.is_hap
    }

    /// Set the loop mode (0=Loop, 1=PlayOnce)
    pub fn set_loop_mode(&self, mode: u8) {
        self.state.loop_mode.store(mode, Ordering::Release);
        tracing::debug!("VideoPlayer: set loop mode to {}", mode);
    }

    /// Get the current loop mode (0=Loop, 1=PlayOnce)
    pub fn loop_mode(&self) -> u8 {
        self.state.loop_mode.load(Ordering::Acquire)
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        // Signal thread to stop
        self.state.running.store(false, Ordering::Release);
        
        // Wake up thread if it's sleeping
        self.state.paused.store(false, Ordering::Release);
        
        // Wait for thread to finish
        if let Some(handle) = self.thread_handle.take() {
            if let Err(e) = handle.join() {
                tracing::warn!("Failed to join decode thread: {:?}", e);
            }
        }
    }
}




