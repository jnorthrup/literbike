// Minimal QUIC protocol parsing helpers (varint example)

/// Encode a QUIC-style varint (RFC vlong) - naive scalar implementation
pub fn encode_varint(mut v: u64) -> Vec<u8> {
    if v < 0x40 {
        return vec![v as u8];
    }
    // very small placeholder implementation for scaffold
    let mut out = Vec::new();
    while v > 0 {
        out.push((v & 0xff) as u8);
        v >>= 8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn varint_small() {
        assert_eq!(encode_varint(10), vec![10u8]);
    }
}
