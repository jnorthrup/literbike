//! SCTP Chunk Types - TLV format per ngSCTP protocol specification
//!
//! All chunks use Type-Length-Value format for forward compatibility.
//! Unknown chunks are automatically skipped.

use std::io::{self, Read, Cursor};

/// SCTP chunk type identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ChunkType {
    /// User data (0x00)
    Data = 0x00,
    /// Initialize association (0x01)
    Init = 0x01,
    /// Initialize acknowledgment (0x02)
    InitAck = 0x02,
    /// Selective acknowledgment (0x03)
    Sack = 0x03,
    /// Heartbeat request (0x04)
    Heartbeat = 0x04,
    /// Heartbeat acknowledgment (0x05)
    HeartbeatAck = 0x05,
    /// Abort association (0x06)
    Abort = 0x06,
    /// Shutdown (0x07)
    Shutdown = 0x07,
    /// Shutdown acknowledgment (0x08)
    ShutdownAck = 0x08,
    /// Error indication (0x09)
    Error = 0x09,
    /// State cookie echo (0x0A)
    CookieEcho = 0x0A,
    /// State cookie acknowledgment (0x0B)
    CookieAck = 0x0B,
    /// Explicit congestion notification (0x0C)
    Ecne = 0x0C,
    /// Congestion window reduced (0x0D)
    Cwr = 0x0D,
    /// Shutdown complete (0x0E)
    ShutdownComplete = 0x0E,
}

impl ChunkType {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(Self::Data),
            0x01 => Some(Self::Init),
            0x02 => Some(Self::InitAck),
            0x03 => Some(Self::Sack),
            0x04 => Some(Self::Heartbeat),
            0x05 => Some(Self::HeartbeatAck),
            0x06 => Some(Self::Abort),
            0x07 => Some(Self::Shutdown),
            0x08 => Some(Self::ShutdownAck),
            0x09 => Some(Self::Error),
            0x0A => Some(Self::CookieEcho),
            0x0B => Some(Self::CookieAck),
            0x0C => Some(Self::Ecne),
            0x0D => Some(Self::Cwr),
            0x0E => Some(Self::ShutdownComplete),
            _ => None,
        }
    }
}

/// DATA chunk flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataFlags(pub u8);

impl DataFlags {
    /// End of fragment
    pub const END: u8 = 0x01;
    /// Beginning of fragment
    pub const BEGIN: u8 = 0x02;
    /// Unordered delivery
    pub const UNORDERED: u8 = 0x04;

    pub fn new(flags: u8) -> Self {
        Self(flags)
    }

    pub fn is_end(self) -> bool {
        self.0 & Self::END != 0
    }

    pub fn is_begin(self) -> bool {
        self.0 & Self::BEGIN != 0
    }

    pub fn is_unordered(self) -> bool {
        self.0 & Self::UNORDERED != 0
    }
}

/// SCTP chunk header (4 bytes)
#[derive(Debug, Clone)]
pub struct ChunkHeader {
    pub chunk_type: u8,
    pub flags: u8,
    pub length: u16,
}

impl ChunkHeader {
    const SIZE: usize = 4;

    pub fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        if bytes.len() < Self::SIZE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "chunk header too short",
            ));
        }
        Ok(Self {
            chunk_type: bytes[0],
            flags: bytes[1],
            length: u16::from_be_bytes([bytes[2], bytes[3]]),
        })
    }

    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        [
            self.chunk_type,
            self.flags,
            self.length.to_be_bytes()[0],
            self.length.to_be_bytes()[1],
        ]
    }

    pub fn value_length(&self) -> usize {
        self.length.saturating_sub(Self::SIZE as u16) as usize
    }
}

/// DATA chunk (0x00)
#[derive(Debug, Clone)]
pub struct DataChunk {
    pub flags: DataFlags,
    pub stream_id: u16,
    pub stream_seq_num: u16,
    pub payload_protocol_id: u32,
    pub transmission_seq_num: u32,
    pub user_data: Vec<u8>,
}

impl DataChunk {
    /// Parse DATA chunk from bytes
    pub fn from_bytes(mut bytes: &[u8]) -> io::Result<Self> {
        let header = ChunkHeader::from_bytes(bytes)?;
        bytes = &bytes[ChunkHeader::SIZE..];

        if bytes.len() < 16 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "DATA chunk too short",
            ));
        }

        let stream_id = u16::from_be_bytes([bytes[0], bytes[1]]);
        let stream_seq_num = u16::from_be_bytes([bytes[2], bytes[3]]);
        let payload_protocol_id = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let transmission_seq_num =
            u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let user_data = bytes[12..].to_vec();

        Ok(Self {
            flags: DataFlags::new(header.flags),
            stream_id,
            stream_seq_num,
            payload_protocol_id,
            transmission_seq_num,
            user_data,
        })
    }

    /// Serialize DATA chunk to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let value_len = 16 + self.user_data.len();
        let length = (ChunkHeader::SIZE + value_len) as u16;

        let mut bytes = Vec::with_capacity(length as usize);
        bytes.push(ChunkType::Data as u8);
        bytes.push(self.flags.0);
        bytes.extend_from_slice(&length.to_be_bytes());
        bytes.extend_from_slice(&self.stream_id.to_be_bytes());
        bytes.extend_from_slice(&self.stream_seq_num.to_be_bytes());
        bytes.extend_from_slice(&self.payload_protocol_id.to_be_bytes());
        bytes.extend_from_slice(&self.transmission_seq_num.to_be_bytes());
        bytes.extend_from_slice(&self.user_data);

        // Pad to 4-byte boundary
        while bytes.len() % 4 != 0 {
            bytes.push(0);
        }

        bytes
    }
}

/// INIT chunk (0x01)
#[derive(Debug, Clone)]
pub struct InitChunk {
    pub initiate_tag: u32,
    pub advertised_receiver_window_credit: u32,
    pub outbound_streams: u16,
    pub inbound_streams: u16,
    pub initial_tsn: u32,
    pub parameters: Vec<InitParam>,
}

/// INIT parameters (TLV)
#[derive(Debug, Clone)]
pub struct InitParam {
    pub param_type: u16,
    pub value: Vec<u8>,
}

impl InitChunk {
    pub fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        let header = ChunkHeader::from_bytes(bytes)?;
        let mut cursor = Cursor::new(&bytes[ChunkHeader::SIZE..]);

        let mut initiate_tag = [0u8; 4];
        cursor.read_exact(&mut initiate_tag)?;
        let initiate_tag = u32::from_be_bytes(initiate_tag);

        let mut arwc = [0u8; 4];
        cursor.read_exact(&mut arwc)?;
        let advertised_receiver_window_credit = u32::from_be_bytes(arwc);

        let mut outbound_streams = [0u8; 2];
        cursor.read_exact(&mut outbound_streams)?;
        let outbound_streams = u16::from_be_bytes(outbound_streams);

        let mut inbound_streams = [0u8; 2];
        cursor.read_exact(&mut inbound_streams)?;
        let inbound_streams = u16::from_be_bytes(inbound_streams);

        let mut initial_tsn = [0u8; 4];
        cursor.read_exact(&mut initial_tsn)?;
        let initial_tsn = u32::from_be_bytes(initial_tsn);

        // Parse parameters (skip for now, would need full TLV parsing)
        let parameters = Vec::new();

        Ok(Self {
            initiate_tag,
            advertised_receiver_window_credit,
            outbound_streams,
            inbound_streams,
            initial_tsn,
            parameters,
        })
    }
}

/// SACK chunk (0x03) - Selective Acknowledgment
#[derive(Debug, Clone)]
pub struct SackChunk {
    pub cumulative_tsn_ack: u32,
    pub a_rwnd: u32,
    pub gap_ack_blocks: Vec<GapAckBlock>,
    pub duplicate_tsn: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct GapAckBlock {
    pub start: u16,
    pub end: u16,
}

/// Generic SCTP chunk
#[derive(Debug, Clone)]
pub enum Chunk {
    Data(DataChunk),
    Init(InitChunk),
    InitAck(InitChunk),
    Sack(SackChunk),
    Heartbeat,
    HeartbeatAck,
    Abort,
    Shutdown,
    ShutdownAck,
    Error,
    CookieEcho(Vec<u8>),
    CookieAck,
    Ecne,
    Cwr,
    ShutdownComplete,
    Unknown { chunk_type: u8, data: Vec<u8> },
}

impl Chunk {
    pub fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        if bytes.is_empty() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "empty chunk"));
        }

        let chunk_type = bytes[0];
        let parsed = match ChunkType::from_u8(chunk_type) {
            Some(ChunkType::Data) => Chunk::Data(DataChunk::from_bytes(bytes)?),
            Some(ChunkType::Init) => Chunk::Init(InitChunk::from_bytes(bytes)?),
            Some(ChunkType::InitAck) => Chunk::InitAck(InitChunk::from_bytes(bytes)?),
            Some(ChunkType::Sack) => {
                // SACK parsing - skip detailed parsing for now
                Chunk::Sack(SackChunk {
                    cumulative_tsn_ack: 0,
                    a_rwnd: 0,
                    gap_ack_blocks: Vec::new(),
                    duplicate_tsn: Vec::new(),
                })
            }
            Some(ChunkType::Heartbeat) => Chunk::Heartbeat,
            Some(ChunkType::HeartbeatAck) => Chunk::HeartbeatAck,
            Some(ChunkType::Abort) => Chunk::Abort,
            Some(ChunkType::Shutdown) => Chunk::Shutdown,
            Some(ChunkType::ShutdownAck) => Chunk::ShutdownAck,
            Some(ChunkType::Error) => Chunk::Error,
            Some(ChunkType::CookieEcho) => {
                let data = bytes.get(4..).unwrap_or(&[]).to_vec();
                Chunk::CookieEcho(data)
            }
            Some(ChunkType::CookieAck) => Chunk::CookieAck,
            Some(ChunkType::Ecne) => Chunk::Ecne,
            Some(ChunkType::Cwr) => Chunk::Cwr,
            Some(ChunkType::ShutdownComplete) => Chunk::ShutdownComplete,
            None => Chunk::Unknown {
                chunk_type,
                data: bytes.to_vec(),
            },
        };

        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_header_roundtrip() {
        let header = ChunkHeader {
            chunk_type: 0x00,
            flags: 0x06, // BEGIN + END
            length: 20,
        };
        let bytes = header.to_bytes();
        let parsed = ChunkHeader::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.chunk_type, 0x00);
        assert_eq!(parsed.flags, 0x06);
        assert_eq!(parsed.length, 20);
    }

    #[test]
    fn test_data_chunk_serialization() {
        let chunk = DataChunk {
            flags: DataFlags(DataFlags::BEGIN | DataFlags::END),
            stream_id: 1,
            stream_seq_num: 0,
            payload_protocol_id: 0,
            transmission_seq_num: 42,
            user_data: b"hello".to_vec(),
        };
        let bytes = chunk.to_bytes();
        let parsed = DataChunk::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.stream_id, 1);
        assert_eq!(parsed.transmission_seq_num, 42);
        assert_eq!(parsed.user_data, b"hello");
    }
}
