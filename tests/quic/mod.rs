//! QUIC Protocol Layer Tests
//!
//! Phase 1: Foundation - QUIC Protocol Layer Tests (Items 1-12)
//! Phase 2: QUIC Engine Layer Tests (Items 13-24)
//! Phase 5: QUIC Stream Ingestion Tests (Items 53-64)
//! Phase 6: Channelized Distribution Tests (Items 65-72)
//!
//! Tests for:
//! - Packet serialization/deserialization
//! - Connection state machine
//! - Stream management
//! - ACK generation
//! - Crypto frame handling
//! - Stream ingestion pipeline
//! - Channel distribution

mod test_packet_serialization;
mod test_connection_state;
mod test_frame_types;
mod test_protocol_validation;
mod test_engine;
mod test_stream_ingestion;
mod test_channel_distribution;
mod lifecycle;
