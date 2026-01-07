# Potential Performance Optimizations

This document outlines the current performance state and potential future optimizations for the video pipeline.

## Current State (Optimized)

### HAP Playback

| Step | CPU Cost | Status |
|------|----------|--------|
| Disk read | Minimal | Kernel-cached |
| HAP container parse | Trivial | ~8 bytes header |
| Snappy decompress | ~10-15% | **Eliminated with uncompressed option** |
| DXT upload | ~5% | GPU-native BC1/BC3 |

**Result:** Uncompressed HAP is near-zero CPU. Just disk I/O + memcpy to GPU.

### NDI Streams

| Step | CPU Cost | Status |
|------|----------|--------|
| NDI SDK receive | External | N/A |
| Buffer copy | ~10% | `Bytes::copy` at ndi.rs:300 |
| Color conversion | ~20% | **Eliminated with BGRA pipeline mode** |
| Texture upload | ~5% | Direct BGRA upload |

**Result:** BGRA pipeline mode eliminates color conversion. One buffer copy remains.

### H.264/HEVC (Not Optimized)

| Step | CPU Cost | Status |
|------|----------|--------|
| Hardware decode | Low | VideoToolbox/NVDEC |
| hwframe→CPU transfer | ~40% | Copies GPU frame back to CPU |
| libswscale conversion | ~30% | CPU-based YUV→RGBA |
| Texture upload | ~15% | Full frame memcpy |

**Result:** Hardware decode benefits are lost due to immediate CPU transfer. Not a priority if using HAP workflow.

## Estimated CPU Usage (4K@60fps)

| Source Type | CPU Usage |
|-------------|-----------|
| Uncompressed HAP | ~1-2% |
| HAP (Snappy) | ~10-15% |
| NDI (BGRA mode) | ~5-10% |
| NDI (RGBA mode) | ~25-30% |
| H.264/HEVC | ~40-60% |

## Future Optimization Opportunities

### 1. Memory-Mapped HAP Files

**Impact:** ~5% CPU reduction
**Effort:** Medium
**Priority:** Low

```rust
// Current: File → Read buffer → Vec → write_texture
// Optimized: File → mmap → write_texture (zero-copy from kernel cache)
```

Would require:
- `memmap2` crate for safe memory mapping
- Modify `parse_hap_packet()` to work with mmap slices
- Handle file lifetime carefully

### 2. Direct NDI Buffer Usage

**Impact:** ~10% CPU reduction
**Effort:** Low-Medium
**Priority:** Low

```rust
// Current: NDI frame.data → Bytes::copy → write_texture
// Optimized: NDI frame.data → write_texture directly
```

Would require:
- Use NDI buffer lifetime directly (valid until next `recv()`)
- Avoid intermediate `Bytes` allocation
- Careful synchronization with texture upload

### 3. Pre-allocated Staging Buffers

**Impact:** ~2-3% CPU reduction
**Effort:** Medium
**Priority:** Low

Instead of letting wgpu allocate staging buffers per-frame:
- Pre-allocate ring buffer of staging buffers
- Map persistently, rotate between frames
- Reduces allocation overhead

### 4. Zero-Copy VideoToolbox Decode (macOS)

**Impact:** ~70% CPU reduction for H.264/HEVC
**Effort:** Major
**Priority:** Not needed if using HAP workflow

Would require:
- Direct FFmpeg C API access (bypass ffmpeg-next)
- IOSurface/CVPixelBuffer integration
- Metal texture import from IOSurface
- macOS only

## Recommendations

The current state is good for HAP + NDI workflows. Further optimization should be deferred until:

1. Scaling to more simultaneous layers (8+)
2. Higher resolutions (8K+)
3. Actual performance issues observed

## Files Reference

- HAP decode: `video/decoder.rs:683-756` (parse_hap_packet)
- HAP upload: `video/texture.rs:139-163` (BC upload path)
- NDI receive: `network/ndi.rs:287-356`
- Texture upload: `video/texture.rs:126-186`
- BGRA pipeline setting: `settings.rs` (bgra_pipeline_enabled)
