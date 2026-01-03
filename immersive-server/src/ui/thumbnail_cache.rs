//! Thumbnail cache for video clip previews
//!
//! Generates and caches thumbnail images from video files using a background
//! thread to avoid blocking the UI.

use egui::{ColorImage, Context, TextureHandle, TextureOptions};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::settings::ThumbnailMode;
use crate::video::VideoDecoder;

/// Thumbnail dimensions (square) - sized for 2x Retina displays
const THUMBNAIL_SIZE: u32 = 160;

/// Maximum number of thumbnails to cache
const MAX_CACHE_SIZE: usize = 200;

/// Result of thumbnail generation
struct ThumbnailResult {
    key: String,
    pixels: Vec<u8>,
    width: u32,
    height: u32,
}

/// Request for thumbnail generation
struct ThumbnailRequest {
    key: String,
    path: PathBuf,
    mode: ThumbnailMode,
}

/// Cache for video thumbnails with background generation
pub struct ThumbnailCache {
    /// Cached thumbnail textures (key -> TextureHandle)
    cache: HashMap<String, TextureHandle>,
    /// Keys currently being generated (to avoid duplicate requests)
    pending: HashSet<String>,
    /// Keys that failed to generate (don't retry)
    failed: HashSet<String>,
    /// Channel to send generation requests to background thread
    request_tx: Sender<ThumbnailRequest>,
    /// Channel to receive completed thumbnails from background thread
    result_rx: Receiver<ThumbnailResult>,
    /// Current thumbnail mode (used to invalidate cache when changed)
    current_mode: ThumbnailMode,
}

impl ThumbnailCache {
    /// Create a new thumbnail cache with a background generation thread
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::channel::<ThumbnailRequest>();
        let (result_tx, result_rx) = mpsc::channel::<ThumbnailResult>();

        // Spawn background thread for thumbnail generation
        thread::Builder::new()
            .name("thumbnail-generator".into())
            .spawn(move || {
                Self::generator_thread(request_rx, result_tx);
            })
            .expect("Failed to spawn thumbnail generator thread");

        Self {
            cache: HashMap::new(),
            pending: HashSet::new(),
            failed: HashSet::new(),
            request_tx,
            result_rx,
            current_mode: ThumbnailMode::default(),
        }
    }

    /// Background thread that generates thumbnails
    fn generator_thread(
        request_rx: Receiver<ThumbnailRequest>,
        result_tx: Sender<ThumbnailResult>,
    ) {
        while let Ok(request) = request_rx.recv() {
            // Generate thumbnail
            if let Some(result) = Self::generate_thumbnail(&request.key, &request.path, request.mode) {
                // Send result back to main thread
                if result_tx.send(result).is_err() {
                    // Main thread dropped, exit
                    break;
                }
            }
            // If generation failed, we just don't send a result
            // The main thread will see it's no longer pending but not in cache
        }
    }

    /// Generate a thumbnail from a video file
    fn generate_thumbnail(key: &str, path: &PathBuf, mode: ThumbnailMode) -> Option<ThumbnailResult> {
        // Try to open the video
        let mut decoder = match VideoDecoder::open(path) {
            Ok(d) => d,
            Err(e) => {
                tracing::debug!("Failed to open video for thumbnail {}: {}", key, e);
                return None;
            }
        };

        // Seek to middle of video (or 1 second if very short)
        let duration = decoder.duration();
        let seek_time = if duration > 2.0 {
            duration / 2.0
        } else {
            duration.min(1.0)
        };

        // Decode a frame (always get RGBA, not DXT for HAP videos)
        let frame = match decoder.seek_and_decode_frame_rgba(seek_time) {
            Ok(f) => f,
            Err(e) => {
                tracing::debug!("Failed to decode frame for thumbnail {}: {}", key, e);
                return None;
            }
        };

        // Resize to thumbnail size based on mode
        tracing::debug!(
            "Generating thumbnail for {} (source: {}x{}, mode={:?})",
            key, frame.width, frame.height, mode
        );
        let (resized_pixels, thumb_w, thumb_h) =
            Self::resize_frame(&frame.data, frame.width, frame.height, THUMBNAIL_SIZE, mode);
        tracing::debug!(
            "Thumbnail {} result: {}x{} (expected {}x{} for Fill)",
            key, thumb_w, thumb_h, THUMBNAIL_SIZE, THUMBNAIL_SIZE
        );

        Some(ThumbnailResult {
            key: key.to_string(),
            pixels: resized_pixels,
            width: thumb_w,
            height: thumb_h,
        })
    }

    /// Resize RGBA frame data based on thumbnail mode
    fn resize_frame(
        data: &[u8],
        src_width: u32,
        src_height: u32,
        target_size: u32,
        mode: ThumbnailMode,
    ) -> (Vec<u8>, u32, u32) {
        let aspect = src_width as f32 / src_height as f32;
        tracing::info!(
            "resize_frame: {}x{} aspect={:.2}, mode={:?}",
            src_width, src_height, aspect, mode
        );

        match mode {
            ThumbnailMode::Fit => {
                tracing::info!("Using FIT mode branch");
                // Fit: Scale to fit within target_size, maintaining aspect ratio
                let (dst_width, dst_height) = if aspect > 1.0 {
                    // Wider than tall
                    (target_size, (target_size as f32 / aspect) as u32)
                } else {
                    // Taller than wide
                    ((target_size as f32 * aspect) as u32, target_size)
                };

                let dst_width = dst_width.max(1);
                let dst_height = dst_height.max(1);

                // Simple nearest-neighbor resize
                let mut resized = vec![0u8; (dst_width * dst_height * 4) as usize];

                for y in 0..dst_height {
                    for x in 0..dst_width {
                        let src_x = (x as f32 * src_width as f32 / dst_width as f32) as u32;
                        let src_y = (y as f32 * src_height as f32 / dst_height as f32) as u32;

                        let src_idx = ((src_y * src_width + src_x) * 4) as usize;
                        let dst_idx = ((y * dst_width + x) * 4) as usize;

                        if src_idx + 3 < data.len() && dst_idx + 3 < resized.len() {
                            resized[dst_idx] = data[src_idx];
                            resized[dst_idx + 1] = data[src_idx + 1];
                            resized[dst_idx + 2] = data[src_idx + 2];
                            resized[dst_idx + 3] = data[src_idx + 3];
                        }
                    }
                }

                (resized, dst_width, dst_height)
            }
            ThumbnailMode::Fill => {
                tracing::info!("Using FILL mode branch");
                // Fill: Center crop to square, then resize to target
                let dst_size = target_size;

                // Calculate center crop region - take the largest centered square
                let crop_size = src_width.min(src_height);
                let crop_x = (src_width.saturating_sub(crop_size)) / 2;
                let crop_y = (src_height.saturating_sub(crop_size)) / 2;

                tracing::info!(
                    "FILL: crop_size={}, crop_x={}, crop_y={}, data.len()={}, expected={}",
                    crop_size, crop_x, crop_y, data.len(), src_width * src_height * 4
                );

                // Resize the cropped square region to target size
                let mut resized = vec![0u8; (dst_size * dst_size * 4) as usize];
                let scale = crop_size as f32 / dst_size as f32;

                for dst_y in 0..dst_size {
                    for dst_x in 0..dst_size {
                        // Map destination pixel to source pixel in the crop region
                        let src_x = crop_x + (dst_x as f32 * scale) as u32;
                        let src_y = crop_y + (dst_y as f32 * scale) as u32;

                        // Clamp to valid range
                        let src_x = src_x.min(src_width.saturating_sub(1));
                        let src_y = src_y.min(src_height.saturating_sub(1));

                        let src_idx = ((src_y as usize) * (src_width as usize) + (src_x as usize)) * 4;
                        let dst_idx = ((dst_y as usize) * (dst_size as usize) + (dst_x as usize)) * 4;

                        if src_idx + 3 < data.len() && dst_idx + 3 < resized.len() {
                            resized[dst_idx] = data[src_idx];
                            resized[dst_idx + 1] = data[src_idx + 1];
                            resized[dst_idx + 2] = data[src_idx + 2];
                            resized[dst_idx + 3] = data[src_idx + 3];
                        }
                    }
                }

                // Log sample pixels to verify
                tracing::info!(
                    "FILL result: first pixel [{},{},{},{}], middle pixel at y=80 [{},{},{},{}]",
                    resized[0], resized[1], resized[2], resized[3],
                    resized[80 * dst_size as usize * 4], resized[80 * dst_size as usize * 4 + 1],
                    resized[80 * dst_size as usize * 4 + 2], resized[80 * dst_size as usize * 4 + 3]
                );

                (resized, dst_size, dst_size)
            }
        }
    }

    /// Poll for completed thumbnails and insert them into the cache
    ///
    /// Call this each frame from the main thread.
    pub fn poll(&mut self, ctx: &Context) {
        // Process all available results
        while let Ok(result) = self.result_rx.try_recv() {
            // Discard results for keys that are no longer pending
            // (they were cleared due to mode change)
            if !self.pending.remove(&result.key) {
                tracing::debug!("Discarding stale thumbnail result for {}", result.key);
                continue;
            }

            tracing::debug!(
                "Thumbnail received: {} ({}x{}, mode={:?})",
                result.key, result.width, result.height, self.current_mode
            );

            // Create egui texture from pixels
            let image = ColorImage::from_rgba_unmultiplied(
                [result.width as usize, result.height as usize],
                &result.pixels,
            );

            let texture = ctx.load_texture(
                format!("thumb_{}", result.key),
                image,
                TextureOptions::LINEAR,
            );

            // Insert into cache (with LRU eviction if needed)
            if self.cache.len() >= MAX_CACHE_SIZE {
                // Simple eviction: remove first key (not true LRU but good enough)
                if let Some(key) = self.cache.keys().next().cloned() {
                    self.cache.remove(&key);
                }
            }

            self.cache.insert(result.key, texture);
        }
    }

    /// Get a cached thumbnail if available
    pub fn get(&self, key: &str) -> Option<&TextureHandle> {
        self.cache.get(key)
    }

    /// Request thumbnail generation for a video file
    ///
    /// Returns true if a new request was made, false if already pending/cached/failed.
    pub fn request(&mut self, key: String, path: PathBuf, mode: ThumbnailMode) -> bool {
        // Don't request if already cached, pending, or failed
        if self.cache.contains_key(&key)
            || self.pending.contains(&key)
            || self.failed.contains(&key)
        {
            return false;
        }

        // Send request to background thread
        let request = ThumbnailRequest {
            key: key.clone(),
            path,
            mode,
        };

        if self.request_tx.send(request).is_ok() {
            self.pending.insert(key);
            true
        } else {
            false
        }
    }

    /// Set the thumbnail mode, clearing the cache if the mode changed
    ///
    /// Returns true if the mode changed and cache was cleared.
    pub fn set_mode(&mut self, mode: ThumbnailMode) -> bool {
        if self.current_mode != mode {
            self.current_mode = mode;
            self.clear();
            true
        } else {
            false
        }
    }

    /// Get the current thumbnail mode
    pub fn mode(&self) -> ThumbnailMode {
        self.current_mode
    }

    /// Check if a thumbnail is pending generation
    pub fn is_pending(&self, key: &str) -> bool {
        self.pending.contains(key)
    }

    /// Clear the entire cache (including pending requests)
    pub fn clear(&mut self) {
        self.cache.clear();
        self.failed.clear();
        self.pending.clear();
        // Note: in-flight requests may still complete but will be discarded
        // since their keys are no longer in pending
    }

    /// Invalidate a specific thumbnail (e.g., when clip is reassigned)
    pub fn invalidate(&mut self, key: &str) {
        self.cache.remove(key);
        self.failed.remove(key);
    }
}

impl Default for ThumbnailCache {
    fn default() -> Self {
        Self::new()
    }
}
