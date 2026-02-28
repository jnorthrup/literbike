# Product Guidelines

## Brand & Voice
- **Tone:** Technical, practical, performance-oriented. Prioritize async networking efficiency over abstraction.
- **Design Philosophy:** "Form follows function." High-performance QUIC transport and networking must be the primary focus.

## User Experience (UX)
- **Modularity:** Every component (QUIC engine, FFI surfaces, protocol codecs) should be independently testable and swappable.
- **Observability:** High-fidelity logging for connection state, packet processing, and FFI boundaries.

## Technical Identity
- **Reliability:** 100% test coverage for critical paths (QUIC handshake, packet encode/decode, FFI error handling).
- **Auditability:** Every connection state change and protocol decision should be traceable in logs.
- **Performance:** Async-first design with tokio runtime for maximum throughput.
- **Interoperability:** Stable FFI boundaries for Python (PyO3) and C (ctypes) integration.

## Product Principles
- Brownfield-first: Preserve existing behavior while adding QUIC capabilities
- Additive change over large rewrites
- Interop foundations first, acceleration later
- Fallback paths must remain functional
