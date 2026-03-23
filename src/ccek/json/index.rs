//! JsonIndex - Offset encoding from TrikeShed JsonIndex.kt
//!
//! 2-bit encoding scheme for offsets:
//! - 00: Short offset (6 bits, 0-63)
//! - 01: UShort extension (6 bits + 8 bits, 0-16383)
//! - 10: ULong64 extension (6 bits + 16 bits, 0-4194303)
//! - 11: Reserved

/// Offset encoding types (2 bits in high nibble)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OffsetEncoding {
    Short = 0,      // 6 bits: 0-63
    UShort = 1,     // 6+8 bits: 0-16383
    ULong = 2,      // 6+16 bits: 0-4194303
    Reserved = 3,
}

impl OffsetEncoding {
    /// Encode offset with encoding type
    pub fn encode(offset: u64) -> Vec<u8> {
        match offset {
            0..=63 => vec![offset as u8],
            64..=16383 => {
                let short_part = ((offset >> 8) as u8) | 0x40; // UShort marker
                let long_part = (offset & 0xFF) as u8;
                vec![short_part, long_part]
            }
            16384..=4194303 => {
                let marker = ((offset >> 16) as u8) | 0x80; // ULong marker
                let mid = ((offset >> 8) & 0xFF) as u8;
                let low = (offset & 0xFF) as u8;
                vec![marker, mid, low]
            }
            _ => panic!("Offset too large for encoding"),
        }
    }

    /// Decode bytes to (offset, encoding_type)
    pub fn decode(bytes: &[u8]) -> Option<(u64, OffsetEncoding)> {
        if bytes.is_empty() {
            return None;
        }

        let marker = bytes[0];
        let encoding = match (marker >> 6) & 0x03 {
            0 => OffsetEncoding::Short,
            1 => OffsetEncoding::UShort,
            2 => OffsetEncoding::ULong,
            _ => OffsetEncoding::Reserved,
        };

        let offset = match encoding {
            OffsetEncoding::Short => (marker & 0x3F) as u64,
            OffsetEncoding::UShort if bytes.len() >= 2 => {
                let high = (marker & 0x3F) as u64;
                let low = bytes[1] as u64;
                (high << 8) | low
            }
            OffsetEncoding::ULong if bytes.len() >= 3 => {
                let high = (marker & 0x3F) as u64;
                let mid = bytes[1] as u64;
                let low = bytes[2] as u64;
                (high << 16) | (mid << 8) | low
            }
            _ => return None,
        };

        Some((offset, encoding))
    }
}

/// JsonIndex - Index structure for JSON
pub struct JsonIndex;

impl JsonIndex {
    /// Create new empty index
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsonIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_short() {
        let encoded = OffsetEncoding::encode(42);
        assert_eq!(encoded.len(), 1);
        assert_eq!(encoded[0], 42);

        let (offset, enc) = OffsetEncoding::decode(&encoded).unwrap();
        assert_eq!(offset, 42);
        assert!(matches!(enc, OffsetEncoding::Short));
    }

    #[test]
    fn test_encode_ushort() {
        let encoded = OffsetEncoding::encode(500);
        assert_eq!(encoded.len(), 2);

        let (offset, enc) = OffsetEncoding::decode(&encoded).unwrap();
        assert_eq!(offset, 500);
        assert!(matches!(enc, OffsetEncoding::UShort));
    }

    #[test]
    fn test_encode_ulong() {
        let encoded = OffsetEncoding::encode(100000);
        assert_eq!(encoded.len(), 3);

        let (offset, enc) = OffsetEncoding::decode(&encoded).unwrap();
        assert_eq!(offset, 100000);
        assert!(matches!(enc, OffsetEncoding::ULong));
    }
}
