//! Camera capture module
//!
//! Provides cross-platform camera capture using the nokhwa crate.
//! Captures frames on a background thread and provides the latest frame
//! to the main render thread.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use nokhwa::pixel_format::RgbAFormat;
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
use nokhwa::Camera;
use parking_lot::Mutex;

/// Camera frame data
#[derive(Clone)]
pub struct CameraFrame {
    /// RGBA pixel data
    pub data: Vec<u8>,
    /// Frame width
    pub width: u32,
    /// Frame height
    pub height: u32,
    /// Frame number
    pub frame_number: u64,
    /// Frame timestamp
    pub timestamp: Instant,
}

impl CameraFrame {
    /// Create a downscaled copy of the frame for ML inference
    pub fn downscale(&self, target_width: u32, target_height: u32) -> Vec<u8> {
        if self.width == target_width && self.height == target_height {
            return self.data.clone();
        }

        let mut output = vec![0u8; (target_width * target_height * 4) as usize];
        let x_ratio = self.width as f32 / target_width as f32;
        let y_ratio = self.height as f32 / target_height as f32;

        for y in 0..target_height {
            for x in 0..target_width {
                let src_x = (x as f32 * x_ratio) as u32;
                let src_y = (y as f32 * y_ratio) as u32;
                let src_idx = ((src_y * self.width + src_x) * 4) as usize;
                let dst_idx = ((y * target_width + x) * 4) as usize;

                if src_idx + 3 < self.data.len() && dst_idx + 3 < output.len() {
                    output[dst_idx] = self.data[src_idx];
                    output[dst_idx + 1] = self.data[src_idx + 1];
                    output[dst_idx + 2] = self.data[src_idx + 2];
                    output[dst_idx + 3] = self.data[src_idx + 3];
                }
            }
        }

        output
    }
}

/// Information about an available camera
#[derive(Clone, Debug)]
pub struct CameraInfo {
    /// Camera index
    pub index: u32,
    /// Camera name
    pub name: String,
}

/// Camera capture interface
pub struct CameraCapture {
    /// Current frame (latest captured) - triple buffered
    frames: [Arc<Mutex<Option<CameraFrame>>>; 3],
    /// Index of the latest complete frame
    latest_frame_idx: Arc<AtomicU64>,
    /// Whether capture is running
    running: Arc<AtomicBool>,
    /// Capture thread handle
    thread_handle: Option<std::thread::JoinHandle<()>>,
    /// Camera resolution
    width: u32,
    height: u32,
    /// Frame counter
    frame_count: Arc<AtomicU64>,
}

impl CameraCapture {
    /// List available cameras
    pub fn list_cameras() -> Vec<CameraInfo> {
        let mut cameras = Vec::new();

        // Try to enumerate cameras
        match nokhwa::query(nokhwa::utils::ApiBackend::Auto) {
            Ok(camera_list) => {
                for (idx, info) in camera_list.iter().enumerate() {
                    cameras.push(CameraInfo {
                        index: idx as u32,
                        name: info.human_name().to_string(),
                    });
                }
            }
            Err(e) => {
                log::warn!("Failed to enumerate cameras: {:?}", e);
            }
        }

        cameras
    }

    /// Create a new camera capture instance
    ///
    /// # Arguments
    /// * `camera_index` - The camera index to use (0 for default)
    /// * `width` - Requested frame width
    /// * `height` - Requested frame height
    pub fn new(camera_index: u32, width: u32, height: u32) -> Result<Self, String> {
        let frames: [Arc<Mutex<Option<CameraFrame>>>; 3] = [
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(None)),
        ];
        let latest_frame_idx = Arc::new(AtomicU64::new(0));
        let running = Arc::new(AtomicBool::new(true));
        let frame_count = Arc::new(AtomicU64::new(0));

        // Clone for the capture thread
        let frames_clone = frames.clone();
        let latest_frame_idx_clone = latest_frame_idx.clone();
        let running_clone = running.clone();
        let frame_count_clone = frame_count.clone();

        // Start capture thread
        let thread_handle = std::thread::Builder::new()
            .name("camera-capture".to_string())
            .spawn(move || {
                Self::capture_thread(
                    camera_index,
                    width,
                    height,
                    frames_clone,
                    latest_frame_idx_clone,
                    running_clone,
                    frame_count_clone,
                );
            })
            .map_err(|e| format!("Failed to spawn capture thread: {}", e))?;

        Ok(Self {
            frames,
            latest_frame_idx,
            running,
            thread_handle: Some(thread_handle),
            width,
            height,
            frame_count,
        })
    }

    /// Camera capture thread
    fn capture_thread(
        camera_index: u32,
        _width: u32,
        _height: u32,
        frames: [Arc<Mutex<Option<CameraFrame>>>; 3],
        latest_frame_idx: Arc<AtomicU64>,
        running: Arc<AtomicBool>,
        frame_count: Arc<AtomicU64>,
    ) {
        log::info!("Starting camera capture thread (camera {})", camera_index);

        // Create camera - use highest resolution that the camera supports
        let index = CameraIndex::Index(camera_index);

        // First try with AbsoluteHighestResolution
        let requested = RequestedFormat::new::<RgbAFormat>(
            RequestedFormatType::AbsoluteHighestResolution
        );

        let mut camera = match Camera::new(index.clone(), requested) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to open camera with highest resolution: {:?}", e);

                // Try with HighestResolution instead
                let requested2 = RequestedFormat::new::<RgbAFormat>(
                    RequestedFormatType::HighestResolution(nokhwa::utils::Resolution::new(640, 480))
                );

                match Camera::new(index.clone(), requested2) {
                    Ok(c) => c,
                    Err(e2) => {
                        log::warn!("Failed with HighestResolution: {:?}", e2);

                        // Last resort: try None with different pixel format
                        let requested3 = RequestedFormat::new::<RgbAFormat>(RequestedFormatType::None);
                        match Camera::new(index, requested3) {
                            Ok(c) => c,
                            Err(e3) => {
                                log::error!("Failed to open camera with all format attempts: {:?}", e3);
                                return;
                            }
                        }
                    }
                }
            }
        };

        // Open the camera stream
        if let Err(e) = camera.open_stream() {
            log::error!("Failed to open camera stream: {:?}", e);
            return;
        }

        log::info!(
            "Camera opened: {} ({}x{})",
            camera.info().human_name(),
            camera.resolution().width(),
            camera.resolution().height()
        );

        let mut write_idx: u64 = 0;

        while running.load(Ordering::Acquire) {
            // Capture frame
            match camera.frame() {
                Ok(frame) => {
                    let decoded = frame.decode_image::<RgbAFormat>();
                    match decoded {
                        Ok(image) => {
                            let frame_num = frame_count.fetch_add(1, Ordering::Relaxed);

                            // Convert to RGBA bytes
                            let rgba_data = image.into_raw();

                            let camera_frame = CameraFrame {
                                data: rgba_data,
                                width: frame.resolution().width(),
                                height: frame.resolution().height(),
                                frame_number: frame_num,
                                timestamp: Instant::now(),
                            };

                            // Write to the next buffer slot
                            let slot = (write_idx % 3) as usize;
                            *frames[slot].lock() = Some(camera_frame);

                            // Update latest frame index
                            latest_frame_idx.store(write_idx, Ordering::Release);
                            write_idx = write_idx.wrapping_add(1);
                        }
                        Err(e) => {
                            log::warn!("Failed to decode frame: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to capture frame: {:?}", e);
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        }

        log::info!("Camera capture thread stopped");
    }

    /// Get the latest captured frame
    pub fn latest_frame(&self) -> Option<CameraFrame> {
        let idx = self.latest_frame_idx.load(Ordering::Acquire);
        let slot = (idx % 3) as usize;
        self.frames[slot].lock().clone()
    }

    /// Check if capture is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Get the camera resolution
    pub fn resolution(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Get frame count
    pub fn frame_count(&self) -> u64 {
        self.frame_count.load(Ordering::Relaxed)
    }

    /// Stop capturing
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Release);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for CameraCapture {
    fn drop(&mut self) {
        self.stop();
    }
}
