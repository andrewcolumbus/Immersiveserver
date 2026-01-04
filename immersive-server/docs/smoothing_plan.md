# FPS Smoothing Plan

This document outlines a 4-phase approach to achieving stable 60fps with minimal jitter in Immersive Server.

## Problem Analysis

### Current Frame Pacing Architecture

The render loop uses a hybrid sleep + spin-wait strategy:

```
main.rs:946-1011 (about_to_wait)
├── Calculate frame_nanos = 1_000_000_000 / target_fps (integer nanoseconds)
├── If >2ms until next frame: OS sleep via WaitUntil
├── If ≤2ms until next frame: spin-wait for precision
├── If >2 frames behind: reset schedule (skip frames)
└── Request redraw
```

### Identified Instability Sources

| Priority | Issue | Location | Impact |
|----------|-------|----------|--------|
| HIGH | `surface.get_current_texture()` blocking | app.rs:2732 | GPU backup stalls CPU |
| HIGH | GPU texture readback for streaming | app.rs:2775-2795 | `capture.process()` calls `device.poll()` |
| HIGH | Multiple layer texture uploads | app.rs:3408-3411 | Round-robin uploads spike bandwidth |
| MEDIUM | No GPU fence tracking | - | Can't detect if GPU is keeping up |
| LOW | egui texture/buffer updates | app.rs:2723-2729 | Scales with UI complexity |

---

## Phase 1: GPU Double-Buffering and Fence Tracking

### Goal
Prevent GPU backup from stalling the CPU by tracking frames in flight and allowing controlled buffering.

### User Setting: Low Latency Mode

Add a user-configurable setting to choose between stability and latency:

```rust
// Add to EnvironmentSettings (settings.rs ~line 54):
/// Low latency mode: trades stability for reduced input lag
/// - true:  1 frame in flight (~16ms less latency, may stutter under load)
/// - false: 2 frames in flight (smoother, but ~16ms more latency)
#[serde(rename = "lowLatencyMode", default = "default_low_latency_mode")]
pub low_latency_mode: bool,

fn default_low_latency_mode() -> bool {
    false // Default to stability (2 frames)
}
```

**UI Presentation** (properties_panel.rs):
- Label: "Low Latency Mode"
- Tooltip: "Reduces input lag by ~16ms but may cause stuttering under heavy GPU load. Disable for smoother playback."
- Default: Off (stability preferred)

### Changes

#### 1.1 Add Frame-in-Flight Tracking

```rust
// Add to App struct (app.rs ~line 240):
/// Frames currently in flight to the GPU
frames_in_flight: VecDeque<wgpu::SubmissionIndex>,
```

Initialize with:
```rust
frames_in_flight: VecDeque::with_capacity(3),
```

#### 1.2 Track Submissions

After `queue.submit()` (app.rs ~line 2772):

```rust
let submission_index = self.queue.submit(std::iter::once(encoder.finish()));
self.frames_in_flight.push_back(submission_index);

// Max frames in flight based on user preference
let max_in_flight = if self.settings.low_latency_mode { 1 } else { 2 };

// Wait for oldest frame if exceeding max (prevents unbounded GPU queue growth)
while self.frames_in_flight.len() > max_in_flight {
    let oldest = self.frames_in_flight.pop_front().unwrap();
    self.device.poll(wgpu::Maintain::WaitForSubmissionIndex(oldest));
}
```

#### 1.3 Early Surface Texture Acquisition

Move `get_current_texture()` from line 2732 to the **beginning** of `render()`:

```rust
pub fn render(&mut self, ...) -> Result<bool, wgpu::SurfaceError> {
    // Acquire surface texture early - GPU can prepare it while CPU does other work
    let output = self.surface.get_current_texture()?;
    let surface_view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

    // ... rest of render function uses surface_view later ...
}
```

This overlaps GPU surface preparation with CPU work (egui building, video polling).

#### 1.4 Configure Surface Frame Latency

Update surface configuration based on setting (app.rs ~line 326):

```rust
let surface_config = wgpu::SurfaceConfiguration {
    // ...
    desired_maximum_frame_latency: if settings.low_latency_mode { 1 } else { 2 },
    // ...
};
```

**Note:** When `low_latency_mode` changes at runtime, the surface must be reconfigured:

```rust
// In sync_settings() or similar:
if settings_changed.low_latency_mode {
    self.surface.configure(&self.device, &new_surface_config);
}
```

### Tradeoff Summary

| Mode | Frames in Flight | Latency | Stability |
|------|------------------|---------|-----------|
| Low Latency (on) | 1 | ~16ms lower | May stutter under load |
| Smooth (off, default) | 2 | ~16ms higher | Consistent frame pacing |

---

## Phase 2: Texture Upload Rate Limiting

### Goal
Prevent bandwidth spikes when multiple layers have new video frames simultaneously.

### Changes

#### 2.1 Rate-Limit Uploads in `update_videos()`

Modify app.rs ~line 3379:

```rust
pub fn update_videos(&mut self) {
    // Limit texture uploads per frame to prevent bandwidth spikes
    const MAX_UPLOADS_PER_FRAME: usize = 2;
    let mut uploads_this_frame = 0;

    let mut layer_ids: Vec<u32> = self.layer_runtimes.keys().copied().collect();
    layer_ids.sort();

    let start_idx = layer_ids.iter()
        .position(|&id| id > self.last_upload_layer)
        .unwrap_or(0);

    for i in 0..layer_ids.len() {
        // Stop if we've hit the upload limit
        if uploads_this_frame >= MAX_UPLOADS_PER_FRAME {
            break;
        }

        let idx = (start_idx + i) % layer_ids.len();
        let layer_id = layer_ids[idx];

        if let Some(runtime) = self.layer_runtimes.get_mut(&layer_id) {
            if runtime.try_update_texture(&self.queue) {
                self.last_upload_layer = layer_id;
                uploads_this_frame += 1;
            }
        }
    }

    // Pending runtimes get priority (user waiting for first frame)
    // But still count toward limit
    if uploads_this_frame < MAX_UPLOADS_PER_FRAME {
        for runtime in self.pending_runtimes.values_mut() {
            if uploads_this_frame >= MAX_UPLOADS_PER_FRAME {
                break;
            }
            if runtime.try_update_texture(&self.queue) {
                uploads_this_frame += 1;
            }
        }
    }

    // Preview player always gets one upload (if needed)
    self.preview_player.update(&self.queue);
}
```

This distributes uploads across multiple frames. With 16 layers all triggering simultaneously, uploads complete over 8 frames (~133ms at 60fps) instead of causing a single-frame spike.

---

## Phase 3: Capture Pipeline Optimization

### Goal
Remove GPU synchronization stalls from the streaming capture path (OMT/NDI).

**This phase is HIGH PRIORITY since OMT/NDI streaming is heavily used.**

### Current Problem

In `omt_capture.rs` and `ndi_capture.rs`, the `process()` function calls `device.poll()` which can block waiting for GPU work to complete:

```rust
// Current blocking pattern in process():
if self.staging_buffers[i].state == BufferState::Pending {
    device.poll(wgpu::Maintain::Poll); // BLOCKS waiting for GPU
    // ...
}
```

### Changes

#### 3.1 Add Async Map Completion Flag

Add to staging buffer struct:

```rust
struct StagingBuffer {
    buffer: wgpu::Buffer,
    state: BufferState,
    // NEW: Callback sets this when map completes
    map_complete: Arc<AtomicBool>,
}
```

#### 3.2 Replace Blocking Poll with Callback

In `process()` function:

```rust
pub fn process(&mut self, device: &wgpu::Device) {
    // NOTE: We no longer call device.poll() here!
    // The frame-in-flight tracker in app.rs handles GPU synchronization.

    for i in 0..self.staging_buffers.len() {
        let staging = &mut self.staging_buffers[i];

        match staging.state {
            BufferState::Pending => {
                // Start async map with completion callback
                let map_complete = staging.map_complete.clone();
                staging.buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| {
                    if result.is_ok() {
                        map_complete.store(true, Ordering::Release);
                    }
                });
                staging.state = BufferState::Mapping;
            }
            BufferState::Mapping => {
                // Check if callback fired (non-blocking)
                if staging.map_complete.load(Ordering::Acquire) {
                    staging.state = BufferState::Ready;
                    staging.map_complete.store(false, Ordering::Release);
                }
            }
            BufferState::Ready => {
                // Read data and send to encoder
                // ... existing logic ...
            }
            _ => {}
        }
    }
}
```

#### 3.3 Files to Modify

- `src/network/omt_capture.rs` - OMT streaming capture
- `src/network/ndi_capture.rs` - NDI streaming capture
- `src/network/spout_capture.rs` - Spout capture (Windows)

---

## Phase 4: Adaptive Frame Scheduling

### Goal
Dynamically adjust frame timing based on measured GPU performance.

### Changes

#### 4.1 Track GPU Lateness

Add to App struct:

```rust
/// Rolling window tracking if GPU was late (couldn't keep up)
gpu_late_window: VecDeque<bool>,
/// How many frames to track (1 second at 60fps)
const GPU_LATE_WINDOW_SIZE: usize = 60;
```

Update after each frame:

```rust
// After frame completes, check if we were late
let frame_was_late = frame_time > target_frame_duration * 1.1; // 10% tolerance
self.gpu_late_window.push_back(frame_was_late);
if self.gpu_late_window.len() > GPU_LATE_WINDOW_SIZE {
    self.gpu_late_window.pop_front();
}
```

#### 4.2 Add Helper Method

```rust
impl App {
    /// Returns true if GPU has been consistently late (>50% of recent frames)
    pub fn gpu_consistently_late(&self) -> bool {
        if self.gpu_late_window.len() < GPU_LATE_WINDOW_SIZE / 2 {
            return false; // Not enough data
        }
        let late_count = self.gpu_late_window.iter().filter(|&&x| x).count();
        late_count > self.gpu_late_window.len() / 2
    }
}
```

#### 4.3 Adaptive Spin Threshold

Modify main.rs ~line 974:

```rust
// Adaptive spin threshold based on GPU load
let spin_threshold = if app.gpu_consistently_late() {
    // GPU is struggling - use tighter spin to minimize overhead
    Duration::from_micros(500)
} else {
    // Normal operation - standard 2ms spin threshold
    Duration::from_micros(2000)
};

if now < self.next_redraw_at {
    if self.next_redraw_at.duration_since(now) <= spin_threshold {
        while Instant::now() < self.next_redraw_at {
            std::hint::spin_loop();
        }
    } else {
        // ...
    }
}
```

---

## Implementation Order

| Order | Phase | Rationale |
|-------|-------|-----------|
| 1st | Phase 1: GPU Double-Buffering | Foundation - enables controlled GPU pipelining |
| 2nd | Phase 3: Capture Optimization | High priority - streaming is heavily used |
| 3rd | Phase 2: Texture Upload Limiting | Smooths bandwidth spikes |
| 4th | Phase 4: Adaptive Scheduling | Refinement - builds on earlier phases |

---

## Expected Impact

| Phase | Improvement |
|-------|-------------|
| Phase 1 | Eliminates GPU backup stalls - largest single improvement |
| Phase 2 | Prevents frame drops during multi-layer scene changes |
| Phase 3 | Removes streaming-induced jank |
| Phase 4 | Adapts to system load dynamically |

**Target:** 60fps locked with < 1ms frame time variance

---

## Files Summary

| File | Changes |
|------|---------|
| `src/settings.rs` | Add `low_latency_mode` setting |
| `src/app.rs` | Frame-in-flight tracking, early surface acquisition, upload rate limiting, GPU late tracking, respect `low_latency_mode` |
| `src/main.rs` | Adaptive spin threshold |
| `src/ui/properties_panel.rs` | Add "Low Latency Mode" toggle in Environment settings |
| `src/network/omt_capture.rs` | Remove blocking poll, async map callback |
| `src/network/ndi_capture.rs` | Remove blocking poll, async map callback |
| `src/network/spout_capture.rs` | Remove blocking poll, async map callback (Windows) |

---

## Measurement

To verify improvements, use the existing `FrameProfiler` in `telemetry/metrics.rs`:

- Monitor p95/p99 frame times (should approach target frame duration)
- Watch for frame time variance (should be < 1ms in stable state)
- Check for frame drops during scene transitions and streaming
