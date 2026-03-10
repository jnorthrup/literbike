# QUIC RFC Comment-Docs Index

This file is the source-of-truth index for `RFC-TRACE` annotations in QUIC code.

## Scope

This index covers RFC-TRACE annotations across three core QUIC modules:

- **src/quic/quic_protocol.rs**: Wire codec wire-format stanzas (packet/frame serialization/deserialization)
- **src/quic/quic_engine.rs**: Packet assembly, protection, and send logic
- **src/quic/quic_server.rs**: Decrypt, parse, and dispatch logic for received packets

Rule: every protocol-critical stanza includes an `RFC-TRACE [RFCxxxx§y.z]` anchor adjacent to the code.

## Stanza Map

| Stanza | Code Location | RFC Anchor | Excerpt Summary |
| --- | --- | --- | --- |
| QP-001 | `validate_stream_id` | RFC 9000 §2.1 | Stream IDs are 62-bit values; out-of-range IDs are invalid. |
| QP-002 | `serialize_packet` | RFC 9000 §17 | Packet encoding branches by long vs short header form and packet type. |
| QP-003 | `serialize_short_header_packet` | RFC 9000 §17.3 | Short header carries fixed bit, DCID, packet number, protected payload. |
| QP-004 | `serialize_long_header_packet` | RFC 9000 §17.2 | Long header carries version, DCID/SCID, and type-specific fields. |
| QP-005 | `serialize_long_header_packet` token stanza | RFC 9000 §17.2.2 | Initial packets include token length and token bytes. |
| QP-006 | `deserialize_long_header_packet` | RFC 9000 §17.2 | Parse and validate long-header version/type/length structure. |
| QP-007 | `deserialize_short_header_packet` | RFC 9000 §17.3 | Short header omits CID length; endpoint context provides DCID length. |
| QP-008 | `encode_frames` / `decode_frames` loop | RFC 9000 §12.4 | Packet payload is an ordered sequence of frames. |
| QP-009 | `encode_frame` PADDING/PING | RFC 9000 §19.1-§19.2 | PADDING and PING frame wire formats. |
| QP-010 | `encode_frame` CRYPTO | RFC 9000 §19.6, RFC 9001 §4 | TLS handshake data is carried in CRYPTO frames by encryption level. |
| QP-011 | `encode_frame` STREAM | RFC 9000 §19.8 | STREAM type bits encode FIN/LEN/OFF and carry stream metadata/data. |
| QP-012 | `encode_ack_frame` | RFC 9000 §19.3, §19.3.1 | ACK uses largest-acked and descending ACK range encoding with gaps. |
| QP-013 | `decode_frames` ACK/ACK_ECN | RFC 9000 §19.3, §19.3.2 | ACK and ACK_ECN range decoding and ECN counter fields. |
| QP-014 | `decode_frames` flow-control frames | RFC 9000 §19.9-§19.14 | MAX_* / *_BLOCKED flow-control frame formats. |
| QP-015 | `decode_frames` connection-id/path frames | RFC 9000 §19.15-§19.18 | NEW/RETIRE_CONNECTION_ID and PATH_CHALLENGE/RESPONSE formats. |
| QP-016 | `decode_frames` CONNECTION_CLOSE | RFC 9000 §19.19 | Transport/application close frame variants and reason payload. |
| QP-017 | `encode_packet_number` / `read_packet_number` | RFC 9000 §17.1 | Packet number truncation is 1-4 bytes on wire. |
| QP-018 | `write_varint` / `read_varint` | RFC 9000 §16 | QUIC varint length prefix and value encoding/decoding. |

## Engine Stanza Map (quic_engine.rs)

| Stanza | Code Location | RFC Anchor | Excerpt Summary |
| --- | --- | --- | --- |
| QE-001 | `encode_server_transport_params` | RFC 9000 §18 | Encodes minimal server transport parameters (idle timeout, max data, stream limits, connection IDs). |
| QE-002 | `expected_inbound_packet_number` | RFC 9000 §17.1 | Returns the expected next inbound packet number for use in truncated PN reconstruction. |
| QE-003 | `infer_packet_number_len` | RFC 9000 §17.1 | Infers the encoded byte length (1–4) of a truncated packet number from its value. |
| QE-004 | `reconstruct_packet_number` | RFC 9000 §17.1 | Reconstructs the full 62-bit packet number from a truncated wire encoding and the expected PN. |
| QE-005 | `apply_outbound_header_protection_hook` | RFC 9001 §5.4 | Applies outbound header protection by invoking the crypto provider hook on the packet header. |
| QE-006 | inbound header protection removal in `process_packet_internal` | RFC 9001 §5.4 | Removes inbound header protection via the crypto provider before packet number reconstruction. |
| QE-007 | stream state transitions in `send_stream_data_with_fin` | RFC 9000 §3.1 | Advances stream state (Idle→Open→HalfClosedLocal→Closed) when data or FIN is queued for send. |
| QE-008 | STREAM frame type-byte construction in `send_stream_data_with_fin` | RFC 9000 §19.8 | Sets OFF/LEN/FIN bits (0x04/0x02/0x01) in the STREAM frame type byte based on offset and fin flag. |
| QE-009 | long-header packet type selection in `send_handshake_responses` | RFC 9000 §17.2, §17.2.4 | Selects Initial (0x00) or Handshake (0x02) type bits for long-header packet first byte. |
| QE-010 | CRYPTO frame offset accounting in `send_handshake_responses` | RFC 9000 §19.6 | Tracks and increments the per-level monotonically increasing CRYPTO frame send offset. |
| QE-011 | Initial packet token field in `send_handshake_responses` | RFC 9000 §17.2.2 | Writes token length = 0 into the long-header Initial packet when no retry token is present. |
| QE-012 | AEAD encryption in `send_handshake_responses` | RFC 9001 §5.3 | Encrypts the handshake packet body with the per-level AEAD key before header protection. |
| QE-013 | HANDSHAKE_DONE frame in `send_handshake_responses` | RFC 9000 §19.20 | Emits HANDSHAKE_DONE frame byte (0x1e) in a 1-RTT packet after handshake completion. |
| QE-014 | AEAD encryption in `send_encrypted_frames` | RFC 9001 §5.3 | Encrypts non-1-RTT (Initial/Handshake) frame payloads with the level-appropriate AEAD key. |
| QE-015 | `send_1rtt_frames` | RFC 9000 §17.3 | Assembles and sends a 1-RTT short-header packet: first byte, DCID, PN, encrypted payload. |
| QE-016 | DCID selection in `send_1rtt_frames` | RFC 9000 §17.3 | Server→client short-header DCID is set to the client's SCID stored as remote_connection_id. |
| QE-017 | AEAD encryption in `send_1rtt_frames` | RFC 9001 §5.3 | Encrypts the 1-RTT payload with the OneRtt-level AEAD key and appends the 16-byte auth tag. |
| QE-018 | stream state transitions in `send_stream_frame` | RFC 9000 §3.1 | Advances stream state (Idle→Open→HalfClosedLocal→Closed) after successful 1-RTT send. |
| QE-019 | `process_stream_frame` | RFC 9000 §3.2 | Buffers out-of-order STREAM frame data and reassembles in-order bytes for delivery. |
| QE-020 | stream state transitions in `process_stream_frame` | RFC 9000 §3.1 | Advances receive-side stream state (Idle→Open→HalfClosedRemote→Closed) on FIN receipt. |
| QE-021 | `create_ack_packet` | RFC 9000 §19.3 | Assembles an ACK frame from pending packet numbers, sorted and deduplicated before encoding. |
| QE-022 | ACK range coalescence in `create_ack_packet` | RFC 9000 §19.3.1 | Coalesces contiguous packet numbers into ACK ranges and records gap boundaries between them. |

## Server Stanza Map (quic_server.rs)

| Stanza | Code Location | RFC Anchor | Excerpt Summary |
| --- | --- | --- | --- |
| QS-001 | `encode_varint` | RFC 9000 §16 | Encodes a QUIC variable-length integer (1/2/4/8 bytes) from a u64 value. |
| QS-002 | `extract_dcid_from_long_header` | RFC 9000 §17.2 (§5.2 in RFC-TRACE) | Extracts the DCID bytes from a long-header packet for connection routing without decryption. |
| QS-003 | `try_decrypt_initial_packet` | RFC 9001 §5.2, §7 (RFC-TRACE §5.2, §7) | Decrypts a QUIC Initial packet using RFC 9001 HKDF-derived initial secrets from the client DCID. |
| QS-004 | `read_varint` | RFC 9000 §16 | Minimal QUIC varint decoder: reads prefix length bits and returns (value, bytes_consumed). |
| QS-005 | `try_decrypt_1rtt_packet` | RFC 9001 §5.3, §9 (RFC-TRACE §5.3, §9) | Removes header protection and AEAD-decrypts a 1-RTT short-header packet using OneRtt-level keys. |
| QS-006 | `try_decrypt_handshake_packet` | RFC 9001 §5.1, §9 (RFC-TRACE §5.1, §9) | Removes header protection and AEAD-decrypts a Handshake long-header packet using handshake_remote keys. |
| QS-007 | coalesced packet split in `start` | RFC 9000 §12.2 (RFC-TRACE §12.2) | Splits a UDP datagram into individual QUIC packet slices to process each coalesced packet in order. |
| QS-008 | packet type dispatch in `start` | RFC 9000 §17 (RFC-TRACE §5, §5.1, §5.2, §5.3) | Routes each packet slice to the correct decrypt path based on long/short header and type bits. |
| QS-009 | connection ID extraction in `start` | RFC 9000 §5.1 (RFC-TRACE §7.6, §3.2) | Extracts client SCID and DCID from the decoded packet header to initialize or locate the connection engine. |
| QS-010 | stream frame dispatch in `start` | RFC 9000 §3.4 (RFC-TRACE §3.4) | After engine processing, iterates received STREAM frames and routes request streams to response logic. |

## Maintenance Rule

When adding or modifying a QUIC wire-format stanza:

1. Add or update an in-code `RFC-TRACE [RFCxxxx§...]` comment adjacent to the stanza.
2. Add or update the corresponding row in this table.
3. Keep frame names and packet field names aligned with RFC terminology.

## Residual Gaps

- **quic_engine.rs anchor style**: `quic_engine.rs` currently uses bare `// RFC 9000 §` comments rather than the canonical `RFC-TRACE [RFC9000§...]` tag format established in `quic_protocol.rs`. The tool `tools/check_rfc_trace.sh` counts both styles, so coverage is tracked, but a future pass should migrate the bare comments to the tagged format so they are tool-parseable without the `RFC 9` fallback heuristic.

- **quic_session_cache.rs (new as of 2026-03-09)**: This module has no RFC anchors. Session cache semantics map loosely to RFC 9000 §7.3 (connection migration / address validation), RFC 9001 §4.6.1 (0-RTT), and RFC 9001 §4.6.2 (session tickets). Anchor addition is deferred to a follow-up pass.
