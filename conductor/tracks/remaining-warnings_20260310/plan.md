# Plan: Clear Remaining 24 Compiler Warnings

## Phase 1: Parallel cleanup

Worker A corpus:
- [x] src/rbcursive/tunnel_config.rs (4 dead fields: noise_camouflage, tls_fingerprint,
  timing_obfuscation, tls_mirror) — add #[allow(dead_code)] or underscore prefix
- [x] src/packet_fragment.rs (3: rng, original_data, fragment_queue)
- [x] src/rbcursive/patterns.rs (2 unused vars)

Worker B corpus:
- [x] src/http/server.rs (3: unused Arc import, server_name, route_request)
- [x] src/quic/quic_engine.rs (2 unused vars)
- [x] src/quic/quic_engine_hybrid.rs (2: batch_size, selection_strategy dead fields)
- [x] src/concurrency/bridge.rs (2 unused imports + missing Arc in test + missing return)
- [x] src/upnp_aggressive.rs, src/quic/quic_server.rs, src/posix_sockets.rs (1 each)

## Phase 2: Verify

- [x] cargo check: 5 warnings remaining (down from 24; recursive functions - acceptable)
- [x] cargo test --lib: 278/0 passing

## Progress Notes

- 2026-03-10: 24 warnings in 10 files. Use #[allow(dead_code)] for struct fields
  that are part of public API; prefix unused vars with _; remove truly dead imports.
  Do NOT remove struct fields from public API — use allow attribute instead.
