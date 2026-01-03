//! NDI camera input for projection mapping calibration.
//!
//! Provides NdiReceiver for receiving camera feeds over NDI.

use super::ndi_ffi::*;
use bytes::Bytes;
use std::ffi::CString;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Video frame received from NDI camera.
#[derive(Debug, Clone)]
pub struct NdiFrame {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Raw pixel data (BGRA format).
    pub data: Bytes,
    /// Timestamp from stream start.
    pub timestamp: Duration,
    /// Frame rate (frames per second).
    pub frame_rate: f64,
}

/// NDI error type.
#[derive(Debug, Clone)]
pub enum NdiError {
    NotInitialized,
    Creation(String),
    Connection(String),
    InvalidName,
}

impl std::fmt::Display for NdiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NdiError::NotInitialized => write!(f, "NDI library not initialized"),
            NdiError::Creation(msg) => write!(f, "NDI creation error: {}", msg),
            NdiError::Connection(msg) => write!(f, "NDI connection error: {}", msg),
            NdiError::InvalidName => write!(f, "Invalid NDI source name"),
        }
    }
}

impl std::error::Error for NdiError {}

/// Shared state between receive thread and main thread.
struct NdiReceiverState {
    current_frame: Mutex<Option<NdiFrame>>,
    new_frame_available: AtomicBool,
    running: AtomicBool,
    connected: AtomicBool,
    frame_count: AtomicU64,
}

impl NdiReceiverState {
    fn new() -> Self {
        Self {
            current_frame: Mutex::new(None),
            new_frame_available: AtomicBool::new(false),
            running: AtomicBool::new(true),
            connected: AtomicBool::new(false),
            frame_count: AtomicU64::new(0),
        }
    }
}

/// NDI receiver for camera input.
pub struct NdiReceiver {
    state: Arc<NdiReceiverState>,
    thread_handle: Option<JoinHandle<()>>,
    source_name: String,
    width: u32,
    height: u32,
    start_time: Instant,
}

unsafe impl Send for NdiReceiver {}

impl NdiReceiver {
    /// Connect to an NDI source by name.
    pub fn connect(ndi_name: &str) -> Result<Self, NdiError> {
        let state = Arc::new(NdiReceiverState::new());
        let source_name = ndi_name.to_string();

        let state_clone = Arc::clone(&state);
        let name_clone = source_name.clone();

        let thread_handle = thread::spawn(move || {
            Self::receive_loop(state_clone, &name_clone);
        });

        log::info!("NDI Receiver: Connecting to '{}'", source_name);

        Ok(Self {
            state,
            thread_handle: Some(thread_handle),
            source_name,
            width: 0,
            height: 0,
            start_time: Instant::now(),
        })
    }

    fn receive_loop(state: Arc<NdiReceiverState>, ndi_name: &str) {
        let c_name = match CString::new(ndi_name) {
            Ok(s) => s,
            Err(e) => {
                log::error!("NDI Receiver: Invalid source name: {}", e);
                return;
            }
        };

        let recv_name = CString::new("ProjMap Calibrator").unwrap();

        let source = NDIlib_source_t {
            p_ndi_name: c_name.as_ptr(),
            p_url_address: std::ptr::null(),
        };

        let create_settings = NDIlib_recv_create_v3_t {
            source_to_connect_to: source,
            color_format: NDIlib_recv_color_format_e::BGRX_BGRA,
            bandwidth: NDIlib_recv_bandwidth_e::Highest,
            allow_video_fields: false,
            p_ndi_recv_name: recv_name.as_ptr(),
        };

        let receiver = unsafe { NDIlib_recv_create_v3(&create_settings) };
        if receiver.is_null() {
            log::error!("NDI Receiver: Failed to create receiver for '{}'", ndi_name);
            return;
        }

        log::info!("NDI Receiver: Created receiver for '{}'", ndi_name);

        let start_time = Instant::now();
        let mut first_frame_logged = false;

        while state.running.load(Ordering::Acquire) {
            let mut video_frame = NDIlib_video_frame_v2_t::default();

            let frame_type = unsafe {
                NDIlib_recv_capture_v2(
                    receiver,
                    &mut video_frame,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    100,
                )
            };

            match frame_type {
                NDIlib_frame_type_e::Video => {
                    if !state.connected.load(Ordering::Acquire) {
                        state.connected.store(true, Ordering::Release);
                        log::info!("NDI Receiver: Connected to '{}'", ndi_name);
                    }

                    if !first_frame_logged {
                        log::info!(
                            "NDI Receiver: First frame {}x{} @ {:.2}fps from '{}'",
                            video_frame.xres,
                            video_frame.yres,
                            video_frame.frame_rate_N as f64 / video_frame.frame_rate_D.max(1) as f64,
                            ndi_name
                        );
                        first_frame_logged = true;
                    }

                    let stride = if video_frame.line_stride_in_bytes > 0 {
                        video_frame.line_stride_in_bytes as usize
                    } else {
                        video_frame.xres as usize * 4
                    };
                    let data_size = stride * video_frame.yres as usize;

                    let data = if !video_frame.p_data.is_null() && data_size > 0 {
                        let slice = unsafe {
                            std::slice::from_raw_parts(video_frame.p_data, data_size)
                        };
                        Bytes::copy_from_slice(slice)
                    } else {
                        Bytes::new()
                    };

                    let frame_rate = if video_frame.frame_rate_D > 0 {
                        video_frame.frame_rate_N as f64 / video_frame.frame_rate_D as f64
                    } else {
                        60.0
                    };

                    let ndi_frame = NdiFrame {
                        width: video_frame.xres as u32,
                        height: video_frame.yres as u32,
                        data,
                        timestamp: start_time.elapsed(),
                        frame_rate,
                    };

                    if let Ok(mut current) = state.current_frame.lock() {
                        *current = Some(ndi_frame);
                        state.new_frame_available.store(true, Ordering::Release);
                        state.frame_count.fetch_add(1, Ordering::Relaxed);
                    }

                    unsafe { NDIlib_recv_free_video_v2(receiver, &video_frame) };
                }
                NDIlib_frame_type_e::Error => {
                    if state.connected.swap(false, Ordering::AcqRel) {
                        log::warn!("NDI Receiver: Connection lost to '{}'", ndi_name);
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                NDIlib_frame_type_e::None => {}
                NDIlib_frame_type_e::StatusChange => {
                    log::debug!("NDI Receiver: Status change from '{}'", ndi_name);
                }
                _ => {}
            }
        }

        unsafe { NDIlib_recv_destroy(receiver) };
        log::info!("NDI Receiver: Stopped receiving from '{}'", ndi_name);
    }

    /// Take the latest frame (non-blocking).
    pub fn take_frame(&mut self) -> Option<NdiFrame> {
        if self.state.new_frame_available.swap(false, Ordering::AcqRel) {
            if let Ok(mut current) = self.state.current_frame.lock() {
                if let Some(frame) = current.take() {
                    self.width = frame.width;
                    self.height = frame.height;
                    return Some(frame);
                }
            }
        }
        None
    }

    /// Check if connected to the source.
    pub fn is_connected(&self) -> bool {
        self.state.connected.load(Ordering::Acquire)
    }

    /// Get the source name.
    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    /// Get video width.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get video height.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get the number of frames received.
    pub fn frame_count(&self) -> u64 {
        self.state.frame_count.load(Ordering::Acquire)
    }

    /// Get the average FPS since connection.
    pub fn average_fps(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.frame_count() as f64 / elapsed
        } else {
            0.0
        }
    }
}

impl Drop for NdiReceiver {
    fn drop(&mut self) {
        self.state.running.store(false, Ordering::Release);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
        log::info!("NDI Receiver: Dropped receiver for '{}'", self.source_name);
    }
}

/// NDI source discovery.
pub struct NdiFinder {
    finder: NDIlib_find_instance_t,
}

impl NdiFinder {
    /// Create a new NDI finder.
    pub fn new() -> Option<Self> {
        let settings = NDIlib_find_create_t::default();
        let finder = unsafe { NDIlib_find_create_v2(&settings) };
        if finder.is_null() {
            return None;
        }
        Some(Self { finder })
    }

    /// Get current list of discovered NDI sources.
    pub fn get_sources(&self) -> Vec<String> {
        let mut count: u32 = 0;
        let sources_ptr = unsafe { NDIlib_find_get_current_sources(self.finder, &mut count) };

        if sources_ptr.is_null() || count == 0 {
            return Vec::new();
        }

        let sources = unsafe { std::slice::from_raw_parts(sources_ptr, count as usize) };
        sources
            .iter()
            .filter_map(|s| {
                if s.p_ndi_name.is_null() {
                    None
                } else {
                    unsafe { std::ffi::CStr::from_ptr(s.p_ndi_name) }
                        .to_str()
                        .ok()
                        .map(|s| s.to_string())
                }
            })
            .collect()
    }

    /// Wait for sources to change.
    pub fn wait_for_sources(&self, timeout_ms: u32) -> bool {
        unsafe { NDIlib_find_wait_for_sources(self.finder, timeout_ms) }
    }
}

impl Drop for NdiFinder {
    fn drop(&mut self) {
        unsafe { NDIlib_find_destroy(self.finder) };
    }
}
