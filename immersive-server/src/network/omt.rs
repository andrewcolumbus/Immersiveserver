//! OMT (Open Media Transport) integration via Aqueduct.
//!
//! Aqueduct is a Rust-native implementation of the OMT protocol,
//! providing low-latency video, audio, and metadata streaming over IP.
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
//! │  • Encodes and transmits via OMT                           │
//! │  • Registers with mDNS discovery                            │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use aqueduct::{
    Discovery, Packet, PixelFormat, Receiver, QuicReceiver, Sender, VideoFrame,
    AqueductError, FrameFlags,
};
use bytes::Bytes;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

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

/// Internal receiver types
pub enum OmtReceiverType {
    Tcp(Receiver),
    Quic(QuicReceiver),
}

/// OMT receiver for receiving video streams from network sources.
pub struct OmtReceiver {
    /// Internal Aqueduct receiver
    receiver: Option<OmtReceiverType>,
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
            receiver: None,
            connected_source: None,
            running: false,
            frame_count: 0,
            start_time: None,
        }
    }

    /// Connect to an OMT source at the given address (host:port) using TCP.
    pub async fn connect(&mut self, address: &str) -> Result<(), AqueductError> {
        log::info!("OMT Receiver: Connecting to {} via TCP", address);
        
        let receiver = Receiver::connect(address).await?;
        self.receiver = Some(OmtReceiverType::Tcp(receiver));
        self.connected_source = Some(address.to_string());
        self.running = true;
        self.frame_count = 0;
        self.start_time = Some(Instant::now());
        
        log::info!("OMT Receiver: Connected successfully to {} via TCP", address);
        Ok(())
    }

    /// Connect to an OMT source at the given address (host:port) using QUIC.
    pub async fn connect_quic(&mut self, address: &str) -> Result<(), AqueductError> {
        log::info!("OMT Receiver: Connecting to {} via QUIC", address);
        
        let receiver = QuicReceiver::connect(address).await?;
        self.receiver = Some(OmtReceiverType::Quic(receiver));
        self.connected_source = Some(address.to_string());
        self.running = true;
        self.frame_count = 0;
        self.start_time = Some(Instant::now());
        
        log::info!("OMT Receiver: Connected successfully to {} via QUIC", address);
        Ok(())
    }

    /// Disconnect from the current source.
    pub fn disconnect(&mut self) {
        if self.connected_source.is_some() {
            log::info!("OMT Receiver: Disconnecting from {:?}", self.connected_source);
        }
        self.receiver = None;
        self.connected_source = None;
        self.running = false;
    }

    /// Check if connected to a source.
    pub fn is_connected(&self) -> bool {
        self.receiver.is_some() && self.running
    }

    /// Get the currently connected source address.
    pub fn connected_source(&self) -> Option<&str> {
        self.connected_source.as_deref()
    }

    /// Receive the next video frame from the stream.
    ///
    /// Returns `None` if not connected or on error.
    pub async fn receive_frame(&mut self) -> Option<OmtFrame> {
        let receiver_type = self.receiver.as_mut()?;
        
        loop {
            let result = match receiver_type {
                OmtReceiverType::Tcp(r) => r.receive().await,
                OmtReceiverType::Quic(r) => r.receive().await,
            };

            match result {
                Ok(Packet::Video(frame)) => {
                    self.frame_count += 1;
                    
                    // Convert BGRA or handle other formats
                    let data = if frame.format == PixelFormat::BGRA {
                        frame.data
                    } else {
                        log::warn!(
                            "OMT Receiver: Unsupported pixel format {:?}, skipping frame",
                            frame.format
                        );
                        continue;
                    };
                    
                    return Some(OmtFrame {
                        width: frame.width,
                        height: frame.height,
                        data,
                        timestamp: frame.timestamp,
                    });
                }
                Ok(Packet::Audio(_)) => {
                    // Audio frames are received but not processed yet
                    // Future: pipe to audio output
                    continue;
                }
                Ok(Packet::Metadata(meta)) => {
                    log::debug!("OMT Receiver: Metadata: {}", meta.content);
                    continue;
                }
                Err(e) => {
                    log::error!("OMT Receiver: Error receiving frame: {}", e);
                    self.running = false;
                    return None;
                }
            }
        }
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

/// OMT sender for transmitting video streams to the network.
pub struct OmtSender {
    /// Internal Aqueduct sender
    sender: Option<Arc<Mutex<Sender>>>,
    /// Discovery service for mDNS registration
    discovery: Option<Discovery>,
    /// Port number
    port: u16,
    /// Stream name
    name: String,
    /// Running flag
    running: bool,
    /// Frame count for statistics
    frame_count: u64,
    /// Start time
    start_time: Option<Instant>,
    /// Tokio runtime handle
    runtime: Option<tokio::runtime::Handle>,
}

impl OmtSender {
    /// Create a new OMT sender with the given name and port.
    pub fn new(name: String, port: u16) -> Self {
        Self {
            sender: None,
            discovery: None,
            port,
            name,
            running: false,
            frame_count: 0,
            start_time: None,
            runtime: tokio::runtime::Handle::try_current().ok(),
        }
    }

    /// Start the OMT sender and register with discovery.
    pub async fn start(&mut self) -> Result<(), AqueductError> {
        if self.sender.is_some() {
            return Ok(()); // Already started
        }

        log::info!("OMT Sender: Starting on port {} as '{}'", self.port, self.name);

        // Create sender
        let sender = Sender::new(self.port).await?;
        self.sender = Some(Arc::new(Mutex::new(sender)));

        // Register with mDNS discovery
        match Discovery::new() {
            Ok(discovery) => {
                if let Err(e) = discovery.register_source(
                    &hostname::get().unwrap_or_default().to_string_lossy(),
                    &self.name,
                    self.port,
                ) {
                    log::warn!("OMT Sender: Failed to register with discovery: {}", e);
                } else {
                    log::info!("OMT Sender: Registered with mDNS discovery");
                }
                self.discovery = Some(discovery);
            }
            Err(e) => {
                log::warn!("OMT Sender: Discovery service unavailable: {}", e);
            }
        }

        self.running = true;
        self.frame_count = 0;
        self.start_time = Some(Instant::now());

        log::info!("OMT Sender: Started successfully");
        Ok(())
    }

    /// Stop the OMT sender.
    pub fn stop(&mut self) {
        if self.sender.is_some() {
            log::info!("OMT Sender: Stopping");
            self.sender = None;
            self.discovery = None;
            self.running = false;
        }
    }

    /// Check if the sender is running.
    pub fn is_running(&self) -> bool {
        self.running && self.sender.is_some()
    }

    /// Get the port number.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get the stream name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Send a video frame.
    ///
    /// The data should be in BGRA format with dimensions width × height.
    pub fn send_frame(
        &mut self,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Result<(), AqueductError> {
        let sender = self.sender.as_ref().ok_or_else(|| {
            AqueductError::Protocol("Sender not started".to_string())
        })?;

        let timestamp = self.start_time
            .map(|s| s.elapsed())
            .unwrap_or(Duration::ZERO);

        let frame = VideoFrame {
            width,
            height,
            format: PixelFormat::BGRA,
            flags: FrameFlags::default(),
            timestamp,
            data: Bytes::copy_from_slice(data),
        };

        // Send via runtime (blocking from sync context)
        if let Some(rt) = &self.runtime {
            rt.block_on(async {
                let sender_lock = sender.lock().await;
                sender_lock.send(Packet::Video(frame))
            })?;
        } else {
            return Err(AqueductError::Protocol("No tokio runtime available".to_string()));
        }

        self.frame_count += 1;
        Ok(())
    }

    /// Send a video frame asynchronously.
    pub async fn send_frame_async(
        &mut self,
        width: u32,
        height: u32,
        data: Bytes,
    ) -> Result<(), AqueductError> {
        let sender = self.sender.as_ref().ok_or_else(|| {
            AqueductError::Protocol("Sender not started".to_string())
        })?;

        let timestamp = self.start_time
            .map(|s| s.elapsed())
            .unwrap_or(Duration::ZERO);

        let frame = VideoFrame {
            width,
            height,
            format: PixelFormat::BGRA,
            flags: FrameFlags::default(),
            timestamp,
            data,
        };

        let sender_lock = sender.lock().await;
        sender_lock.send(Packet::Video(frame))?;

        self.frame_count += 1;
        Ok(())
    }

    /// Get the number of frames sent.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
}

impl Default for OmtSender {
    fn default() -> Self {
        Self::new("Immersive Server".to_string(), 9000)
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
        assert_eq!(sender.port(), 9001);
        assert_eq!(sender.name(), "Test");
    }
}

