//! SCTP Chunk - protocol messages
//!
//! This module CANNOT see association or stream.

use ccek_core::{Element, Key};
use std::any::{Any, TypeId};

/// ChunkKey - SCTP chunk processing
pub struct ChunkKey;

impl ChunkKey {
    pub const FACTORY: fn() -> ChunkElement = || ChunkElement::new();
}

impl Key for ChunkKey {
    type Element = ChunkElement;
    const FACTORY: fn() -> Self::Element = ChunkKey::FACTORY;
}

/// ChunkElement - chunk registry
pub struct ChunkElement;

impl ChunkElement {
    pub fn new() -> Self {
        Self
    }
}

impl Element for ChunkElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<ChunkKey>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// SCTP chunk header
#[derive(Debug, Clone)]
pub struct ChunkHeader {
    pub chunk_type: u8,
    pub flags: u8,
    pub length: u16,
}

/// SCTP DATA chunk
#[derive(Debug, Clone)]
pub struct DataChunk {
    pub header: ChunkHeader,
    pub tsn: u32,
    pub stream_id: u16,
    pub stream_seq: u16,
    pub payload_proto: u32,
    pub data: Vec<u8>,
}

/// SCTP INIT chunk
#[derive(Debug, Clone)]
pub struct InitChunk {
    pub header: ChunkHeader,
    pub initiate_tag: u32,
    pub advertised_window: u32,
    pub num_out_streams: u16,
    pub max_in_streams: u16,
    pub initial_tsn: u32,
}

/// SCTP SACK chunk
#[derive(Debug, Clone)]
pub struct SackChunk {
    pub header: ChunkHeader,
    pub cumulative_tsn: u32,
    pub advertised_window: u32,
    pub num_gaps: u16,
    pub num_dup_tsns: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_factory() {
        let _elem = ChunkKey::FACTORY();
        // Just verify it constructs
        assert!(true);
    }
}
