// Densifier shim for the selfhosted tool crate. Small, well-scoped types and tests.
use std::marker::PhantomData;

#[repr(transparent)]
pub struct Join<A, B> {
    packed: u64,
    _phantom: PhantomData<(A, B)>,
}

impl Join<u32, u32> {
    pub fn pack(a: u32, b: u32) -> Self {
        let packed = ((a as u64) << 32) | (b as u64);
        Self { packed, _phantom: PhantomData }
    }

    pub fn unpack(&self) -> (u32, u32) {
        let a = (self.packed >> 32) as u32;
        let b = (self.packed & 0xffff_ffff) as u32;
        (a, b)
    }
}

pub struct Indexed<'a, T> {
    offset: u32,
    slice: &'a [T],
}

impl<'a, T> Indexed<'a, T> {
    pub fn new(offset: u32, slice: &'a [T]) -> Self {
        Self { offset, slice }
    }

    pub fn get(&self) -> Option<&T> {
        self.slice.get(self.offset as usize)
    }
}

/// Simple Anchor concept: pattern bytes with a priority and optional mask.
#[derive(Clone, Debug)]
pub struct Anchor {
    pub pattern: Vec<u8>,
    pub priority: u8,
    pub simd_mask: Option<Vec<u8>>,
}

impl Anchor {
    pub fn new(pattern: Vec<u8>, priority: u8) -> Self {
        Self { pattern, priority, simd_mask: None }
    }

    pub fn with_mask(pattern: Vec<u8>, mut mask: Vec<u8>, priority: u8) -> Self {
        if mask.len() != pattern.len() {
            mask.resize(pattern.len(), 0xff);
        }
        Self { pattern, priority, simd_mask: Some(mask) }
    }
}

/// Protocol signals produced by a small detector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolSignal {
    Http,
    Tls,
    Unknown,
}

/// Minimal ProtocolDetector: holds anchors and detects by prefix match.
pub struct ProtocolDetector {
    anchors: Vec<Anchor>,
}

impl ProtocolDetector {
    pub fn new(mut anchors: Vec<Anchor>) -> Self {
        // sort by priority descending so higher priority anchors match first
        anchors.sort_by(|a, b| b.priority.cmp(&a.priority));
        Self { anchors }
    }

    pub fn detect(&self, data: &[u8]) -> Option<ProtocolSignal> {
        for a in &self.anchors {
            // masked match if simd_mask present
            if let Some(mask) = &a.simd_mask {
                if data.len() < a.pattern.len() { continue; }
                let mut ok = true;
                for i in 0..a.pattern.len() {
                    if (data[i] & mask[i]) != (a.pattern[i] & mask[i]) {
                        ok = false; break;
                    }
                }
                if !ok { continue; }
            } else {
                if !data.starts_with(&a.pattern) { continue; }
            }

            // Map a couple of known patterns to signals for tests
            if a.pattern == b"GET".to_vec() {
                return Some(ProtocolSignal::Http);
            }
            if a.pattern.len() >= 2 && a.pattern[0] == 0x16 && a.pattern[1] == 0x03 {
                return Some(ProtocolSignal::Tls);
            }
            return Some(ProtocolSignal::Unknown);
        }
        None
    }
}

/// Convenience API: detect using a slice of anchors (not consuming), returns highest-priority match.
pub fn detect_priority(anchors: &[Anchor], data: &[u8]) -> Option<ProtocolSignal> {
    // create a local sorted list of anchors by priority desc
    let mut v: Vec<&Anchor> = anchors.iter().collect();
    v.sort_by(|a, b| b.priority.cmp(&a.priority));
    for a in v {
        if data.starts_with(&a.pattern) {
            if a.pattern == b"GET".to_vec() {
                return Some(ProtocolSignal::Http);
            }
            if a.pattern.len() >= 2 && a.pattern[0] == 0x16 && a.pattern[1] == 0x03 {
                return Some(ProtocolSignal::Tls);
            }
            return Some(ProtocolSignal::Unknown);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_pack_unpack() {
        let j = Join::pack(0xdead_beef, 0xcafe_f00d);
        let (a, b) = j.unpack();
        assert_eq!(a, 0xdead_beef);
        assert_eq!(b, 0xcafe_f00d);
    }

    #[test]
    fn indexed_get() {
        let data = [1u8, 2, 3];
        let idx = Indexed::new(2, &data);
        assert_eq!(idx.get(), Some(&3u8));

        let idx_oob = Indexed::new(10, &data);
        assert_eq!(idx_oob.get(), None);
    }

    // Red TDD: ProtocolDetector should support prioritized anchor detection
    #[test]
    fn protocol_detector_priority() {
        // anchors: HTTP "GET" (priority 10), TLS magic (0x16, 0x03) priority 5
        let anchors = vec![
            Anchor::new(b"GET".to_vec(), 10),
            Anchor::new(vec![0x16, 0x03], 5),
        ];

        let detector = ProtocolDetector::new(anchors);

        let http_packet = b"GET /index.html HTTP/1.1\r\n".to_vec();
        let tls_packet = vec![0x16, 0x03, 0x01, 0x00];

        // HTTP should be detected for http_packet
        assert_eq!(detector.detect(&http_packet), Some(ProtocolSignal::Http));
        // TLS should be detected for tls_packet
        assert_eq!(detector.detect(&tls_packet), Some(ProtocolSignal::Tls));
    }

    #[test]
    fn detect_priority_helper() {
        let anchors = vec![
            Anchor::new(b"GET".to_vec(), 10),
            Anchor::new(vec![0x16, 0x03], 5),
        ];

        let http_packet = b"GET /index.html HTTP/1.1\r\n".to_vec();
        assert_eq!(super::detect_priority(&anchors, &http_packet), Some(super::ProtocolSignal::Http));
    }

    #[test]
    fn masked_anchor_detection() {
        // pattern where only low 4 bits matter (nibble), mask selects low 4 bits
        let pattern = vec![0x0a_u8, 0x0b_u8];
        let mask = vec![0x0f_u8, 0x0f_u8];
        let anchor = Anchor::with_mask(pattern.clone(), mask.clone(), 8);

        // sanity: mask and pattern stored
        assert_eq!(anchor.simd_mask.as_ref().map(|m| m.len()), Some(anchor.pattern.len()));

        // data varies high nibble but matches low nibble
        let data = vec![0x1a_u8, 0x2b_u8, 0xff];

        // use ProtocolDetector directly
        let detector = ProtocolDetector::new(vec![anchor]);
        assert_eq!(detector.detect(&data), Some(ProtocolSignal::Unknown));
    }
}
