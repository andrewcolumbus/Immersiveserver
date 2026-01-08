//! Telemetry and logging infrastructure
//!
//! Provides structured logging with tracing and performance profiling.

pub mod logging;
pub mod metrics;
pub mod profiling;

pub use logging::{init_logging, LogConfig};
pub use metrics::{FrameProfiler, FrameStats, GpuMemoryStats, NdiStats, OmtStats, PerformanceMetrics};
pub use profiling::GpuProfiler;
