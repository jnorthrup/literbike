# Spec: QUIC Proto RFC Comment-Docs Discipline

## Overview

Establish a hard documentation discipline for QUIC protocol code: every wire-level
stanza must map to an RFC section anchor in code comments and to an indexed
comment-doc artifact.

This track is a quality-control course correction for debugging and interop work.

## Problem

- QUIC/TLS/HTTP3 behavior is currently hard to audit under pressure.
- Protocol code and RFC requirements are not consistently cross-linked.
- Debug sessions lose context because rationale is not attached to code stanzas.

## Goals

- Make protocol intent local to code via explicit RFC anchors.
- Keep a single comment-doc index that maps stanzas to RFC anchors.
- Require new protocol code to ship with RFC mappings before merge.

## Functional Requirements

1. QUIC protocol wire codec stanzas in:
   - `/Users/jim/work/literbike/src/quic/quic_protocol.rs`
   must include inline `RFC-TRACE [RFCxxxx§...]` anchors.

2. A comment-doc index must exist at:
   - `/Users/jim/work/literbike/docs/QUIC_RFC_COMMENT_DOCS.md`
   mapping stanza identifiers to code locations and RFC anchors.

3. The same discipline must be extended to protocol-critical send/decrypt paths in:
   - `/Users/jim/work/literbike/src/quic/quic_engine.rs`
   - `/Users/jim/work/literbike/src/quic/quic_server.rs`

4. PR/workflow rule: protocol changes are incomplete without comment-doc mapping.

## Acceptance Criteria

1. `quic_protocol.rs` has RFC-anchored comments for all packet/frame encode/decode stanzas.
2. `docs/QUIC_RFC_COMMENT_DOCS.md` indexes all anchored stanzas with maintainable identifiers.
3. `quic_engine.rs` and `quic_server.rs` critical TLS/packet-protection stanzas are RFC-anchored.
4. Track plan documents enforcement steps and validation checks.

## Out of Scope

- Proving full RFC conformance of implementation behavior.
- Rewriting architecture solely for docs.
- Non-QUIC modules outside transport/protocol paths.
