# Plan: KMPngSCTP Integration

## Scope

Port KMPngSCTP's modern SCTP implementation into literbike Rust, providing
an alternative transport to QUIC with multi-stream, multi-homing, and PR-SCTP
support.

## Phase 1: Protocol Foundation

- [ ] Study `KMPngSCTP/docs/protocol.md` for TLV chunk format
- [ ] Port TLV chunk definitions from `ngsctp/src/commonMain/kotlin/dev/jnorthrup/ngsctp/chunks/`
- [ ] Implement chunk parser in `src/sctp/chunks.rs`
- [ ] Add unit tests for each chunk type

## Phase 2: Association Management

- [ ] Port `NgSctpAssociation.kt` patterns to `src/sctp/association.rs`
- [ ] Implement association state machine (CLOSED → COOKIE_WAIT → ESTABLISHED)
- [ ] Addmulti-stream support (openStream, sendChannel, receiveChannel)
- [ ] Handle graceful shutdown (FIN exchange)

## Phase 3: Transport Integration

- [ ] Wire SCTP into `src/syscall_net` as alternative to UD P/QUIC
- [ ] Add `SctpListener` matching `QuicServer` pattern
- [ ] Implement `SctpConnection` trait for async read/write
- [ ] Add feature flag `sctp` (already in Cargo.toml)

## Phase 4: CLI Support

- [ ] Verify `sctp-server` command works (already added)
- [ ] Add `sctp-client` command for testing
- [ ] Add `sctp-stats` command for diagnostics

## Phase 5: Testing

- [ ] Unit tests for chunk parsing
- [ ] Integration tests for association lifecycle
- [ ] Multi-stream concurrency tests
- [ ] Compare throughput vs QUIC

## Progress Notes

- 2026-03-14: Track created. Basic scaffold `src/sctp/mod.rs` exists with
  structs for SctpServer, SctpClient, SctpAssociation, SctpStream.
- 2026-03-14: `sctp` feature flag added to Cargo.toml.
- 2026-03-14: `sctp-server` command added to litebike CLI.