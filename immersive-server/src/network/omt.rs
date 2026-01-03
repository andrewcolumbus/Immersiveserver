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
//! │  • Connects to OMT sources                                  │
//! │  • Receives video frames                                    │
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

use super::omt_ffi::LibOmtSender;
use bytes::Bytes;
use std::time::{Duration, Instant};

/// Video frame received from OMT stream.
#[derive(Debug, Clone)]
pub struct OmtFrame {
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Raw pixel data (BGRA format)
    pub data: Bytes,
    /// Timestamp from stream start
    pub timestamp: Duration,
}

/// OMT receiver for receiving video streams from network sources.
///
/// NOTE: Receiver functionality is not yet implemented with libOMT.
/// This is a placeholder for future implementation.
pub struct OmtReceiver {
    /// Currently connected source address
    connected_source: Option<String>,
    /// Running flag
    running: bool,
    /// Frame count for statistics
    frame_count: u64,
    /// Start time for latency calculation
    start_time: Option<Instant>,
}

impl OmtReceiver {
    /// Create a new OMT receiver (not yet connected).
    pub fn new() -> Self {
        Self {
            connected_source: None,
            running: false,
            frame_count: 0,
            start_time: None,
        }
    }

    /// Connect to an OMT source at the given address.
    ///
    /// NOTE: Not yet implemented with libOMT.
    pub async fn connect(&mut self, address: &str) -> Result<(), OmtError> {
        tracing::warn!("OMT Receiver: libOMT receiver not yet implemented");
        self.connected_source = Some(address.to_string());
        Err(OmtError::NotImplemented)
    }

    /// Disconnect from the current source.
    pub fn disconnect(&mut self) {
        if self.connected_source.is_some() {
            tracing::info!("OMT Receiver: Disconnecting from {:?}", self.connected_source);
        }
        self.connected_source = None;
        self.running = false;
    }

    /// Check if connected to a source.
    pub fn is_connected(&self) -> bool {
        self.running
    }

    /// Get the currently connected source address.
    pub fn connected_source(&self) -> Option<&str> {
        self.connected_source.as_deref()
    }

    /// Receive the next video frame from the stream.
    pub async fn receive_frame(&mut self) -> Option<OmtFrame> {
        // Not implemented yet
        None
    }

    /// Get the number of frames received.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Get the average FPS since connection.
    pub fn average_fps(&self) -> f64 {
        if let Some(start) = self.start_time {
            let elapsed = start.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                return self.frame_count as f64 / elapsed;
            }
        }
        0.0
    }
}

impl Default for OmtReceiver {
    fn default() -> Self {
        Self::new()
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
    fn test_omt_receiver_new() {
        let receiver = OmtReceiver::new();
        assert!(!receiver.is_connected());
        assert!(receiver.connected_source().is_none());
        assert_eq!(receiver.frame_count(), 0);
    }

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
}
