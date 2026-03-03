# QUIC RFC Comment-Docs Index

This file is the source-of-truth index for `RFC-TRACE` annotations in QUIC code.

## Scope

- Module: `src/quic/quic_protocol.rs`
- Rule: every wire-format stanza includes an `RFC-TRACE [RFCxxxx§y.z]` anchor.

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

## Maintenance Rule

When adding or modifying a QUIC wire-format stanza:

1. Add or update an in-code `RFC-TRACE [RFCxxxx§...]` comment adjacent to the stanza.
2. Add or update the corresponding row in this table.
3. Keep frame names and packet field names aligned with RFC terminology.
