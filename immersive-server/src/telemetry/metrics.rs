//! Performance metrics and frame timing
//!
//! Provides frame timing statistics and performance metrics collection.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// Frame timing statistics
#[derive(Debug, Clone, Default)]
pub struct FrameStats {
    /// Average frame time in milliseconds
    pub avg_ms: f64,
    /// Minimum frame time in milliseconds
    pub min_ms: f64,
    /// Maximum frame time in milliseconds
    pub max_ms: f64,
    /// 50th percentile (median) frame time
    pub p50_ms: f64,
    /// 95th percentile frame time
    pub p95_ms: f64,
    /// 99th percentile frame time
    pub p99_ms: f64,
    /// Number of samples in the statistics
    pub sample_count: usize,
}

/// GPU memory usage statistics
#[derive(Debug, Clone, Default)]
pub struct GpuMemoryStats {
    /// Environment texture memory in bytes
    pub environment_texture: u64,
    /// Layer texture memory in bytes
    pub layer_textures: u64,
    /// Effect buffer memory in bytes
    pub effect_buffers: u64,
    /// Total estimated GPU memory in bytes
    pub total: u64,
}

/// NDI receiver statistics
#[derive(Debug, Clone, Default)]
pub struct NdiStats {
    /// NDI source name
    pub source_name: String,
    /// Last frame pickup latency in milliseconds
    pub pickup_latency_ms: f64,
    /// Current number of frames in buffer
    pub queue_depth: usize,
    /// Buffer capacity
    pub buffer_capacity: usize,
    /// Total frames received
    pub frames_received: u64,
    /// Frames dropped due to full buffer
    pub frames_dropped: u64,
}

impl GpuMemoryStats {
    /// Get total memory in megabytes
    pub fn total_mb(&self) -> f64 {
        self.total as f64 / (1024.0 * 1024.0)
    }
}

/// Frame profiler for CPU timing
///
/// Collects frame timing data and computes statistics.
pub struct FrameProfiler {
    /// Frame durations
    frame_times: VecDeque<Duration>,
    /// Maximum samples to keep (5 seconds at 60fps)
    max_samples: usize,
    /// Last frame start time
    last_frame_start: Option<Instant>,
    /// Frame start times for FPS calculation
    frame_starts: VecDeque<Instant>,
}

impl Default for FrameProfiler {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameProfiler {
    /// Create a new frame profiler
    pub fn new() -> Self {
        Self {
            frame_times: VecDeque::with_capacity(300),
            max_samples: 300,
            last_frame_start: None,
            frame_starts: VecDeque::with_capacity(300),
        }
    }

    /// Mark the beginning of a frame
    ///
    /// Call this at the start of each frame to record timing.
    pub fn begin_frame(&mut self) {
        let now = Instant::now();

        // Record frame duration from last frame
        if let Some(start) = self.last_frame_start {
            let duration = now.duration_since(start);
            self.frame_times.push_back(duration);
            if self.frame_times.len() > self.max_samples {
                self.frame_times.pop_front();
            }
        }

        self.last_frame_start = Some(now);

        // Track frame starts for FPS
        self.frame_starts.push_back(now);
        if self.frame_starts.len() > self.max_samples {
            self.frame_starts.pop_front();
        }
    }

    /// Get frame timing statistics
    pub fn stats(&self) -> FrameStats {
        if self.frame_times.is_empty() {
            return FrameStats::default();
        }

        let mut times: Vec<f64> = self
            .frame_times
            .iter()
            .map(|d| d.as_secs_f64() * 1000.0)
            .collect();
        times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let sum: f64 = times.iter().sum();
        let count = times.len() as f64;

        FrameStats {
            avg_ms: sum / count,
            min_ms: times.first().copied().unwrap_or(0.0),
            max_ms: times.last().copied().unwrap_or(0.0),
            p50_ms: percentile(&times, 0.50),
            p95_ms: percentile(&times, 0.95),
            p99_ms: percentile(&times, 0.99),
            sample_count: times.len(),
        }
    }

    /// Calculate current FPS from frame start times
    pub fn fps(&self) -> f64 {
        if self.frame_starts.len() < 2 {
            return 0.0;
        }

        let first = self.frame_starts.front().unwrap();
        let last = self.frame_starts.back().unwrap();
        let duration = last.duration_since(*first).as_secs_f64();

        if duration > 0.0 {
            (self.frame_starts.len() - 1) as f64 / duration
        } else {
            0.0
        }
    }

    /// Get the last frame time in milliseconds
    pub fn last_frame_time_ms(&self) -> f64 {
        self.frame_times
            .back()
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0)
    }
}

/// Calculate percentile from sorted array
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() - 1) as f64 * p) as usize;
    sorted[idx]
}

/// Combined performance metrics
#[derive(Debug, Clone, Default)]
pub struct PerformanceMetrics {
    /// Frame timing statistics
    pub frame_stats: FrameStats,
    /// Current FPS
    pub fps: f64,
    /// Target FPS
    pub target_fps: u32,
    /// GPU render pass timings (in milliseconds)
    pub gpu_timings: HashMap<String, f64>,
    /// Total GPU time for the frame
    pub gpu_total_ms: f64,
    /// Number of active layers
    pub layer_count: usize,
    /// Number of actively playing clips
    pub active_clip_count: usize,
    /// Total number of effects applied
    pub effect_count: usize,
    /// GPU memory statistics
    pub gpu_memory: GpuMemoryStats,
    /// Time spent uploading video textures to GPU (milliseconds)
    pub video_frame_time_ms: f64,
    /// Time spent on UI rendering (milliseconds)
    pub ui_frame_time_ms: f64,
    /// NDI receiver statistics
    pub ndi_stats: Vec<NdiStats>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_profiler() {
        let mut profiler = FrameProfiler::new();

        // Simulate a few frames
        for _ in 0..10 {
            profiler.begin_frame();
            std::thread::sleep(Duration::from_millis(16));
        }

        let stats = profiler.stats();
        assert!(stats.avg_ms > 0.0);
        assert!(stats.sample_count > 0);
    }

    #[test]
    fn test_percentile() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        assert_eq!(percentile(&values, 0.5), 5.0); // Median
        assert_eq!(percentile(&values, 0.0), 1.0); // Min
        assert_eq!(percentile(&values, 1.0), 10.0); // Max
    }

    #[test]
    fn test_gpu_memory_stats() {
        let stats = GpuMemoryStats {
            environment_texture: 1024 * 1024, // 1 MB
            layer_textures: 2 * 1024 * 1024,  // 2 MB
            effect_buffers: 512 * 1024,       // 0.5 MB
            total: 3 * 1024 * 1024 + 512 * 1024,
        };
        assert!((stats.total_mb() - 3.5).abs() < 0.01);
    }
}
