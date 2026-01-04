# Immersive Server vs Resolume Arena Feature Comparison

Comparison of video transformation and compositing features between Immersive Server and Resolume Arena.

## Summary

Immersive Server has **solid foundations** for video compositing but is **roughly 60-70% complete** compared to Resolume Arena's transform/compositing feature set. The core layer system is robust, but advanced output mapping and MIDI/OSC control are missing.

---

## Feature Status Overview

### Fully Implemented

| Feature | Notes |
|---------|-------|
| **Layer Transforms** | |
| Position (X/Y) | Pixel-based, UI controls |
| Scale (uniform/non-uniform) | Independent X/Y |
| Rotation | Radians internally, degrees in UI |
| Anchor Point | 0.0-1.0 normalized |
| **Layer Controls** | |
| Opacity | 0.0-1.0 with clamping |
| Layer Ordering | Full reorder API |
| Visible toggle | Per-layer visibility |
| **Clip System** | |
| Clip Grid (VJ launcher) | 8+ slots per layer |
| Clip-level transforms | Separate from layer |
| Cut transition | Instant switch |
| Fade transition | Configurable duration |
| **Automation** | |
| BPM Sync | Tap tempo, bar sync |
| LFO Shapes | Sine, Triangle, Square, Saw, Random (6 total) |
| Audio Reactive (FFT) | Low/Mid/High/Full bands |
| ADSR Envelopes | Beat-triggered |
| **Output** | |
| NDI Streaming | Complete |
| OMT Streaming | Complete |
| Viewport Pan/Zoom | Spring physics |

### Partially Implemented

| Feature | Gap Description |
|---------|-----------------|
| **Blend Modes** | Only 4 modes (Normal, Additive, Multiply, Screen) vs Resolume's 20+ |
| **Masking** | Auto Mask effect only; no layer-to-layer luma/alpha masking |
| **Crossfade Transition** | Fade exists but no distinct crossfade UI |
| **Multi-Output** | Basic API exists; needs multi-window rendering |

### Not Implemented

| Feature | Priority | Roadmap Phase |
|---------|----------|---------------|
| **Timeline Keyframes** | High | Not planned |
| **MIDI Control** | High | Planned (types.rs has placeholders) |
| **OSC Control** | High | Planned (types.rs has placeholders) |
| **Advanced Output Mapping** | High | Phase 9 |
| - Slices/Screens | | |
| - Mesh Warp | | |
| - Edge Blending | | |
| - Per-output Color | | |
| **Soft Edge Masking** | Medium | Phase 9 |
| **Master Opacity** | Low | Quick addition |
| **Syphon/Spout** | Medium | Planned |
| **Additional Blend Modes** | Medium | Straightforward |

---

## Architecture Comparison

### Layer Hierarchy

Both systems use similar hierarchies:

```
Resolume Arena                    Immersive Server
─────────────────                 ─────────────────
Composition                       Environment
  └─ Layer[]                        └─ Layer[]
       ├─ Blend Mode                     ├─ BlendMode
       ├─ Opacity                        ├─ opacity
       ├─ Transform                      ├─ Transform2D
       └─ Clip[]                         └─ clips: Vec<ClipCell>
            └─ Clip Transform                 └─ transform: Transform2D
```

### Key Differences

| Aspect | Resolume | Immersive Server |
|--------|----------|------------------|
| **Transform Order** | Position → Rotation → Scale | Anchor → Scale → Rotate → Position |
| **Mask Layers** | Dedicated layer mode | Effect-based (Auto Mask) |
| **Slice Transform** | Effect routes to output slices | Not implemented |
| **Parameter Animation** | Timeline + envelopes | BPM/LFO/FFT only |

---

## Priority Gaps to Close

### 1. MIDI/OSC Control (High Impact)
- Placeholders exist in `effects/types.rs`
- Essential for live VJ performance
- Resolume's strength is real-time control

### 2. Advanced Output Mapping (High Impact)
- Slices, mesh warp, edge blend
- Planned for Phase 9
- Required for projection mapping installs

### 3. Additional Blend Modes (Medium Impact)
- Currently 4 vs 20+
- Straightforward shader additions
- File: `compositor/blend.rs`

### 4. Timeline Keyframes (Medium Impact)
- Currently no keyframe editor
- Only procedural automation (LFO/BPM/FFT)
- Would require significant UI work

### 5. Layer-to-Layer Masking (Medium Impact)
- Would need architectural change
- Currently effects-only approach

---

## Key File Locations

| Feature | Path |
|---------|------|
| Layer/Transform | `src/compositor/layer.rs` |
| Blend Modes | `src/compositor/blend.rs` |
| Clips | `src/compositor/clip.rs` |
| Environment | `src/compositor/environment.rs` |
| Automation | `src/effects/automation.rs` |
| FFT | `src/audio/fft.rs` |
| Effects Types | `src/effects/types.rs` |
| 3D Preview | `src/previs/` |

---

## Unique to Immersive Server

Features not found in Resolume:

- **3D Preview System** (previs) - Circle, Walls, Dome surface visualization for projection planning
- **OMT Protocol** - Ultra-low latency streaming via Aqueduct
- **Rust/wgpu Architecture** - Modern, cross-platform GPU backend
