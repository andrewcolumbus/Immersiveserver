# Aqueduct (Rust OMT) Evaluation

**Date:** December 2024  
**Version Evaluated:** 0.1.0  
**Purpose:** Evaluate Aqueduct as the Rust-native OMT implementation for immersive-server

---

## Executive Summary

**Recommendation: ✅ ADOPT Aqueduct** as the primary OMT implementation for immersive-server.

Aqueduct is a well-designed, pure Rust implementation of the Open Media Transport (OMT) protocol that aligns perfectly with our technology stack. It provides sender/receiver functionality, mDNS-based discovery, and efficient zero-copy frame handling.

---

## What is Aqueduct?

Aqueduct is a high-performance Rust implementation of the **Open Media Transport (OMT)** protocol. OMT is an open, royalty-free alternative to NDI for transmitting real-time video, audio, and metadata over IP networks with minimal latency.

### Key Differentiators from NDI
- **Open Source:** MIT licensed, no SDK fees or licensing restrictions
- **Rust Native:** Memory-safe, concurrent, and integrates seamlessly with our codebase
- **Zero-Copy Architecture:** Uses `bytes::BytesMut` for minimal allocations
- **Modern Async:** Built on Tokio for efficient non-blocking I/O

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         AQUEDUCT STACK                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    APPLICATION LAYER                          │   │
│  │  • Sender: Capture → Encode → Send                           │   │
│  │  • Receiver: Discover → Connect → Decode → Display           │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                │                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    PROTOCOL LAYER                             │   │
│  │  • Packet Types: Video, Audio, Metadata                      │   │
│  │  • Header: [Type: u8][Length: u32]                           │   │
│  │  • Pixel Formats: UYVY, BGRA, NV12, YV12, etc.              │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                │                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    CODEC LAYER                                │   │
│  │  • LZ4 Compression (default)                                 │   │
│  │  • Extensible codec interface                                │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                │                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    TRANSPORT LAYER                            │   │
│  │  • TCP/IP (reliable streaming)                               │   │
│  │  • Tokio async runtime                                       │   │
│  │  • Broadcast channel for multi-receiver                      │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                │                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    DISCOVERY LAYER                            │   │
│  │  • mDNS-SD (Bonjour-compatible)                              │   │
│  │  • Service type: _omt._tcp.local.                            │   │
│  │  • Auto-registration and browsing                            │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Feature Analysis

### ✅ Implemented Features

| Feature | Status | Notes |
|---------|--------|-------|
| **TCP Sender** | ✅ Complete | Multi-receiver via broadcast channel |
| **TCP Receiver** | ✅ Complete | Zero-copy frame reassembly |
| **mDNS Discovery** | ✅ Complete | Using `mdns-sd` crate |
| **Video Frames** | ✅ Complete | Multiple pixel formats supported |
| **Audio Frames** | ✅ Complete | 32-bit float samples |
| **Metadata Frames** | ✅ Complete | XML-based sidecar data |
| **LZ4 Compression** | ✅ Complete | Fast codec for real-time |
| **Timestamps** | ✅ Complete | Microsecond precision |
| **Screen Capture** | ✅ Optional | Via `xcap` feature flag |

### ⚠️ Roadmap Features (Not Yet Implemented)

| Feature | Priority | Notes |
|---------|----------|-------|
| QUIC Transport | Medium | Currently TCP-only |
| Hardware Encoding | Medium | CPU-only for now |
| Auto-Reconnection | Medium | Manual reconnect required |
| FEC/Retransmission | Low | Packet loss not handled |
| PTP/NTP Sync | Low | Timestamp sync not precise |

---

## Code Quality Assessment

### Strengths

1. **Clean API Design**
   ```rust
   // Simple sender creation
   let sender = Sender::new(9000).await?;
   sender.send(Packet::Video(frame))?;
   
   // Simple receiver
   let mut receiver = Receiver::connect("192.168.1.50:9000").await?;
   while let Ok(packet) = receiver.receive().await { ... }
   ```

2. **Type Safety**
   - Strong typing for pixel formats, packet types
   - Error handling via `thiserror`
   - No unsafe code in core modules

3. **Async-First Design**
   - Built on Tokio (same as our web server)
   - Non-blocking I/O throughout
   - Efficient broadcast channel for multi-receiver

4. **Memory Efficiency**
   - Uses `bytes::Bytes` for zero-copy frame passing
   - Reusable compression/decompression buffers
   - Careful buffer management

### Considerations

1. **TCP-Only Transport**
   - Currently no QUIC support (unlike official OMT spec)
   - TCP adds latency vs UDP-based protocols
   - Acceptable for our use case (reliable delivery preferred)

2. **No Hardware Acceleration**
   - Compression/decompression is CPU-bound
   - LZ4 is very fast, so impact is minimal
   - Future enhancement possible

---

## Integration Plan for immersive-server

### Phase 1: Basic Integration (This Phase)
1. Add `aqueduct` as a dependency
2. Create `src/network/` module structure
3. Implement OMT input source type
4. Implement OMT output for compositor

### Phase 2: Discovery Integration
1. Register server as OMT source on network
2. Browse and list available OMT sources
3. UI for source selection

### Phase 3: Production Hardening
1. Auto-reconnection on disconnect
2. Performance monitoring and metrics
3. Graceful degradation on high latency

---

## Dependency Analysis

```toml
# Direct dependencies from Aqueduct
tokio = "1.36"     # ✅ Already using in immersive-server
bytes = "1.5"      # ✅ Common, lightweight
mdns-sd = "0.10"   # ⚠️ New dependency for discovery
lz4_flex = "0.12"  # ✅ Already using for Hap codec
thiserror = "1.0"  # ✅ Already using
```

**Verdict:** Minimal new dependencies, excellent compatibility with existing stack.

---

## Performance Expectations

Based on Aqueduct's architecture and LZ4 compression:

| Metric | Expected | Notes |
|--------|----------|-------|
| Latency | 1-2 frames | TCP adds ~10-20ms |
| Throughput | 4K60 | LZ4 compression very fast |
| CPU Usage | Low | Zero-copy, efficient buffers |
| Memory | Low | Bounded channel, reused buffers |

---

## Comparison with Alternatives

### vs. Official libOMT (C/C++)
| Aspect | Aqueduct | libOMT |
|--------|----------|--------|
| Language | Rust | C/C++ |
| Safety | Memory-safe | Manual management |
| Integration | Native crate | FFI bindings needed |
| QUIC Support | No | Yes |
| License | MIT | Proprietary terms |

**Verdict:** Aqueduct preferred for Rust integration; libOMT available as fallback.

### vs. NDI SDK
| Aspect | Aqueduct/OMT | NDI |
|--------|--------------|-----|
| License | Open/MIT | Proprietary |
| SDK Fee | None | None (but restricted) |
| Ecosystem | Growing | Mature |
| Performance | Comparable | Industry standard |

**Verdict:** OMT for open workflows; NDI for compatibility with existing systems.

---

## Conclusion

Aqueduct is an excellent fit for immersive-server:

1. **✅ Pure Rust** — No FFI complexity, memory safe
2. **✅ Async/Tokio** — Matches our existing async runtime
3. **✅ Well-Designed API** — Simple, ergonomic, type-safe
4. **✅ Open Source** — MIT license, no restrictions
5. **✅ Already Proven** — Working in immersive-player

**Recommendation:** Proceed with Aqueduct integration for OMT support.

---

## References

- Aqueduct source: `external_libraries/Aqueduct (Rust OMT)/`
- Official OMT: https://www.intopix.com/omt
- libOMT binaries: `external_libraries/OpenMediaTransport.Binaries.Release.v1.0.0.13/`

