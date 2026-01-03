# Telemetry & Performance Profiling

This document describes the logging, tracing, and performance profiling systems in Immersive Server.

## Overview

Immersive Server uses the `tracing` ecosystem for structured logging and performance monitoring:

- **Structured Logging** - Replace unstructured log lines with contextual spans
- **Frame Profiling** - CPU-side frame timing with percentile statistics
- **GPU Profiling** - Hardware timestamp queries for GPU timing breakdown
- **Performance Panel** - Real-time UI for monitoring performance metrics

## Configuration

### Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `IMMERSIVE_LOG` | Log level filter | `debug`, `info`, `warn`, `error` |
| `IMMERSIVE_LOG_JSON` | Enable JSON output | `1` or `true` |

### Examples

```bash
# Debug logging to console
IMMERSIVE_LOG=debug cargo run

# JSON logging for production
IMMERSIVE_LOG=info IMMERSIVE_LOG_JSON=1 cargo run

# Filter by module
IMMERSIVE_LOG=immersive_server::video=debug cargo run
```

## Performance Panel

Access via **View > Performance** in the menu bar.

### Metrics Displayed

| Metric | Description |
|--------|-------------|
| **FPS** | Current frames per second (color-coded) |
| **Frame Time** | Milliseconds per frame with visual bar |
| **Frame Stats** | Average, min, max, p50, p95, p99 percentiles |
| **GPU Timings** | Per-pass GPU timing (if hardware supports) |
| **Resources** | Layer count, active clips, effect count |
| **GPU Memory** | Estimated VRAM usage breakdown |

### FPS Color Coding

| Color | Meaning |
|-------|---------|
| Green | >= 95% of target FPS |
| Yellow | 80-95% of target FPS |
| Orange | 50-80% of target FPS |
| Red | < 50% of target FPS |

## API Endpoint

### GET /api/status/performance

Returns detailed performance metrics:

```json
{
  "fps": 60.0,
  "frame_time_ms": 16.67,
  "target_fps": 60,
  "frame_time_avg_ms": 16.5,
  "frame_time_min_ms": 15.2,
  "frame_time_max_ms": 18.1,
  "frame_time_p95_ms": 17.8,
  "frame_time_p99_ms": 18.0,
  "gpu_timings": {
    "layer_composition": 8.2,
    "effects": 3.1,
    "present": 1.5
  },
  "gpu_total_ms": 12.8,
  "layer_count": 4,
  "active_clips": 3,
  "effect_count": 2,
  "gpu_memory_mb": 256.5
}
```

## Architecture

### Module Structure

```
src/telemetry/
├── mod.rs        # Module exports
├── logging.rs    # Tracing subscriber setup
├── metrics.rs    # FrameProfiler, PerformanceMetrics
└── profiling.rs  # GpuProfiler (timestamp queries)
```

### FrameProfiler

Collects CPU-side frame timing over a rolling window (default 300 frames / 5 seconds at 60fps):

```rust
pub struct FrameProfiler {
    frame_times: VecDeque<Duration>,
    last_frame_start: Option<Instant>,
}

// Usage in render loop
self.frame_profiler.begin_frame();
// ... render ...
let stats = self.frame_profiler.stats();
```

### GpuProfiler

Uses wgpu timestamp queries for GPU-side profiling:

```rust
pub struct GpuProfiler {
    query_set: Option<QuerySet>,
    enabled: bool,
    // ...
}

// Usage
self.gpu_profiler.begin_region(&mut encoder, "layer_composition");
// ... GPU work ...
self.gpu_profiler.end_region(&mut encoder, "layer_composition");

// After submit
self.gpu_profiler.resolve(&mut encoder);
queue.submit(...);
self.gpu_profiler.process(&device);
```

**Note:** GPU timestamp queries require hardware support (`Features::TIMESTAMP_QUERY`). The profiler gracefully degrades when unavailable.

### GpuMemoryStats

Estimates GPU memory usage:

```rust
pub struct GpuMemoryStats {
    pub environment_texture: u64,  // Environment canvas
    pub layer_textures: u64,       // Video frame textures
    pub effect_buffers: u64,       // Effect ping-pong buffers
    pub total: u64,
}
```

## Performance Targets

| Metric | Target |
|--------|--------|
| Frame Rate | 60fps locked (vsync) |
| Frame Time | < 16.67ms @ 60fps |
| P95 Frame Time | < 20ms |
| GPU Memory | < 2GB for typical usage |

## Troubleshooting

### High Frame Times

1. Check **GPU Timings** breakdown to identify slow passes
2. Reduce layer count or disable unused effects
3. Lower environment resolution
4. Check for thermal throttling

### Missing GPU Timings

GPU timestamp queries require:
- Metal on macOS (usually supported)
- Vulkan/DX12 on Windows (varies by GPU)

If unavailable, the panel shows "GPU timing not available".

### High Memory Usage

Check the GPU Memory breakdown:
- **Environment**: Reduce canvas resolution
- **Layers**: Reduce video resolution or layer count
- **Effects**: Disable unused effects

## Extending

### Adding New Tracing Spans

```rust
use tracing::{span, Level};

fn my_function() {
    let _span = span!(Level::DEBUG, "my_operation").entered();
    // ... work ...
}
```

### Adding GPU Timing Regions

```rust
// In render code
self.gpu_profiler.begin_region(&mut encoder, "my_pass");
// ... GPU commands ...
self.gpu_profiler.end_region(&mut encoder, "my_pass");
```

The new region will automatically appear in the Performance panel.
