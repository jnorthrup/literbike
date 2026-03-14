# Spec: KMPngSCTP Integration

## Overview

Port and integrate the KMPngSCTP (Next-gen SCTP in Kotlin Multiplatform) protocol
into literbike as a first-class transport option alongside QUIC.

## Background

KMPngSCTP is a modern SCTP implementation with:
- TLV chunks everywhere (unknown = skip, Wireshark happy)
- Streams as Kotlin coroutine channels with structured scopes
- Association as coroutine scope (auto-cleanup, cancellation = FIN)
- JVM: io_uring + AF_XDP + eBPF JIT packet router
- Native: linuxPosix + DPDK-style raw sockets
- ML congestion slot (ONNX/TFLite model loader)
- Built on kotlin-spirit-parser for zero-copy TLV

## Goals

1. **Rust FFI bindings** - Create JNI bindings to call KMPngSCTP from Rust
2. **Native Rust implementation** - Port core TLV parsing and association management
3. **Transport integration** - Wire SCTP as an alternative to QUIC in the transport layer
4. **CLI support** - Add sctp-server and sctp-client commands to litebike

## Non-Goals

- Full Kotlin compatibility layer (out of scope)
- eBPF/JVM-specific features (platform-dependent)

## Key Files

- `KMPngSCTP/ngsctp/src/commonMain/kotlin/dev/jnorthrup/ngsctp/` - Core protocol
- `KMPngSCTP/docs/protocol.md` - Protocol specification
- `src/sctp/mod.rs` - Rust integration (scaffolded)

## Success Criteria

- [ ] Can establish SCTP association from litebike
- [ ] Multi-stream support works (at least 2 concurrent streams)
- [ ] TLV parser handles known + unknown chunks
- [ ] Basic congestion control functions (even if naive)
- [ ] CLI sctp-server command accepts connections