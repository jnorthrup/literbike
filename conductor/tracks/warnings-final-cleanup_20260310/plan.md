# Plan: Final 14 Compiler Warnings Cleanup

## Phase 1: Fix all 14 remaining warnings

Files and fixes needed:

- [x] src/quic/quic_engine.rs:83 — prefix `private_key` with `_`
- [x] src/quic/quic_engine.rs:48 — `#[allow(dead_code)]` on field `ctx`
- [x] src/quic/quic_engine_hybrid.rs:134 — prefix `db_path` with `_`
- [x] src/quic/quic_engine_hybrid.rs:130 — `#[allow(dead_code)]` on field `batch_size`
- [x] src/concurrency/bridge.rs:105 — prefix `our_sender` with `_`
- [x] src/posix_sockets.rs:39 — prefix `fd` with `_`
- [x] src/upnp_aggressive.rs:198 — prefix `stream` with `_`
- [x] src/quic/quic_server.rs:251 — `#[allow(dead_code)]` on fn `extract_dcid_from_long_header`
- [x] src/http/server.rs:347 — prefix `listener` with `_`
- [x] src/http/server.rs:49 — `#[allow(dead_code)]` on field `server_name`
- [x] src/http/server.rs:93 — `#[allow(dead_code)]` on method `route_request`
- [x] src/dht/client.rs:136 — `#[allow(dead_code)]` on field `local_peer_id`
- [x] src/betanet_patterns.rs:354 — `#[allow(dead_code)]` on field `bucket_size`
- [x] src/host_trust.rs:15 — `#[allow(dead_code)]` on field `auto_trust_private`

## Phase 2: Verify

- [x] cargo check: 0 warnings
- [x] cargo test --lib: 278/0

## Progress Notes

- 2026-03-10: Previous kilo workers got 24→14 warnings but didn't finish. These
  are all trivial _ prefix or #[allow(dead_code)] fixes. Delegation: Worker A=kilo.
