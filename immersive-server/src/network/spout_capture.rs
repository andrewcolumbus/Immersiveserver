//! GPU texture capture for Spout (Windows).
//!
//! This module handles capturing the compositor's environment texture
//! and preparing it for transmission via Spout.
//!
//! Uses triple-buffered async capture similar to OmtCapture to avoid
//! blocking the render loop.

#![cfg(target_os = "windows")]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::spout_ffi::{SpoutLibrary, DxgiFormat, GL_BGRA};

/// Default target frame rate for Spout capture (30fps to reduce CPU load).
const DEFAULT_CAPTURE_FPS: u32 = 30;

/// Number of staging buffers for async capture pipeline.
const NUM_STAGING_BUFFERS: usize = 3;

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

/// Handles GPU texture capture for Spout streaming.
///
/// Uses triple-buffered async capture to avoid blocking the render loop.
/// Frames are captured to rotating staging buffers and read back when ready.
pub struct SpoutCapture {
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

    /// Spout library handle.
    spout: Option<SpoutLibrary>,
    /// Whether Spout sender is active.
    active: bool,
    /// Sender name.
    name: String,

    /// Frame count for statistics.
    frame_count: u64,
    /// Frames skipped due to pipeline backup.
    frames_skipped: u64,

    /// Reusable buffer for unpacking row-padded data.
    unpack_buffer: Vec<u8>,

    /// Target capture frame rate.
    target_fps: u32,
    /// Minimum interval between captures.
    min_capture_interval: Duration,
    /// Last capture time for frame rate throttling.
    last_capture_time: Instant,
    /// Poll throttle counter.
    poll_counter: u32,
}

impl SpoutCapture {
    /// Create a new SpoutCapture for the given environment dimensions.
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let (staging_buffers, bytes_per_row, unpadded_bytes_per_row) =
            Self::create_staging_buffers(device, width, height);

        // Pre-allocate unpack buffer
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
            spout: None,
            active: false,
            name: String::new(),
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
        let unpadded_bytes_per_row = width * 4; // BGRA = 4 bytes per pixel
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;

        let buffer_size = (padded_bytes_per_row * height) as u64;

        let buffers: Vec<StagingBuffer> = (0..NUM_STAGING_BUFFERS)
            .map(|i| {
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("Spout Staging Buffer {}", i)),
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

    /// Start the Spout sender with the given name.
    pub fn start(&mut self, name: &str) -> Result<(), String> {
        if self.active {
            return Ok(());
        }

        // Initialize Spout library
        let spout = SpoutLibrary::new()?;

        // Set sender name and format
        spout.set_sender_name(name);
        spout.set_sender_format(DxgiFormat::B8G8R8A8Unorm);

        self.spout = Some(spout);
        self.name = name.to_string();
        self.active = true;

        log::info!(
            "Spout: Started capture as '{}' ({}x{}) @ {}fps",
            name,
            self.width,
            self.height,
            self.target_fps
        );

        Ok(())
    }

    /// Stop the Spout sender.
    pub fn stop(&mut self) {
        if self.active {
            if let Some(ref spout) = self.spout {
                spout.release_sender();
            }
            log::info!(
                "Spout: Stopped capture (sent {} frames, skipped {})",
                self.frame_count,
                self.frames_skipped
            );
        }
        self.spout = None;
        self.active = false;
        self.name.clear();
    }

    /// Check if sender is active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get the sender name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Queue a copy from the environment texture to a staging buffer.
    ///
    /// Call this after rendering to the environment but before queue.submit().
    pub fn capture_frame(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        env_texture: &wgpu::Texture,
    ) -> bool {
        if !self.active {
            return false;
        }

        // Frame rate throttling
        let now = Instant::now();
        if now.duration_since(self.last_capture_time) < self.min_capture_interval {
            return false;
        }

        // Adaptive skip: if 2+ buffers are pending, skip
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

    /// Process the capture pipeline - call this each frame after queue.submit().
    ///
    /// This is NON-BLOCKING. It processes buffers and sends ready frames to Spout.
    pub fn process(&mut self, device: &wgpu::Device) {
        if !self.active {
            return;
        }

        // Throttle GPU polling
        self.poll_counter = (self.poll_counter + 1) % 3;
        if self.poll_counter == 0 {
            device.poll(wgpu::Maintain::Poll);
        }

        // Process each buffer
        for i in 0..self.staging_buffers.len() {
            let state = self.staging_buffers[i].state;

            match state {
                BufferState::Pending => {
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
                    let staging = &mut self.staging_buffers[i];
                    if staging.map_complete.load(Ordering::Acquire) {
                        staging.state = BufferState::Ready;
                    }
                }
                BufferState::Ready => {
                    // Read the data and send via Spout
                    self.read_and_send(i);
                }
                BufferState::Available => {}
            }
        }
    }

    /// Read data from a ready buffer and send via Spout.
    fn read_and_send(&mut self, buffer_index: usize) {
        let staging = &self.staging_buffers[buffer_index];
        let data = staging.buffer.slice(..).get_mapped_range();

        // Remove row padding
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

        // Send via Spout
        if let Some(ref spout) = self.spout {
            // SendImage with invert=true to flip the image
            if spout.send_image(&self.unpack_buffer, self.width, self.height, GL_BGRA, true) {
                self.frame_count += 1;
                if self.frame_count == 1 || self.frame_count % 300 == 0 {
                    log::info!(
                        "📺 Spout: Sent {} frames ({}x{})",
                        self.frame_count,
                        self.width,
                        self.height
                    );
                }
            }
        }

        // Unmap and reset buffer state
        let staging = &mut self.staging_buffers[buffer_index];
        staging.buffer.unmap();
        staging.map_complete.store(false, Ordering::Release);
        staging.state = BufferState::Available;
    }

    /// Resize the capture buffers for new dimensions.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }

        // Wait for all buffers to be available
        for staging in &mut self.staging_buffers {
            if staging.state != BufferState::Available {
                // Force unmap if needed
                if staging.state == BufferState::Ready {
                    staging.buffer.unmap();
                }
                staging.state = BufferState::Available;
                staging.map_complete.store(false, Ordering::Release);
            }
        }

        // Recreate buffers
        let (staging_buffers, bytes_per_row, unpadded_bytes_per_row) =
            Self::create_staging_buffers(device, width, height);

        self.staging_buffers = staging_buffers;
        self.bytes_per_row = bytes_per_row;
        self.unpadded_bytes_per_row = unpadded_bytes_per_row;
        self.width = width;
        self.height = height;
        self.next_capture_buffer = 0;

        // Resize unpack buffer
        self.unpack_buffer = Vec::with_capacity((width * height * 4) as usize);

        log::info!("Spout: Resized capture to {}x{}", width, height);
    }

    /// Set target capture frame rate.
    pub fn set_target_fps(&mut self, fps: u32) {
        self.target_fps = fps.max(1).min(60);
        self.min_capture_interval = Duration::from_secs_f64(1.0 / self.target_fps as f64);
    }

    /// Get frame count.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
}

impl Drop for SpoutCapture {
    fn drop(&mut self) {
        self.stop();
    }
}
