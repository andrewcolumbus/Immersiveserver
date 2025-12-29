//! Aqueduct streaming output integration
//!
//! Provides streaming output via the Aqueduct protocol for network-based video distribution.

#![allow(dead_code)]

use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Aqueduct output sender wrapper
pub struct AqueductOutput {
    /// Sender for streaming frames
    sender: Option<Arc<Mutex<aqueduct::Sender>>>,
    /// Port number
    port: u16,
    /// Stream name
    name: String,
    /// Whether the sender is active
    active: bool,
    /// Tokio runtime handle for async operations
    runtime: Option<tokio::runtime::Handle>,
}

impl Default for AqueductOutput {
    fn default() -> Self {
        Self {
            sender: None,
            port: 9000,
            name: "Immersive Player".to_string(),
            active: false,
            runtime: None,
        }
    }
}

impl AqueductOutput {
    /// Create a new Aqueduct output with the given port
    pub fn new(name: String, port: u16) -> Self {
        Self {
            sender: None,
            port,
            name,
            active: false,
            runtime: tokio::runtime::Handle::try_current().ok(),
        }
    }

    /// Start the Aqueduct sender
    pub async fn start(&mut self) -> Result<(), aqueduct::AqueductError> {
        if self.sender.is_some() {
            return Ok(());
        }

        log::info!("Starting Aqueduct sender on port {} as '{}'", self.port, self.name);
        
        let sender = aqueduct::Sender::new(self.port).await?;
        self.sender = Some(Arc::new(Mutex::new(sender)));
        self.active = true;
        
        log::info!("Aqueduct sender started successfully");
        Ok(())
    }

    /// Stop the Aqueduct sender
    pub fn stop(&mut self) {
        if self.sender.is_some() {
            log::info!("Stopping Aqueduct sender");
            self.sender = None;
            self.active = false;
        }
    }

    /// Check if the sender is active
    pub fn is_active(&self) -> bool {
        self.active && self.sender.is_some()
    }

    /// Get the port number
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get the stream name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Send a video frame
    /// 
    /// The frame data should be in BGRA format matching the specified width and height.
    pub fn send_frame(
        &self,
        width: u32,
        height: u32,
        data: &[u8],
        timestamp: std::time::Duration,
    ) -> Result<(), aqueduct::AqueductError> {
        let Some(sender) = &self.sender else {
            return Err(aqueduct::AqueductError::Protocol("Sender not started".to_string()));
        };

        // Create video frame
        let frame = aqueduct::VideoFrame {
            width,
            height,
            format: aqueduct::PixelFormat::BGRA,
            flags: aqueduct::FrameFlags::default(),
            timestamp,
            data: Bytes::copy_from_slice(data),
        };

        // Send the frame (blocking - would need async in production)
        if let Some(rt) = &self.runtime {
            rt.block_on(async {
                let sender_lock = sender.lock().await;
                sender_lock.send(aqueduct::Packet::Video(frame))
            })
        } else {
            // Fallback: try to send synchronously
            // This requires a runtime, so we'll need to handle this case
            Err(aqueduct::AqueductError::Protocol("No tokio runtime available".to_string()))
        }
    }

    /// Send a video frame from a texture/buffer asynchronously
    pub async fn send_frame_async(
        &self,
        width: u32,
        height: u32,
        data: bytes::Bytes,
        timestamp: std::time::Duration,
    ) -> Result<(), aqueduct::AqueductError> {
        let Some(sender) = &self.sender else {
            return Err(aqueduct::AqueductError::Protocol("Sender not started".to_string()));
        };

        let frame = aqueduct::VideoFrame {
            width,
            height,
            format: aqueduct::PixelFormat::BGRA,
            flags: aqueduct::FrameFlags::default(),
            timestamp,
            data,
        };

        let sender_lock = sender.lock().await;
        sender_lock.send(aqueduct::Packet::Video(frame))
    }
}

impl Clone for AqueductOutput {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            port: self.port,
            name: self.name.clone(),
            active: self.active,
            runtime: self.runtime.clone(),
        }
    }
}

/// Manager for multiple Aqueduct outputs
#[derive(Default)]
pub struct AqueductManager {
    /// Active outputs by screen ID
    outputs: std::collections::HashMap<u32, AqueductOutput>,
}

impl AqueductManager {
    pub fn new() -> Self {
        Self {
            outputs: std::collections::HashMap::new(),
        }
    }

    /// Create or get an output for a screen
    pub fn get_or_create(&mut self, screen_id: u32, name: String, port: u16) -> &mut AqueductOutput {
        self.outputs
            .entry(screen_id)
            .or_insert_with(|| AqueductOutput::new(name, port))
    }

    /// Remove an output
    pub fn remove(&mut self, screen_id: u32) {
        if let Some(mut output) = self.outputs.remove(&screen_id) {
            output.stop();
        }
    }

    /// Stop all outputs
    pub fn stop_all(&mut self) {
        for output in self.outputs.values_mut() {
            output.stop();
        }
    }

    /// Get an active output
    pub fn get(&self, screen_id: u32) -> Option<&AqueductOutput> {
        self.outputs.get(&screen_id)
    }

    /// Get an active output mutably
    pub fn get_mut(&mut self, screen_id: u32) -> Option<&mut AqueductOutput> {
        self.outputs.get_mut(&screen_id)
    }
}

