//! SCTP TLV Chunk Definitions
//!
//! Matches KMPngSCTP protocol spec from `KMPngSCTP/docs/protocol.md`
//! All chunks use Type-Length-Value format for forward compatibility.
//! Unknown chunks are automatically skipped - no parsing errors!

use thiserror::Error;

/// SCTP Chunk Types (RFC 4960 + ngSCTP extensions)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ChunkType {
    // RFC 4960 Core Types
    Data = 0x00,
    Init = 0x01,
    InitAck = 0x02,
    Sack = 0x03,
    Heartbeat = 0x04,
    HeartbeatAck = 0x05,
    Abort = 0x06,
    Shutdown = 0x07,
    ShutdownAck = 0x08,
    Error = 0x09,
    CookieEcho = 0x0A,
    CookieAck = 0x0B,
    Ecne = 0x0C,
    Cwr = 0x0D,
    ShutdownComplete = 0x0E,
    Auth = 0x0F,

    // ngSCTP Extensions
    AsconfAck = 0x80,
    Asconf = 0x81,
    ReConfig = 0x82,
    ForwardTsn = 0xC0,
    IData = 0xD0,
}

impl ChunkType {
    /// Parse chunk type from byte, returns None for unknown types
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x00 => Some(ChunkType::Data),
            0x01 => Some(ChunkType::Init),
            0x02 => Some(ChunkType::InitAck),
            0x03 => Some(ChunkType::Sack),
            0x04 => Some(ChunkType::Heartbeat),
            0x05 => Some(ChunkType::HeartbeatAck),
            0x06 => Some(ChunkType::Abort),
            0x07 => Some(ChunkType::Shutdown),
            0x08 => Some(ChunkType::ShutdownAck),
            0x09 => Some(ChunkType::Error),
            0x0A => Some(ChunkType::CookieEcho),
            0x0B => Some(ChunkType::CookieAck),
            0x0C => Some(ChunkType::Ecne),
            0x0D => Some(ChunkType::Cwr),
            0x0E => Some(ChunkType::ShutdownComplete),
            0x0F => Some(ChunkType::Auth),
            0x80 => Some(ChunkType::AsconfAck),
            0x81 => Some(ChunkType::Asconf),
            0x82 => Some(ChunkType::ReConfig),
            0xC0 => Some(ChunkType::ForwardTsn),
            0xD0 => Some(ChunkType::IData),
            _ => None, // Unknown chunk - skip it
        }
    }

    /// Check if this chunk type is known
    pub fn is_known(b: u8) -> bool {
        Self::from_byte(b).is_some()
    }
}

/// Chunk flags (bitwise)
#[derive(Debug, Clone, Copy, Default)]
pub struct ChunkFlags(pub u8);

impl ChunkFlags {
    pub const EMPTY: ChunkFlags = ChunkFlags(0);

    /// End fragment flag (E)
    pub fn is_end(&self) -> bool {
        (self.0 & 0x01) != 0
    }

    /// Beginning fragment flag (B)
    pub fn is_beginning(&self) -> bool {
        (self.0 & 0x02) != 0
    }

    /// Unordered delivery flag (U)
    pub fn is_unordered(&self) -> bool {
        (self.0 & 0x04) != 0
    }

    /// Create flags from bits
    pub fn new(bits: u8) -> Self {
        ChunkFlags(bits)
    }

    /// Set end flag
    pub fn with_end(&self) -> Self {
        ChunkFlags(self.0 | 0x01)
    }

    /// Set beginning flag
    pub fn with_beginning(&self) -> Self {
        ChunkFlags(self.0 | 0x02)
    }

    /// Set unordered flag
    pub fn with_unordered(&self) -> Self {
        ChunkFlags(self.0 | 0x04)
    }
}

/// TLV chunk header (4 bytes)
#[derive(Debug, Clone)]
pub struct ChunkHeader {
    pub chunk_type: ChunkType,
    pub flags: ChunkFlags,
    pub length: u16,
}

impl ChunkHeader {
    pub const SIZE: usize = 4;

    /// Parse header from bytes
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < Self::SIZE {
            return None;
        }

        let chunk_type = ChunkType::from_byte(data[0])?;
        let flags = ChunkFlags::new(data[1]);
        let length = u16::from_be_bytes([data[2], data[3]]);

        Some(ChunkHeader {
            chunk_type,
            flags,
            length,
        })
    }

    /// Serialize header to bytes
    pub fn serialize(&self) -> [u8; 4] {
        [
            self.chunk_type as u8,
            self.flags.0,
            (self.length >> 8) as u8,
            (self.length & 0xFF) as u8,
        ]
    }
}

/// Chunk parsing error
#[derive(Error, Debug)]
pub enum ChunkError {
    #[error("Buffer too short: need {needed} bytes, have {have}")]
    BufferTooShort { needed: usize, have: usize },

    #[error("Invalid chunk type: {0}")]
    InvalidChunkType(u8),

    #[error("Invalid chunk length: {0}")]
    InvalidLength(u16),

    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: u32, actual: u32 },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// DATA chunk (Type 0x00)
#[derive(Debug, Clone)]
pub struct DataChunk {
    pub flags: ChunkFlags,
    pub stream_id: u16,
    pub stream_seq: u16,
    pub ppid: u32,
    pub tsn: u32,
    pub payload: Vec<u8>,
}

impl DataChunk {
    /// Minimum size: header (4) + stream_id (2) + stream_seq (2) + ppid (4) + tsn (4) = 16
    pub const MIN_SIZE: usize = 16;

    pub fn chunk_type(&self) -> ChunkType {
        ChunkType::Data
    }

    pub fn parse(data: &[u8]) -> Result<Self, ChunkError> {
        if data.len() < Self::MIN_SIZE {
            return Err(ChunkError::BufferTooShort {
                needed: Self::MIN_SIZE,
                have: data.len(),
            });
        }

        let flags = ChunkFlags::new(data[1]);
        let length = u16::from_be_bytes([data[2], data[3]]) as usize;

        if data.len() < length {
            return Err(ChunkError::InvalidLength(length as u16));
        }

        let stream_id = u16::from_be_bytes([data[4], data[5]]);
        let stream_seq = u16::from_be_bytes([data[6], data[7]]);
        let ppid = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let tsn = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);

        let payload = data[16..length].to_vec();

        Ok(DataChunk {
            flags,
            stream_id,
            stream_seq,
            ppid,
            tsn,
            payload,
        })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let length = Self::MIN_SIZE + self.payload.len();
        let mut buf = Vec::with_capacity(length);

        buf.push(ChunkType::Data as u8);
        buf.push(self.flags.0);
        buf.extend_from_slice(&(length as u16).to_be_bytes());
        buf.extend_from_slice(&self.stream_id.to_be_bytes());
        buf.extend_from_slice(&self.stream_seq.to_be_bytes());
        buf.extend_from_slice(&self.ppid.to_be_bytes());
        buf.extend_from_slice(&self.tsn.to_be_bytes());
        buf.extend_from_slice(&self.payload);

        buf
    }
}

/// INIT chunk (Type 0x01)
#[derive(Debug, Clone)]
pub struct InitChunk {
    pub flags: ChunkFlags,
    pub initiate_tag: u32,
    pub a_rwnd: u32,
    pub num_outbound_streams: u16,
    pub num_inbound_streams: u16,
    pub initial_tsn: u32,
    pub params: Vec<u8>, // Optional parameters
}

impl InitChunk {
    pub const MIN_SIZE: usize = 20;

    pub fn parse(data: &[u8]) -> Result<Self, ChunkError> {
        if data.len() < Self::MIN_SIZE {
            return Err(ChunkError::BufferTooShort {
                needed: Self::MIN_SIZE,
                have: data.len(),
            });
        }

        let flags = ChunkFlags::new(data[1]);
        let length = u16::from_be_bytes([data[2], data[3]]) as usize;

        let initiate_tag = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let a_rwnd = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let num_outbound_streams = u16::from_be_bytes([data[12], data[13]]);
        let num_inbound_streams = u16::from_be_bytes([data[14], data[15]]);
        let initial_tsn = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);

        let params = if length > Self::MIN_SIZE {
            data[Self::MIN_SIZE..length].to_vec()
        } else {
            Vec::new()
        };

        Ok(InitChunk {
            flags,
            initiate_tag,
            a_rwnd,
            num_outbound_streams,
            num_inbound_streams,
            initial_tsn,
            params,
        })
    }
}

/// SACK chunk (Type 0x03)
#[derive(Debug, Clone)]
pub struct SackChunk {
    pub flags: ChunkFlags,
    pub cumulative_tsn_ack: u32,
    pub a_rwnd: u32,
    pub gap_ack_blocks: Vec<(u16, u16)>,
    pub dup_tsns: Vec<u32>,
}

impl SackChunk {
    pub const MIN_SIZE: usize = 16;

    pub fn parse(data: &[u8]) -> Result<Self, ChunkError> {
        if data.len() < Self::MIN_SIZE {
            return Err(ChunkError::BufferTooShort {
                needed: Self::MIN_SIZE,
                have: data.len(),
            });
        }

        let flags = ChunkFlags::new(data[1]);
        let cumulative_tsn_ack = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let a_rwnd = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let num_gap_ack_blocks = u16::from_be_bytes([data[12], data[13]]) as usize;
        let num_dup_tsns = u16::from_be_bytes([data[14], data[15]]) as usize;

        // Parse gap ack blocks
        let mut gap_ack_blocks = Vec::with_capacity(num_gap_ack_blocks);
        let offset = Self::MIN_SIZE;
        for i in 0..num_gap_ack_blocks {
            let start = offset + i * 4;
            if start + 4 > data.len() {
                break;
            }
            let start_ack = u16::from_be_bytes([data[start], data[start + 1]]);
            let end_ack = u16::from_be_bytes([data[start + 2], data[start + 3]]);
            gap_ack_blocks.push((start_ack, end_ack));
        }

        // Parse duplicate TSNs
        let dup_offset = offset + num_gap_ack_blocks * 4;
        let mut dup_tsns = Vec::with_capacity(num_dup_tsns);
        for i in 0..num_dup_tsns {
            let start = dup_offset + i * 4;
            if start + 4 > data.len() {
                break;
            }
            let tsn = u32::from_be_bytes([
                data[start],
                data[start + 1],
                data[start + 2],
                data[start + 3],
            ]);
            dup_tsns.push(tsn);
        }

        Ok(SackChunk {
            flags,
            cumulative_tsn_ack,
            a_rwnd,
            gap_ack_blocks,
            dup_tsns,
        })
    }
}

/// HEARTBEAT chunk (Type 0x04)
#[derive(Debug, Clone)]
pub struct HeartbeatChunk {
    pub flags: ChunkFlags,
    pub heartbeat_info: Vec<u8>,
}

impl HeartbeatChunk {
    pub const MIN_SIZE: usize = 4;

    pub fn parse(data: &[u8]) -> Result<Self, ChunkError> {
        if data.len() < Self::MIN_SIZE {
            return Err(ChunkError::BufferTooShort {
                needed: Self::MIN_SIZE,
                have: data.len(),
            });
        }

        let flags = ChunkFlags::new(data[1]);
        let length = u16::from_be_bytes([data[2], data[3]]) as usize;

        let heartbeat_info = if length > Self::MIN_SIZE {
            data[Self::MIN_SIZE..length].to_vec()
        } else {
            Vec::new()
        };

        Ok(HeartbeatChunk {
            flags,
            heartbeat_info,
        })
    }
}

/// COOKIE ECHO chunk (Type 0x0A)
#[derive(Debug, Clone)]
pub struct CookieEchoChunk {
    pub flags: ChunkFlags,
    pub cookie: Vec<u8>,
}

impl CookieEchoChunk {
    pub const MIN_SIZE: usize = 4;

    pub fn parse(data: &[u8]) -> Result<Self, ChunkError> {
        if data.len() < Self::MIN_SIZE {
            return Err(ChunkError::BufferTooShort {
                needed: Self::MIN_SIZE,
                have: data.len(),
            });
        }

        let flags = ChunkFlags::new(data[1]);
        let length = u16::from_be_bytes([data[2], data[3]]) as usize;
        let cookie = data[Self::MIN_SIZE..length].to_vec();

        Ok(CookieEchoChunk { flags, cookie })
    }
}

/// ABORT chunk (Type 0x06)
#[derive(Debug, Clone)]
pub struct AbortChunk {
    pub flags: ChunkFlags,
    pub error_causes: Vec<u8>,
}

impl AbortChunk {
    pub const MIN_SIZE: usize = 4;

    pub fn parse(data: &[u8]) -> Result<Self, ChunkError> {
        if data.len() < Self::MIN_SIZE {
            return Err(ChunkError::BufferTooShort {
                needed: Self::MIN_SIZE,
                have: data.len(),
            });
        }

        let flags = ChunkFlags::new(data[1]);
        let length = u16::from_be_bytes([data[2], data[3]]) as usize;
        let error_causes = data[Self::MIN_SIZE..length].to_vec();

        Ok(AbortChunk {
            flags,
            error_causes,
        })
    }
}

/// Generic chunk for unknown types
#[derive(Debug, Clone)]
pub struct UnknownChunk {
    pub chunk_type: u8,
    pub flags: ChunkFlags,
    pub data: Vec<u8>,
}

impl UnknownChunk {
    pub fn parse(data: &[u8]) -> Result<Self, ChunkError> {
        if data.len() < ChunkHeader::SIZE {
            return Err(ChunkError::BufferTooShort {
                needed: ChunkHeader::SIZE,
                have: data.len(),
            });
        }

        let chunk_type = data[0];
        let flags = ChunkFlags::new(data[1]);
        let length = u16::from_be_bytes([data[2], data[3]]) as usize;

        if data.len() < length {
            return Err(ChunkError::InvalidLength(length as u16));
        }

        // Skip unknown chunks per KMPngSCTP spec
        Ok(UnknownChunk {
            chunk_type,
            flags,
            data: data[ChunkHeader::SIZE..length].to_vec(),
        })
    }
}

/// Parsed chunk enum
#[derive(Debug, Clone)]
pub enum Chunk {
    Data(DataChunk),
    Init(InitChunk),
    InitAck(InitChunk),
    Sack(SackChunk),
    Heartbeat(HeartbeatChunk),
    HeartbeatAck(HeartbeatChunk),
    Abort(AbortChunk),
    CookieEcho(CookieEchoChunk),
    CookieAck(CookieEchoChunk),
    Unknown(UnknownChunk),
}

impl Chunk {
    /// Parse a chunk from raw bytes
    /// Returns None for unknown chunk types (per KMPngSCTP spec, skip unknowns)
    pub fn parse(data: &[u8]) -> Result<Option<Self>, ChunkError> {
        if data.len() < ChunkHeader::SIZE {
            return Err(ChunkError::BufferTooShort {
                needed: ChunkHeader::SIZE,
                have: data.len(),
            });
        }

        let chunk_type_byte = data[0];

        match ChunkType::from_byte(chunk_type_byte) {
            Some(ChunkType::Data) => Ok(Some(Chunk::Data(DataChunk::parse(data)?))),
            Some(ChunkType::Init) => Ok(Some(Chunk::Init(InitChunk::parse(data)?))),
            Some(ChunkType::InitAck) => Ok(Some(Chunk::InitAck(InitChunk::parse(data)?))),
            Some(ChunkType::Sack) => Ok(Some(Chunk::Sack(SackChunk::parse(data)?))),
            Some(ChunkType::Heartbeat) => Ok(Some(Chunk::Heartbeat(HeartbeatChunk::parse(data)?))),
            Some(ChunkType::HeartbeatAck) => {
                Ok(Some(Chunk::HeartbeatAck(HeartbeatChunk::parse(data)?)))
            }
            Some(ChunkType::Abort) => Ok(Some(Chunk::Abort(AbortChunk::parse(data)?))),
            Some(ChunkType::CookieEcho) => {
                Ok(Some(Chunk::CookieEcho(CookieEchoChunk::parse(data)?)))
            }
            Some(ChunkType::CookieAck) => Ok(Some(Chunk::CookieAck(CookieEchoChunk::parse(data)?))),
            _ => {
                // Unknown chunk type - skip per KMPngSCTP spec
                let unknown = UnknownChunk::parse(data)?;
                Ok(Some(Chunk::Unknown(unknown)))
            }
        }
    }

    /// Get the chunk type
    pub fn chunk_type(&self) -> Option<ChunkType> {
        match self {
            Chunk::Data(_) => Some(ChunkType::Data),
            Chunk::Init(_) => Some(ChunkType::Init),
            Chunk::InitAck(_) => Some(ChunkType::InitAck),
            Chunk::Sack(_) => Some(ChunkType::Sack),
            Chunk::Heartbeat(_) => Some(ChunkType::Heartbeat),
            Chunk::HeartbeatAck(_) => Some(ChunkType::HeartbeatAck),
            Chunk::Abort(_) => Some(ChunkType::Abort),
            Chunk::CookieEcho(_) => Some(ChunkType::CookieEcho),
            Chunk::CookieAck(_) => Some(ChunkType::CookieAck),
            Chunk::Unknown(_) => None,
        }
    }
}

/// Chunk parser for reading multiple chunks from a packet
pub struct ChunkParser;

impl ChunkParser {
    /// Parse all chunks from a packet buffer
    /// Unknown chunks are skipped per KMPngSCTP spec
    pub fn parse_all(data: &[u8]) -> Result<Vec<Chunk>, ChunkError> {
        let mut chunks = Vec::new();
        let mut offset = 0;

        while offset < data.len() {
            if offset + ChunkHeader::SIZE > data.len() {
                break;
            }

            // Read length from header
            let length = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
            if length == 0 {
                break; // Invalid length
            }

            // Pad to 4-byte boundary
            let padded_length = (length + 3) & !3;

            if offset + padded_length > data.len() {
                return Err(ChunkError::BufferTooShort {
                    needed: offset + padded_length,
                    have: data.len(),
                });
            }

            let chunk_data = &data[offset..offset + length];
            if let Some(chunk) = Chunk::parse(chunk_data)? {
                chunks.push(chunk);
            }

            offset += padded_length;
        }

        Ok(chunks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_type_from_byte() {
        assert_eq!(ChunkType::from_byte(0x00), Some(ChunkType::Data));
        assert_eq!(ChunkType::from_byte(0x01), Some(ChunkType::Init));
        assert_eq!(ChunkType::from_byte(0xFF), None); // Unknown
    }

    #[test]
    fn test_chunk_flags() {
        let flags = ChunkFlags::new(0x05);
        assert!(flags.is_end());
        assert!(!flags.is_beginning());
        assert!(flags.is_unordered());
    }

    #[test]
    fn test_data_chunk_serialize_parse() {
        let chunk = DataChunk {
            flags: ChunkFlags::EMPTY.with_beginning().with_end(),
            stream_id: 1,
            stream_seq: 0,
            ppid: 0,
            tsn: 1,
            payload: b"hello".to_vec(),
        };

        let serialized = chunk.serialize();
        let parsed = DataChunk::parse(&serialized).unwrap();

        assert_eq!(parsed.stream_id, 1);
        assert_eq!(parsed.payload, b"hello");
    }

    #[test]
    fn test_unknown_chunk_skip() {
        // Unknown chunk type 0xFF
        let data = [0xFF, 0x00, 0x00, 0x08, 0xDE, 0xAD, 0xBE, 0xEF];

        if let Ok(Some(Chunk::Unknown(unknown))) = Chunk::parse(&data) {
            assert_eq!(unknown.chunk_type, 0xFF);
            assert_eq!(unknown.data, &[0xDE, 0xAD, 0xBE, 0xEF]);
        } else {
            panic!("Expected unknown chunk");
        }
    }
}
