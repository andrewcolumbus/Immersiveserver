//! OMT (Open Media Transport) integration via official libOMT.
//!
//! This module provides OMT sending/receiving using the official libOMT C library
//! from IntoPix, ensuring compatibility with OBS and other OMT-enabled applications.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     OmtReceiver                              │
//! │  • Connects to OMT sources via background thread            │
//! │  • Receives video frames with ring buffer                   │
//! │  • Delivers to compositor layer                             │
//! └─────────────────────────────────────────────────────────────┘
//!
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     OmtSender                                │
//! │  • Captures compositor output                               │
//! │  • Encodes with VMX1 codec via libOMT                       │
//! │  • Automatic mDNS discovery registration                    │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use super::omt_ffi::{LibOmtReceiver, LibOmtSender};
use bytes::Bytes;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Video frame received from OMT stream.
#[derive(Debug, Clone)]
pub struct OmtFrame {
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
    /// When the frame was received (for latency calculation).
    pub received_at: Instant,
    /// Whether the data is BGRA format (true) or needs R↔B swap (false).
    pub is_bgra: bool,
}

/// Default ring buffer capacity for OMT receiver.
const DEFAULT_OMT_BUFFER_CAPACITY: usize = 3;

/// Shared state between main thread and receiver thread.
struct OmtReceiverState {
    /// Ring buffer for received frames (FIFO).
    frame_buffer: Mutex<VecDeque<OmtFrame>>,
    /// Flag to signal thread to stop.
    running: AtomicBool,
    /// Connection status (set true on first frame).
    connected: AtomicBool,
    /// Total frames received.
    frame_count: AtomicU64,
    /// Frames dropped due to buffer overflow.
    frames_dropped: AtomicU64,
    /// Last pickup latency in microseconds.
    last_pickup_latency_us: AtomicU64,
    /// Current queue depth.
    last_queue_depth: AtomicUsize,
    /// Buffer capacity (configurable).
    buffer_capacity: AtomicUsize,
}

impl OmtReceiverState {
    fn new() -> Self {
        Self {
            frame_buffer: Mutex::new(VecDeque::with_capacity(DEFAULT_OMT_BUFFER_CAPACITY)),
            running: AtomicBool::new(true),
            connected: AtomicBool::new(false),
            frame_count: AtomicU64::new(0),
            frames_dropped: AtomicU64::new(0),
            last_pickup_latency_us: AtomicU64::new(0),
            last_queue_depth: AtomicUsize::new(0),
            buffer_capacity: AtomicUsize::new(DEFAULT_OMT_BUFFER_CAPACITY),
        }
    }
}

/// OMT receiver for receiving video streams from network sources.
///
/// Uses a background thread to receive frames from libOMT and delivers them
/// via a ring buffer to the main render thread.
pub struct OmtReceiver {
    /// Shared state with background thread.
    state: Arc<OmtReceiverState>,
    /// Handle to the receive thread.
    thread_handle: Option<JoinHandle<()>>,
    /// Source address (discovery format or URL).
    source_address: String,
    /// Cached frame width.
    width: u32,
    /// Cached frame height.
    height: u32,
    /// Connection start time.
    start_time: Instant,
}

impl OmtReceiver {
    /// Connect to an OMT source at the given address.
    ///
    /// Address can be either:
    /// - Discovery format: "HOSTNAME (NAME)"
    /// - Direct URL: "omt://hostname:port"
    ///
    /// Spawns a background thread to receive frames.
    pub fn connect(address: &str) -> Result<Self, OmtError> {
        tracing::info!("📡 OMT Receiver: Connecting to '{}'", address);

        let state = Arc::new(OmtReceiverState::new());
        let state_clone = Arc::clone(&state);
        let address_clone = address.to_string();

        // Spawn background receive thread
        let thread_handle = thread::Builder::new()
            .name("omt-receiver".into())
            .spawn(move || {
                Self::receive_loop(state_clone, &address_clone);
            })
            .map_err(|e| OmtError::Creation(format!("Failed to spawn receiver thread: {}", e)))?;

        Ok(Self {
            state,
            thread_handle: Some(thread_handle),
            source_address: address.to_string(),
            width: 0,
            height: 0,
            start_time: Instant::now(),
        })
    }

    /// Background receive loop - runs on separate thread.
    fn receive_loop(state: Arc<OmtReceiverState>, address: &str) {
        tracing::info!("📡 OMT receiver thread started for '{}'", address);

        // Create libOMT receiver
        let receiver = match LibOmtReceiver::new(address) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("📡 OMT receiver failed to create: {}", e);
                return;
            }
        };

        let start_time = Instant::now();

        // Main receive loop
        while state.running.load(Ordering::Acquire) {
            // Receive frame with 100ms timeout
            if let Some(frame_ref) = receiver.receive_frame(100) {
                // Mark as connected on first frame
                if !state.connected.swap(true, Ordering::AcqRel) {
                    tracing::info!(
                        "📡 OMT receiver connected: {}x{} @ {:.1}fps",
                        frame_ref.width,
                        frame_ref.height,
                        if frame_ref.frame_rate_d > 0 {
                            frame_ref.frame_rate_n as f64 / frame_ref.frame_rate_d as f64
                        } else {
                            60.0
                        }
                    );
                }

                // Copy frame data (must be done before next omt_receive call)
                let data = unsafe { frame_ref.copy_data() };

                let omt_frame = OmtFrame {
                    width: frame_ref.width,
                    height: frame_ref.height,
                    data: Bytes::from(data),
                    timestamp: start_time.elapsed(),
                    frame_rate: if frame_ref.frame_rate_d > 0 {
                        frame_ref.frame_rate_n as f64 / frame_ref.frame_rate_d as f64
                    } else {
                        60.0
                    },
                    received_at: Instant::now(),
                    is_bgra: frame_ref.is_bgra,
                };

                // Store in ring buffer
                if let Ok(mut buffer) = state.frame_buffer.lock() {
                    let capacity = state.buffer_capacity.load(Ordering::Relaxed);
                    if buffer.len() >= capacity {
                        // Drop oldest frame
                        buffer.pop_front();
                        state.frames_dropped.fetch_add(1, Ordering::Relaxed);
                    }
                    buffer.push_back(omt_frame);
                }

                state.frame_count.fetch_add(1, Ordering::Relaxed);
            }
            // Timeout - continue loop (will check running flag)
        }

        tracing::info!("📡 OMT receiver thread stopping for '{}'", address);
        // LibOmtReceiver::drop() called here, which calls omt_receive_destroy()
    }

    /// Take the next frame from the buffer (non-blocking).
    ///
    /// Returns None if no frame is available.
    pub fn take_frame(&mut self) -> Option<OmtFrame> {
        if let Ok(mut buffer) = self.state.frame_buffer.lock() {
            if let Some(frame) = buffer.pop_front() {
                // Update cached dimensions
                self.width = frame.width;
                self.height = frame.height;

                // Store pickup latency for UI
                let pickup_latency_us = frame.received_at.elapsed().as_micros() as u64;
                let remaining = buffer.len();
                self.state
                    .last_pickup_latency_us
                    .store(pickup_latency_us, Ordering::Release);
                self.state
                    .last_queue_depth
                    .store(remaining, Ordering::Release);

                return Some(frame);
            }
        }
        None
    }

    /// Check if connected to the source (received at least one frame).
    pub fn is_connected(&self) -> bool {
        self.state.connected.load(Ordering::Acquire)
    }

    /// Get the source address.
    pub fn source_address(&self) -> &str {
        &self.source_address
    }

    /// Get the current frame width.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get the current frame height.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get total frames received.
    pub fn frame_count(&self) -> u64 {
        self.state.frame_count.load(Ordering::Relaxed)
    }

    /// Get frames dropped due to buffer overflow.
    pub fn frames_dropped(&self) -> u64 {
        self.state.frames_dropped.load(Ordering::Relaxed)
    }

    /// Get average FPS since connection.
    pub fn average_fps(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.frame_count() as f64 / elapsed
        } else {
            0.0
        }
    }

    /// Get the last pickup latency in milliseconds.
    pub fn pickup_latency_ms(&self) -> f64 {
        self.state.last_pickup_latency_us.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Get the current queue depth (frames waiting in buffer).
    pub fn queue_depth(&self) -> usize {
        self.state.last_queue_depth.load(Ordering::Relaxed)
    }

    /// Get the buffer capacity.
    pub fn buffer_capacity(&self) -> usize {
        self.state.buffer_capacity.load(Ordering::Relaxed)
    }

    /// Set the buffer capacity.
    pub fn set_buffer_capacity(&mut self, capacity: usize) {
        self.state
            .buffer_capacity
            .store(capacity.max(1), Ordering::Relaxed);
    }
}

impl Drop for OmtReceiver {
    fn drop(&mut self) {
        // Signal thread to stop
        self.state.running.store(false, Ordering::Release);

        // Wait for thread to finish
        if let Some(handle) = self.thread_handle.take() {
            tracing::info!(
                "📡 OMT Receiver: Waiting for thread to stop (source: {})",
                self.source_address
            );
            // Give the thread a moment to notice the flag
            thread::sleep(Duration::from_millis(150));
            // Try to join, but don't block forever
            let _ = handle.join();
            tracing::info!("📡 OMT Receiver: Thread stopped");
        }
    }
}

/// OMT error type.
#[derive(Debug, Clone)]
pub enum OmtError {
    /// Failed to create sender/receiver
    Creation(String),
    /// Failed to send frame
    Send(String),
    /// Feature not yet implemented
    NotImplemented,
    /// I/O error
    Io(String),
}

impl std::fmt::Display for OmtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OmtError::Creation(msg) => write!(f, "OMT creation error: {}", msg),
            OmtError::Send(msg) => write!(f, "OMT send error: {}", msg),
            OmtError::NotImplemented => write!(f, "OMT feature not implemented"),
            OmtError::Io(msg) => write!(f, "IO Error: {}", msg),
        }
    }
}

impl std::error::Error for OmtError {}

/// OMT sender for transmitting video streams to the network.
///
/// Uses the official libOMT library for OBS compatibility.
/// The sender automatically registers with mDNS discovery.
pub struct OmtSender {
    /// libOMT sender wrapper
    sender: Option<LibOmtSender>,
    /// Stream name
    name: String,
    /// Running flag
    running: bool,
    /// Frame count for statistics
    frame_count: u64,
    /// Start time
    start_time: Option<Instant>,
    /// Frame rate (for frame metadata)
    frame_rate: u32,
}

// OmtSender uses LibOmtSender which is Send
unsafe impl Send for OmtSender {}

impl OmtSender {
    /// Create a new OMT sender with the given name.
    ///
    /// Note: The port parameter is ignored - libOMT uses auto port allocation
    /// in the range 6400-6600 (configurable via settings).
    pub fn new(name: String, _port: u16) -> Self {
        Self {
            sender: None,
            name,
            running: false,
            frame_count: 0,
            start_time: None,
            frame_rate: 60,
        }
    }

    /// Start the OMT sender.
    ///
    /// libOMT handles mDNS registration automatically.
    pub async fn start(&mut self) -> Result<(), OmtError> {
        if self.sender.is_some() {
            return Ok(()); // Already started
        }

        tracing::info!("libOMT Sender: Starting as '{}'", self.name);

        match LibOmtSender::new(&self.name) {
            Ok(sender) => {
                if let Some(addr) = sender.get_address() {
                    tracing::info!("libOMT Sender: Registered as '{}'", addr);
                }
                self.sender = Some(sender);
                self.running = true;
                self.frame_count = 0;
                self.start_time = Some(Instant::now());
                tracing::info!("libOMT Sender: Started successfully");
                Ok(())
            }
            Err(e) => {
                tracing::error!("libOMT Sender: Failed to create: {}", e);
                Err(OmtError::Creation(e))
            }
        }
    }

    /// Stop the OMT sender.
    pub fn stop(&mut self) {
        if self.sender.is_some() {
            tracing::info!("libOMT Sender: Stopping");
            self.sender = None;
            self.running = false;
        }
    }

    /// Check if the sender is running.
    pub fn is_running(&self) -> bool {
        self.running && self.sender.is_some()
    }

    /// Get the port number (returns 0 since libOMT handles port allocation).
    pub fn port(&self) -> u16 {
        0 // libOMT handles port allocation internally
    }

    /// Get the stream name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Send a video frame synchronously.
    ///
    /// The data should be in BGRA format with dimensions width × height.
    pub fn send_frame(
        &mut self,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Result<(), OmtError> {
        let sender = self.sender.as_mut().ok_or_else(|| {
            OmtError::Send("Sender not started".to_string())
        })?;

        sender.send_frame(width, height, self.frame_rate, data)
            .map_err(OmtError::Send)?;

        self.frame_count += 1;
        Ok(())
    }

    /// Send a video frame asynchronously.
    ///
    /// Note: libOMT send is synchronous, but we keep the async signature
    /// for API compatibility.
    pub async fn send_frame_async(
        &mut self,
        width: u32,
        height: u32,
        data: Bytes,
    ) -> Result<(), OmtError> {
        self.send_frame(width, height, &data)
    }

    /// Get the number of frames sent.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Get the number of connected receivers.
    pub fn connection_count(&self) -> i32 {
        self.sender.as_ref().map(|s| s.connection_count()).unwrap_or(0)
    }

    /// Set the frame rate for outgoing frames.
    pub fn set_frame_rate(&mut self, fps: u32) {
        self.frame_rate = fps;
    }
}

impl Default for OmtSender {
    fn default() -> Self {
        Self::new("Immersive Server".to_string(), 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_omt_sender_new() {
        let sender = OmtSender::new("Test".to_string(), 9001);
        assert!(!sender.is_running());
        assert_eq!(sender.name(), "Test");
    }

    #[test]
    fn test_omt_error_display() {
        let err = OmtError::Creation("test".to_string());
        assert!(err.to_string().contains("creation"));
    }

    #[test]
    fn test_omt_frame_fields() {
        let frame = OmtFrame {
            width: 1920,
            height: 1080,
            data: Bytes::from(vec![0u8; 1920 * 1080 * 4]),
            timestamp: Duration::from_secs(1),
            frame_rate: 60.0,
            received_at: Instant::now(),
            is_bgra: true,
        };
        assert_eq!(frame.width, 1920);
        assert_eq!(frame.height, 1080);
        assert!(frame.is_bgra);
    }
}
