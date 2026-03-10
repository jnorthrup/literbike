# Plan: QUIC Proto RFC Comment-Docs Discipline

## Phase 1: Rule Definition and Stanza Inventory

- [x] Create track scope and acceptance criteria for RFC-mapped stanza documentation
- [x] Enumerate protocol stanzas in `src/quic/quic_protocol.rs`
- [x] Enumerate protocol-critical TLS/packet stanzas in `src/quic/quic_engine.rs` and `src/quic/quic_server.rs`

## Phase 2: Inline RFC Anchors

- [x] Add `RFC-TRACE` anchors in `src/quic/quic_protocol.rs` wire codec paths
- [x] Add `RFC-TRACE` anchors in `src/quic/quic_engine.rs` for packet assembly/protection/send logic
- [x] Add `RFC-TRACE` anchors in `src/quic/quic_server.rs` for decrypt/parse/dispatch logic

## Phase 3: Comment-Docs Index

- [x] Create `docs/QUIC_RFC_COMMENT_DOCS.md`
- [x] Complete stanza-to-RFC coverage table for all in-scope files (QP-001..QP-018, QE-001..QE-014, QS-001..QS-011)
- [x] Add maintenance rules for future protocol edits

## Phase 4: Enforcement and Validation

- [x] Add CI/verification step (or scripted check) for missing `RFC-TRACE` anchors in changed protocol stanzas
- [x] Validate build/tests after annotation pass (build/tests confirmed passing 2026-03-09)
- [x] Document residual mapping gaps and assign follow-up tasks

## Status Notes

- **PHASE 2 COMPLETE (2026-03-08)**: All RFC anchor annotations added to quic_server.rs decrypt/parse/dispatch stanzas. Validated with cargo build/test.
- **2026-03-09**: Phase 4 closed. check_rfc_trace.sh added; quic_engine.rs RFC anchor style migration tracked as follow-up.
- This track is active as a course correction to preserve protocol intent during high-velocity debugging.
