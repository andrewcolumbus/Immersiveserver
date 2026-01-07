//! OMT (Open Media Transport) frame capture from GPU.
//!
//! This module handles capturing the compositor's environment texture
//! and preparing it for transmission over OMT.
//!
//! # Architecture
//!
//! Uses triple-buffered async capture to avoid blocking the render loop:
//! 1. Frame N: Copy texture to buffer A
//! 2. Frame N+1: Copy texture to buffer B, poll buffer A (non-blocking)
//! 3. Frame N+2: Copy texture to buffer C, read buffer A if ready, poll buffer B
//!
//! This allows the GPU and CPU to work in parallel without stalling.

use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bytes::Bytes;

use super::OmtSender;

/// Default target frame rate for OMT capture (30fps to reduce CPU load).
const DEFAULT_CAPTURE_FPS: u32 = 30;

/// Number of staging buffers for async capture pipeline.
const NUM_STAGING_BUFFERS: usize = 3;

/// A captured frame ready for transmission.
/// Uses `Bytes` for zero-copy sharing between threads.
#[derive(Debug, Clone)]
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    pub data: Bytes,
}

/// State of a staging buffer in the capture pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BufferState {
    /// Buffer is available for a new capture.
    Available,
    /// Buffer has been written to by GPU, waiting for map.
    Pending,
    /// Buffer map_async has been called, waiting for callback.
    Mapping,
    /// Buffer is mapped and ready to read.
    Ready,
}

/// A staging buffer with its state.
struct StagingBuffer {
    buffer: wgpu::Buffer,
    state: BufferState,
    /// Flag set by the async map callback when mapping completes.
    map_complete: Arc<AtomicBool>,
}

/// Handles GPU texture capture for OMT streaming.
///
/// Uses triple-buffered async capture to avoid blocking the render loop.
/// Frames are captured to rotating staging buffers and read back when ready.
pub struct OmtCapture {
    /// Triple-buffered staging buffers.
    staging_buffers: Vec<StagingBuffer>,
    /// Index of the next buffer to use for capture.
    next_capture_buffer: usize,
    /// Bytes per row (with padding for wgpu alignment).
    bytes_per_row: u32,
    /// Unpadded bytes per row (actual pixel data).
    unpadded_bytes_per_row: u32,

    /// Environment dimensions.
    width: u32,
    height: u32,

    /// Background sender thread.
    sender_thread: Option<JoinHandle<()>>,
    /// Channel to send frames to background thread.
    frame_tx: Option<mpsc::SyncSender<CapturedFrame>>,
    /// Flag to signal thread shutdown.
    shutdown_tx: Option<mpsc::Sender<()>>,

    /// Frame count for statistics.
    frame_count: u64,
    /// Frames skipped due to pipeline backup.
    frames_skipped: u64,

    /// Reusable buffer for unpacking row-padded data.
    /// Avoids per-frame heap allocation.
    unpack_buffer: Vec<u8>,

    /// Target capture frame rate.
    target_fps: u32,
    /// Minimum interval between captures.
    min_capture_interval: Duration,
    /// Last capture time for frame rate throttling.
    last_capture_time: Instant,
    /// Poll throttle counter - only poll GPU every N frames to reduce sync stalls.
    poll_counter: u32,
}

impl OmtCapture {
    /// Create a new OmtCapture for the given environment dimensions.
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let (staging_buffers, bytes_per_row, unpadded_bytes_per_row) =
            Self::create_staging_buffers(device, width, height);

        // Pre-allocate unpack buffer for row-padding removal
        let unpack_buffer = Vec::with_capacity((width * height * 4) as usize);

        let target_fps = DEFAULT_CAPTURE_FPS;
        let min_capture_interval = Duration::from_secs_f64(1.0 / target_fps as f64);

        Self {
            staging_buffers,
            next_capture_buffer: 0,
            bytes_per_row,
            unpadded_bytes_per_row,
            width,
            height,
            sender_thread: None,
            frame_tx: None,
            shutdown_tx: None,
            frame_count: 0,
            frames_skipped: 0,
            unpack_buffer,
            target_fps,
            min_capture_interval,
            last_capture_time: Instant::now(),
            poll_counter: 0,
        }
    }

    /// Create staging buffers for GPU readback.
    fn create_staging_buffers(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (Vec<StagingBuffer>, u32, u32) {
        // Calculate bytes per row with wgpu alignment requirements
        let unpadded_bytes_per_row = width * 4; // BGRA = 4 bytes per pixel
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;

        let buffer_size = (padded_bytes_per_row * height) as u64;

        let buffers: Vec<StagingBuffer> = (0..NUM_STAGING_BUFFERS)
            .map(|i| {
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("OMT Staging Buffer {}", i)),
                    size: buffer_size,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                StagingBuffer {
                    buffer,
                    state: BufferState::Available,
                    map_complete: Arc::new(AtomicBool::new(false)),
                }
            })
            .collect();

        (buffers, padded_bytes_per_row, unpadded_bytes_per_row)
    }

    /// Start the background sender thread.
    ///
    /// The thread receives captured frames and sends them via the OmtSender.
    /// Requires a tokio runtime handle for async network operations.
    pub fn start_sender_thread(
        &mut self,
        mut sender: OmtSender,
        runtime: tokio::runtime::Handle,
    ) {
        if self.sender_thread.is_some() {
            return;
        }

        let (frame_tx, frame_rx) = mpsc::sync_channel::<CapturedFrame>(8);
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

        let handle = thread::Builder::new()
            .name("omt-sender".into())
            .spawn(move || {
                tracing::info!("OMT: Sender thread started");

                loop {
                    // Check for shutdown signal (non-blocking)
                    if shutdown_rx.try_recv().is_ok() {
                        tracing::info!("OMT: Sender thread shutting down");
                        break;
                    }

                    // Wait for a frame (with timeout to check shutdown)
                    match frame_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                        Ok(frame) => {
                            // Use the runtime handle to execute async send
                            // frame.data is already Bytes, no copy needed
                            let result = runtime.block_on(async {
                                sender.send_frame_async(
                                    frame.width,
                                    frame.height,
                                    frame.data,
                                ).await
                            });
                            match &result {
                                Ok(()) => {
                                    let count = sender.frame_count();
                                    if count == 1 || count % 300 == 0 {
                                        tracing::info!("📡 OMT: Sent {} frames ({}x{})", count, frame.width, frame.height);
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("OMT: Failed to send frame: {}", e);
                                }
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            tracing::info!("OMT: Frame channel disconnected");
                            break;
                        }
                    }
                }

                sender.stop();
                tracing::info!("OMT: Sender thread stopped");
            })
            .expect("Failed to spawn OMT sender thread");

        self.sender_thread = Some(handle);
        self.frame_tx = Some(frame_tx);
        self.shutdown_tx = Some(shutdown_tx);
    }

    /// Stop the background sender thread.
    /// This is non-blocking - it signals the thread to stop but doesn't wait.
    pub fn stop_sender_thread(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        self.frame_tx = None;

        // Take the handle but don't join - let the thread cleanup on its own
        // This avoids blocking the main thread
        if let Some(handle) = self.sender_thread.take() {
            // Spawn a cleanup thread to join the sender thread in the background
            std::thread::spawn(move || {
                let _ = handle.join();
            });
        }
    }

    /// Queue a copy from the environment texture to a staging buffer.
    ///
    /// Call this after rendering to the environment but before queue.submit().
    /// Returns true if a capture was queued, false if skipped (throttled or pipeline full).
    pub fn capture_frame(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        env_texture: &wgpu::Texture,
    ) -> bool {
        // Frame rate throttling - skip if we're capturing too fast
        let now = Instant::now();
        if now.duration_since(self.last_capture_time) < self.min_capture_interval {
            return false; // Don't count as skipped, just throttled
        }

        // Adaptive skip: if 2+ buffers are pending, skip to reduce GPU pressure
        let pending_count = self.staging_buffers.iter()
            .filter(|b| b.state == BufferState::Pending)
            .count();
        if pending_count >= 2 {
            self.frames_skipped += 1;
            return false;
        }

        // Find an available buffer
        let buffer_index = self.next_capture_buffer;
        let staging = &mut self.staging_buffers[buffer_index];

        // Skip if buffer isn't available (pipeline is backed up)
        if staging.state != BufferState::Available {
            self.frames_skipped += 1;
            return false;
        }

        // Queue the copy
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: env_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        staging.state = BufferState::Pending;
        self.next_capture_buffer = (self.next_capture_buffer + 1) % NUM_STAGING_BUFFERS;
        self.last_capture_time = now;
        true
    }

    /// Set the target capture frame rate.
    pub fn set_target_fps(&mut self, fps: u32) {
        self.target_fps = fps.max(1).min(60);
        self.min_capture_interval = Duration::from_secs_f64(1.0 / self.target_fps as f64);
        tracing::info!("OMT: Capture target FPS set to {}", self.target_fps);
    }

    /// Get the current target capture frame rate.
    pub fn target_fps(&self) -> u32 {
        self.target_fps
    }

    /// Process the capture pipeline - call this each frame after queue.submit().
    ///
    /// This is NON-BLOCKING. It:
    /// 1. Polls GPU to make progress on pending work
    /// 2. Starts async map operations for pending buffers
    /// 3. Checks if any mapped buffers are ready to read
    /// 4. Reads ready buffers and sends frames to the background thread
    pub fn process(&mut self, device: &wgpu::Device) {
        // Throttle GPU polling to every 3rd frame to reduce main thread stalls.
        // This reduces GPU-CPU sync overhead by ~66% while still making progress.
        self.poll_counter = (self.poll_counter + 1) % 3;
        if self.poll_counter == 0 {
            device.poll(wgpu::Maintain::Poll);
        }

        // Process each buffer by index to avoid borrow issues
        for i in 0..self.staging_buffers.len() {
            let state = self.staging_buffers[i].state;

            match state {
                BufferState::Pending => {
                    // Start async map operation (only once)
                    let staging = &mut self.staging_buffers[i];
                    let map_complete = staging.map_complete.clone();
                    staging.buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| {
                        if result.is_ok() {
                            map_complete.store(true, Ordering::Release);
                        }
                    });
                    staging.state = BufferState::Mapping;
                }
                BufferState::Mapping => {
                    // Check if map completed
                    let staging = &mut self.staging_buffers[i];
                    if staging.map_complete.load(Ordering::Acquire) {
                        staging.state = BufferState::Ready;
                    }
                }
                BufferState::Ready => {
                    // Read the data
                    let staging = &self.staging_buffers[i];
                    let data = staging.buffer.slice(..).get_mapped_range();

                    // Remove row padding using reusable buffer (avoids per-frame allocation)
                    self.unpack_buffer.clear();
                    if self.bytes_per_row != self.unpadded_bytes_per_row {
                        for row in 0..self.height {
                            let start = (row * self.bytes_per_row) as usize;
                            let end = start + self.unpadded_bytes_per_row as usize;
                            self.unpack_buffer.extend_from_slice(&data[start..end]);
                        }
                    } else {
                        self.unpack_buffer.extend_from_slice(&data);
                    }
                    drop(data);

                    // Send to background thread (non-blocking)
                    // Bytes::copy_from_slice does one copy; Bytes is then zero-copy shared
                    if let Some(tx) = &self.frame_tx {
                        let frame = CapturedFrame {
                            width: self.width,
                            height: self.height,
                            data: Bytes::copy_from_slice(&self.unpack_buffer),
                        };
                        if tx.try_send(frame).is_err() {
                            tracing::debug!("OMT: Frame dropped (sender busy)");
                        } else {
                            self.frame_count += 1;
                        }
                    }

                    // Reset buffer state
                    let staging = &mut self.staging_buffers[i];
                    staging.buffer.unmap();
                    staging.map_complete.store(false, Ordering::Relaxed);
                    staging.state = BufferState::Available;
                }
                BufferState::Available => {}
            }
        }
    }

    /// Resize the capture buffers for a new environment size.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == self.width && height == self.height {
            return;
        }

        tracing::info!("OMT: Resizing capture buffers to {}x{}", width, height);

        // Unmap any mapped buffers first
        for staging in &mut self.staging_buffers {
            if staging.state == BufferState::Ready {
                staging.buffer.unmap();
            }
        }

        let (staging_buffers, bytes_per_row, unpadded_bytes_per_row) =
            Self::create_staging_buffers(device, width, height);

        self.staging_buffers = staging_buffers;
        self.bytes_per_row = bytes_per_row;
        self.unpadded_bytes_per_row = unpadded_bytes_per_row;
        self.width = width;
        self.height = height;
        self.next_capture_buffer = 0;

        // Reallocate unpack buffer for new dimensions
        self.unpack_buffer = Vec::with_capacity((width * height * 4) as usize);
    }

    /// Get the number of frames captured and sent.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Get the number of frames skipped due to pipeline backup.
    pub fn frames_skipped(&self) -> u64 {
        self.frames_skipped
    }

    /// Get the current dimensions.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Check if dimensions match the current capture size.
    pub fn dimensions_match(&self, width: u32, height: u32) -> bool {
        self.width == width && self.height == height
    }

    /// Check if the sender thread is running.
    pub fn is_sender_running(&self) -> bool {
        self.sender_thread.is_some()
    }
}

impl Drop for OmtCapture {
    fn drop(&mut self) {
        self.stop_sender_thread();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_bytes_per_row_alignment() {
        // Test that bytes_per_row calculation respects 256-byte alignment
        let width: u32 = 1920;
        let unpadded = width * 4; // 7680 bytes
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = unpadded.div_ceil(align) * align;

        // 7680 / 256 = 30, so no padding needed for 1920 width
        assert_eq!(padded, 7680);

        // Test odd width
        let width: u32 = 100;
        let unpadded = width * 4; // 400 bytes
        let padded = unpadded.div_ceil(align) * align;
        // 400 / 256 = 1.56, rounds up to 512
        assert_eq!(padded, 512);
    }
}
