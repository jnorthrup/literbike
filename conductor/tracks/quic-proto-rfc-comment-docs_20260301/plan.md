# Plan: QUIC Proto RFC Comment-Docs Discipline

## Phase 1: Rule Definition and Stanza Inventory

- [x] Create track scope and acceptance criteria for RFC-mapped stanza documentation
- [ ] Enumerate protocol stanzas in `src/quic/quic_protocol.rs`
- [ ] Enumerate protocol-critical TLS/packet stanzas in `src/quic/quic_engine.rs` and `src/quic/quic_server.rs`

## Phase 2: Inline RFC Anchors

- [x] Add `RFC-TRACE` anchors in `src/quic/quic_protocol.rs` wire codec paths
- [ ] Add `RFC-TRACE` anchors in `src/quic/quic_engine.rs` for packet assembly/protection/send logic
- [ ] Add `RFC-TRACE` anchors in `src/quic/quic_server.rs` for decrypt/parse/dispatch logic

## Phase 3: Comment-Docs Index

- [x] Create `docs/QUIC_RFC_COMMENT_DOCS.md`
- [ ] Complete stanza-to-RFC coverage table for all in-scope files
- [ ] Add maintenance rules for future protocol edits

## Phase 4: Enforcement and Validation

- [ ] Add CI/verification step (or scripted check) for missing `RFC-TRACE` anchors in changed protocol stanzas
- [ ] Validate build/tests after annotation pass
- [ ] Document residual mapping gaps and assign follow-up tasks

## Status Notes

- This track is active as a course correction to preserve protocol intent during high-velocity debugging.
