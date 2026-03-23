// densifier insight: SIMD-first anchor matching with deterministic priority; MLIR fallback placeholder

/// A small, testable Anchor type and a naive detector for TDD.
/// This is intentionally narrowly scoped: it models Anchor matching and priority resolution.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    pub pattern: u64,
    pub priority: u8,
    pub mask: u128, // reserved for wider/masked matches in future
}

impl Anchor {
    /// Returns true if the anchor matches anywhere in `data`.
    pub fn matches(&self, data: &[u8]) -> bool {
        self.first_match_offset(data).is_some()
    }

    /// Returns the offset (byte index) of the first 8-byte window that matches,
    /// or None if no match. This is useful for tie-breaking on earliest match.
    pub fn first_match_offset(&self, data: &[u8]) -> Option<usize> {
        if data.len() < 8 {
            return None;
        }

        // Interpret mask: use lower 64 bits. A mask of 0 means "full mask" (match all bits).
        let mask_u64: u64 = if self.mask == 0 { !0u64 } else { (self.mask & 0xFFFF_FFFF_FFFF_FFFF) as u64 };
        let pat = self.pattern & mask_u64;

        for (i, w) in data.windows(8).enumerate() {
            let word = u64::from_be_bytes([w[0], w[1], w[2], w[3], w[4], w[5], w[6], w[7]]);
            if (word & mask_u64) == pat {
                return Some(i);
            }
        }
        None
    }
}

/// Protocol detector composed from an Anchor set and a tiny recognition stub.
pub struct ProtocolDetector {
    anchors: Vec<Anchor>,
}

impl ProtocolDetector {
    pub fn new(anchors: Vec<Anchor>) -> Self {
        Self { anchors }
    }

    /// Returns the highest-priority matching anchor, if any.
    pub fn detect(&self, data: &[u8]) -> Option<Anchor> {
        // Track best by (priority, -offset) so we prefer higher priority, and for equal
        // priority prefer smaller offset (earlier match).
        let mut best: Option<(Anchor, usize)> = None; // (anchor, offset)

        for a in &self.anchors {
            if let Some(off) = a.first_match_offset(data) {
                match &best {
                    None => best = Some((a.clone(), off)),
                    Some((existing, ex_off)) => {
                        if a.priority > existing.priority {
                            best = Some((a.clone(), off));
                        } else if a.priority == existing.priority && off < *ex_off {
                            // tie on priority, pick earliest offset
                            best = Some((a.clone(), off));
                        }
                    }
                }
            }
        }

        best.map(|(a, _)| a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_match_happy_path() {
        let anchor = Anchor { pattern: 0x1122_3344_5566_7788, priority: 5, mask: 0 };
        let bytes = vec![0, 1, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0];
        assert!(anchor.matches(&bytes));
    }

    #[test]
    fn detector_priority_resolution() {
        let a1 = Anchor { pattern: 0xAA00_0000_0000_0000, priority: 1, mask: 0 };
        let a2 = Anchor { pattern: 0x1122_3344_5566_7788, priority: 10, mask: 0 };
        let data = vec![0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0];
        let det = ProtocolDetector::new(vec![a1.clone(), a2.clone()]);
        let matched = det.detect(&data).expect("should match");
        assert_eq!(matched, a2);
    }

    #[test]
    fn detector_no_match() {
        let det = ProtocolDetector::new(vec![Anchor { pattern: 1, priority: 1, mask: 0 }]);
        let data = vec![0u8; 4];
        assert!(det.detect(&data).is_none());
    }

    #[test]
    fn detector_tie_break_earliest_offset() {
        // Two anchors with same priority; one appears earlier in the data and should win
        let a1 = Anchor { pattern: 0x0000_0000_0000_1111, priority: 5, mask: 0 };
        let a2 = Anchor { pattern: 0x0000_0000_0000_2222, priority: 5, mask: 0 };

        // data: a2 appears later than a1
        let mut data = vec![0u8; 0];
        data.extend_from_slice(&a1.pattern.to_be_bytes()); // offset 0
        data.extend_from_slice(&[0u8; 4]);
        data.extend_from_slice(&a2.pattern.to_be_bytes()); // offset > 0

        let det = ProtocolDetector::new(vec![a1.clone(), a2.clone()]);
        let matched = det.detect(&data).expect("should match one");
        assert_eq!(matched, a1);
    }

    // -- property-style deterministic tests (no external deps)
    fn xorshift64(seed: &mut u64) -> u64 {
        // simple xorshift64* variant
        let mut x = *seed;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *seed = x;
        x
    }

    #[test]
    fn prop_priority_preference_randomized() {
        let mut seed = 0xDEADBEEF_FEEDFACEu64;
        for _ in 0..200 {
            let low_pat = xorshift64(&mut seed);
            let high_pat = xorshift64(&mut seed);

            let p_low = (xorshift64(&mut seed) % 10) as u8;
            let p_high = p_low + 1; // ensure higher priority

            let a_low = Anchor { pattern: low_pat, priority: p_low, mask: 0 };
            let a_high = Anchor { pattern: high_pat, priority: p_high, mask: 0 };

            // build data: low appears earlier, high appears later
            let mut data = Vec::new();
            data.extend_from_slice(&a_low.pattern.to_be_bytes());
            data.extend_from_slice(&[0u8; 3]);
            data.extend_from_slice(&a_high.pattern.to_be_bytes());

            let det = ProtocolDetector::new(vec![a_low.clone(), a_high.clone()]);
            let matched = det.detect(&data).expect("should match one");
            assert_eq!(matched, a_high, "expected higher priority to win regardless of offset");
        }
    }

    #[test]
    fn prop_tie_break_earliest_offset_randomized() {
        let mut seed = 0xC0FFEE_BABE_CAFEu64;
        for _ in 0..200 {
            let pat1 = xorshift64(&mut seed);
            let pat2 = xorshift64(&mut seed);
            let p = (xorshift64(&mut seed) % 10) as u8;

            let a1 = Anchor { pattern: pat1, priority: p, mask: 0 };
            let a2 = Anchor { pattern: pat2, priority: p, mask: 0 };

            // place a2 earlier than a1
            let mut data = Vec::new();
            data.extend_from_slice(&a2.pattern.to_be_bytes()); // offset 0
            data.extend_from_slice(&[0u8; 4]);
            data.extend_from_slice(&a1.pattern.to_be_bytes()); // later

            let det = ProtocolDetector::new(vec![a1.clone(), a2.clone()]);
            let matched = det.detect(&data).expect("should match one");
            // same priority → earliest offset (a2) should win
            assert_eq!(matched, a2, "expected earliest offset to win when priorities equal");
        }
    }

    #[test]
    fn prop_mask_respected_randomized() {
        let mut seed = 0xFEED_FACE_DEAD_BEEFu64;
        for _ in 0..200 {
            let base = xorshift64(&mut seed);
            // choose a mask that keeps lower 16 bits only
            let mask: u128 = 0x0000_0000_0000_FFFFu128;
            let pattern = (base & 0xFFFF) as u64;
            let anchor = Anchor { pattern, priority: 8, mask };

            // craft a word that differs in upper bits but matches low 16 bits
            let word = ((xorshift64(&mut seed) & 0xFFFF_FFFF_FFFF_0000) | (pattern as u64)) as u64;
            let data = word.to_be_bytes().to_vec();

            assert!(anchor.matches(&data), "mask should allow matching low bits only");
        }
    }
}
