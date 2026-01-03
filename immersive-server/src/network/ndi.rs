//! NDI (Network Device Interface) integration via official NDI SDK.
//!
//! This module provides NDI sending/receiving using the official NDI SDK
//! from Vizrt, enabling high-quality, low-latency video streaming over
//! standard IP networks.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     NdiReceiver                              │
//! │  • Discovers and connects to NDI sources                    │
//! │  • Receives video frames on background thread               │
//! │  • Lock-free frame delivery to main thread                  │
//! └─────────────────────────────────────────────────────────────┘
//!
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     NdiSender                                │
//! │  • Captures compositor output                               │
//! │  • Transmits as NDI stream                                  │
//! │  • Automatic network discovery registration                 │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use super::ndi_ffi::*;
use bytes::Bytes;
use std::ffi::{CStr, CString};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

// =============================================================================
// NdiFrame
// =============================================================================

/// Video frame received from NDI stream.
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

// =============================================================================
// NdiError
// =============================================================================

/// NDI error type.
#[derive(Debug, Clone)]
pub enum NdiError {
    /// NDI library not initialized.
    NotInitialized,
    /// Failed to create sender/receiver/finder.
    Creation(String),
    /// Failed to connect to source.
    Connection(String),
    /// Failed to send frame.
    Send(String),
    /// Invalid source name.
    InvalidName,
    /// Data size mismatch.
    DataSize,
}

impl std::fmt::Display for NdiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NdiError::NotInitialized => write!(f, "NDI library not initialized"),
            NdiError::Creation(msg) => write!(f, "NDI creation error: {}", msg),
            NdiError::Connection(msg) => write!(f, "NDI connection error: {}", msg),
            NdiError::Send(msg) => write!(f, "NDI send error: {}", msg),
            NdiError::InvalidName => write!(f, "Invalid NDI source name"),
            NdiError::DataSize => write!(f, "Data size mismatch"),
        }
    }
}

impl std::error::Error for NdiError {}

// =============================================================================
// NdiReceiver
// =============================================================================

/// Shared state between receive thread and main thread.
struct NdiReceiverState {
    /// The latest received frame (if any).
    current_frame: Mutex<Option<NdiFrame>>,
    /// Whether a new frame is available for pickup.
    new_frame_available: AtomicBool,
    /// Whether the receiver is running.
    running: AtomicBool,
    /// Whether connected to a source.
    connected: AtomicBool,
    /// Frame count for statistics.
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

/// NDI receiver for receiving video streams from NDI sources.
///
/// Uses a background thread to receive frames without blocking the main thread.
/// Follows the same pattern as VideoPlayer for lock-free frame delivery.
pub struct NdiReceiver {
    /// Shared state with receive thread.
    state: Arc<NdiReceiverState>,
    /// Receive thread handle.
    thread_handle: Option<JoinHandle<()>>,
    /// NDI source name (format: "MACHINE (SOURCE)").
    source_name: String,
    /// Video width (updated when first frame received).
    width: u32,
    /// Video height (updated when first frame received).
    height: u32,
    /// Start time for timing calculations.
    start_time: Instant,
}

// NdiReceiver uses thread-safe primitives
unsafe impl Send for NdiReceiver {}

impl NdiReceiver {
    /// Connect to an NDI source by name.
    ///
    /// The `ndi_name` should be in the format "MACHINE_NAME (SOURCE_NAME)"
    /// as returned by NDI discovery.
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

    /// Background receive loop.
    fn receive_loop(state: Arc<NdiReceiverState>, ndi_name: &str) {
        // Create C strings for NDI API
        let c_name = match CString::new(ndi_name) {
            Ok(s) => s,
            Err(e) => {
                log::error!("NDI Receiver: Invalid source name: {}", e);
                return;
            }
        };

        let recv_name = CString::new("Immersive Server").unwrap();

        // Create source descriptor
        let source = NDIlib_source_t {
            p_ndi_name: c_name.as_ptr(),
            p_url_address: std::ptr::null(),
        };

        // Create receiver with BGRA color format
        let create_settings = NDIlib_recv_create_v3_t {
            source_to_connect_to: source,
            color_format: NDIlib_recv_color_format_e::BGRX_BGRA,
            bandwidth: NDIlib_recv_bandwidth_e::Highest,
            allow_video_fields: false, // Always progressive
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

        // Receive loop
        while state.running.load(Ordering::Acquire) {
            let mut video_frame = NDIlib_video_frame_v2_t::default();

            let frame_type = unsafe {
                NDIlib_recv_capture_v2(
                    receiver,
                    &mut video_frame,
                    std::ptr::null_mut(), // No audio
                    std::ptr::null_mut(), // No metadata
                    100, // 100ms timeout
                )
            };

            match frame_type {
                NDIlib_frame_type_e::Video => {
                    // Mark as connected on first video frame
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

                    // Calculate data size and copy frame data
                    let stride = if video_frame.line_stride_in_bytes > 0 {
                        video_frame.line_stride_in_bytes as usize
                    } else {
                        video_frame.xres as usize * 4
                    };
                    let data_size = stride * video_frame.yres as usize;

                    // Copy data from NDI buffer
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

                    // Store for main thread pickup
                    if let Ok(mut current) = state.current_frame.lock() {
                        *current = Some(ndi_frame);
                        state.new_frame_available.store(true, Ordering::Release);
                        state.frame_count.fetch_add(1, Ordering::Relaxed);
                    }

                    // Free NDI's buffer
                    unsafe { NDIlib_recv_free_video_v2(receiver, &video_frame) };
                }
                NDIlib_frame_type_e::Error => {
                    if state.connected.swap(false, Ordering::AcqRel) {
                        log::warn!("NDI Receiver: Connection lost to '{}'", ndi_name);
                    }
                    // Keep trying to reconnect
                    thread::sleep(Duration::from_millis(100));
                }
                NDIlib_frame_type_e::None => {
                    // No frame available, continue
                }
                NDIlib_frame_type_e::StatusChange => {
                    log::debug!("NDI Receiver: Status change from '{}'", ndi_name);
                }
                _ => {}
            }
        }

        // Cleanup
        unsafe { NDIlib_recv_destroy(receiver) };
        log::info!("NDI Receiver: Stopped receiving from '{}'", ndi_name);
    }

    /// Take the latest frame (non-blocking).
    ///
    /// Returns None if no new frame is available.
    pub fn take_frame(&mut self) -> Option<NdiFrame> {
        if self.state.new_frame_available.swap(false, Ordering::AcqRel) {
            if let Ok(mut current) = self.state.current_frame.lock() {
                if let Some(frame) = current.take() {
                    // Update cached dimensions
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

    /// Get video width (0 if no frame received yet).
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get video height (0 if no frame received yet).
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
        // Signal thread to stop
        self.state.running.store(false, Ordering::Release);

        // Wait for thread to finish
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }

        log::info!("NDI Receiver: Dropped receiver for '{}'", self.source_name);
    }
}

// =============================================================================
// NdiSender
// =============================================================================

/// NDI sender for transmitting video streams to the network.
///
/// Creates an NDI source that can be discovered and received by other
/// NDI-compatible applications on the network.
pub struct NdiSender {
    /// NDI sender instance handle.
    sender: NDIlib_send_instance_t,
    /// Stream name.
    name: String,
    /// Frame count for statistics.
    frame_count: u64,
    /// Frame rate for timing.
    frame_rate: u32,
    /// Start time for statistics.
    start_time: Instant,
}

// NdiSender FFI handle is thread-safe for sending
unsafe impl Send for NdiSender {}

impl NdiSender {
    /// Create a new NDI sender with the given name.
    ///
    /// The sender will be automatically registered on the network
    /// and discoverable by NDI receivers.
    pub fn new(name: &str) -> Result<Self, NdiError> {
        let c_name = CString::new(name).map_err(|_| NdiError::InvalidName)?;

        let create_settings = NDIlib_send_create_t {
            p_ndi_name: c_name.as_ptr(),
            p_groups: std::ptr::null(),
            clock_video: false, // Don't block - we handle frame rate throttling ourselves
            clock_audio: false,
        };

        let sender = unsafe { NDIlib_send_create(&create_settings) };
        if sender.is_null() {
            return Err(NdiError::Creation("Failed to create NDI sender".into()));
        }

        log::info!("NDI Sender: Created sender '{}'", name);

        Ok(Self {
            sender,
            name: name.to_string(),
            frame_count: 0,
            frame_rate: 60,
            start_time: Instant::now(),
        })
    }

    /// Send a BGRA video frame.
    ///
    /// The data should be in BGRA format with dimensions width × height.
    pub fn send_frame(&mut self, width: u32, height: u32, bgra_data: &[u8]) -> Result<(), NdiError> {
        let expected_len = (width * height * 4) as usize;
        if bgra_data.len() != expected_len {
            return Err(NdiError::DataSize);
        }

        let video_frame = NDIlib_video_frame_v2_t {
            xres: width as i32,
            yres: height as i32,
            FourCC: NDIlib_FourCC_video_type_e::BGRA,
            frame_rate_N: self.frame_rate as i32,
            frame_rate_D: 1,
            picture_aspect_ratio: width as f32 / height as f32,
            frame_format_type: NDIlib_frame_format_type_e::Progressive,
            timecode: NDILIB_SEND_TIMECODE_SYNTHESIZE,
            p_data: bgra_data.as_ptr() as *mut u8,
            line_stride_in_bytes: (width * 4) as i32,
            p_metadata: std::ptr::null(),
            timestamp: 0,
        };

        unsafe { NDIlib_send_send_video_v2(self.sender, &video_frame) };
        self.frame_count += 1;

        Ok(())
    }

    /// Send a BGRA video frame asynchronously.
    ///
    /// The caller must ensure the data buffer remains valid until the next
    /// send call or until the sender is destroyed.
    pub fn send_frame_async(&mut self, width: u32, height: u32, bgra_data: &[u8]) -> Result<(), NdiError> {
        let expected_len = (width * height * 4) as usize;
        if bgra_data.len() != expected_len {
            return Err(NdiError::DataSize);
        }

        let video_frame = NDIlib_video_frame_v2_t {
            xres: width as i32,
            yres: height as i32,
            FourCC: NDIlib_FourCC_video_type_e::BGRA,
            frame_rate_N: self.frame_rate as i32,
            frame_rate_D: 1,
            picture_aspect_ratio: width as f32 / height as f32,
            frame_format_type: NDIlib_frame_format_type_e::Progressive,
            timecode: NDILIB_SEND_TIMECODE_SYNTHESIZE,
            p_data: bgra_data.as_ptr() as *mut u8,
            line_stride_in_bytes: (width * 4) as i32,
            p_metadata: std::ptr::null(),
            timestamp: 0,
        };

        unsafe { NDIlib_send_send_video_async_v2(self.sender, &video_frame) };
        self.frame_count += 1;

        Ok(())
    }

    /// Get the sender name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the number of frames sent.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Get the number of connected receivers.
    pub fn connection_count(&self) -> i32 {
        unsafe { NDIlib_send_get_no_connections(self.sender, 0) }
    }

    /// Set the frame rate for outgoing frames.
    pub fn set_frame_rate(&mut self, fps: u32) {
        self.frame_rate = fps;
    }

    /// Get the average FPS since creation.
    pub fn average_fps(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.frame_count as f64 / elapsed
        } else {
            0.0
        }
    }

    /// Get the NDI source info (for logging).
    pub fn get_source_info(&self) -> Option<String> {
        let source_ptr = unsafe { NDIlib_send_get_source_name(self.sender) };
        if source_ptr.is_null() {
            return None;
        }

        let source = unsafe { &*source_ptr };
        if source.p_ndi_name.is_null() {
            return None;
        }

        unsafe { CStr::from_ptr(source.p_ndi_name) }
            .to_str()
            .ok()
            .map(|s| s.to_string())
    }
}

impl Drop for NdiSender {
    fn drop(&mut self) {
        log::info!("NDI Sender: Destroying sender '{}'", self.name);
        unsafe { NDIlib_send_destroy(self.sender) };
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ndi_error_display() {
        let err = NdiError::Creation("test".to_string());
        assert!(err.to_string().contains("creation"));

        let err = NdiError::NotInitialized;
        assert!(err.to_string().contains("not initialized"));
    }

    #[test]
    fn test_ndi_frame_clone() {
        let frame = NdiFrame {
            width: 1920,
            height: 1080,
            data: Bytes::from_static(&[0, 1, 2, 3]),
            timestamp: Duration::from_secs(1),
            frame_rate: 60.0,
        };

        let cloned = frame.clone();
        assert_eq!(cloned.width, 1920);
        assert_eq!(cloned.height, 1080);
        assert_eq!(cloned.frame_rate, 60.0);
    }
}
