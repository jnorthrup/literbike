// densifier insight: SIMD-first detection scaffold (naive u64 scan as a fast path)
// This module provides a small policy wrapper that prefers a simulated SIMD path
// when AVX2 is reported available by `capabilities::has_avx2()`; falls back to
// the scalar `ProtocolDetector` otherwise. This is intentionally small and
// testable — a next TDD step is to replace the naive loop with actual SIMD ops.

use crate::anchor::{Anchor, ProtocolDetector};
use crate::capabilities;

pub fn detect_with_policy(anchors: &[Anchor], data: &[u8]) -> Option<Anchor> {
    // SIMD path is opt-in via the `simd` cargo feature. Also require runtime AVX2
    // detection (or env override) to enable the fast path. This keeps tests
    // deterministic while making SIMD explicit for CI/bench runs.
    if cfg!(feature = "simd") && capabilities::has_avx2() {
        simd_detect(anchors, data)
    } else {
        ProtocolDetector::new(anchors.to_vec()).detect(data)
    }
}

fn simd_detect(anchors: &[Anchor], data: &[u8]) -> Option<Anchor> {
    // If compiled for x86/x86_64 and the CPU reports AVX2, prefer the AVX2 scaffold.
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("avx2") {
            // Safe wrapper around the unsafe AVX2 scaffold; the scaffold currently
            // mirrors the scalar semantics but is marked as a target feature so
            // future iterations can swap in true SIMD operations.
            unsafe {
                if let Some(found) = avx2_scaffold_detect(anchors, data) {
                    return Some(found);
                }
            }
        }
    }
    // Naive fast-path: compare 8-byte words (big-endian) against anchor patterns.
    // Keeps the semantics of priority resolution.
    let mut best: Option<Anchor> = None;
    if data.len() < 8 {
        return None;
    }

    for window in data.windows(8) {
        // Use Anchor::matches so masks are respected by the fast path.
        for a in anchors {
            if a.matches(window) {
                match &best {
                    None => best = Some(a.clone()),
                    Some(existing) => {
                        if a.priority > existing.priority {
                            best = Some(a.clone())
                        }
                    }
                }
            }
        }
    }

    best
}

// AVX2 scaffold: currently a semantic-equivalent to the naive loop but
// annotated for `avx2`. This is intentionally simple for TDD; future steps
// will replace the inner loop with actual vectorized compares.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn avx2_scaffold_detect(anchors: &[Anchor], data: &[u8]) -> Option<Anchor> {
    let mut best: Option<Anchor> = None;
    if data.len() < 8 {
        return None;
    }

    for window in data.windows(8) {
        for a in anchors {
            if a.matches(window) {
                match &best {
                    None => best = Some(a.clone()),
                    Some(existing) => {
                        if a.priority > existing.priority {
                            best = Some(a.clone())
                        }
                    }
                }
            }
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn simd_and_scalar_paths_agree() {
        let a_low = Anchor { pattern: 0x0102_0304_0506_0708, priority: 1, mask: 0 };
        let a_high = Anchor { pattern: 0x1122_3344_5566_7788, priority: 10, mask: 0 };
        let anchors = vec![a_low.clone(), a_high.clone()];

        // data containing the high-priority pattern
        let data = vec![0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0];

        // Force scalar path
        env::set_var("BETANET_FORCE_AVX2", "0");
        let s = detect_with_policy(&anchors, &data).expect("scalar should match");
        assert_eq!(s, a_high);

        // Force simd path
        env::set_var("BETANET_FORCE_AVX2", "1");
        let v = detect_with_policy(&anchors, &data).expect("simd should match");
        assert_eq!(v, a_high);
    }

    #[test]
    fn masked_anchor_match() {
        // anchor looks for pattern where only lower 32 bits must match
        let pattern: u64 = 0xDEADBEEF_0000_1234;
        let mask: u128 = 0x0000_0000_FFFF_FFFF; // only lower 32 bits
        let anchor = Anchor { pattern, priority: 1, mask };

        // data where only lower 32 bits match
        let data = vec![
            0x00, 0x00, 0x00, 0x00, // high bytes
            0x00, 0x00, 0x12, 0x34, // low 32 bits = 0x00001234
        ];

        assert!(anchor.matches(&data));
    }

    #[test]
    fn overlapping_anchor_priority() {
        // Two anchors with overlapping patterns; higher priority should win
        let a1 = Anchor { pattern: 0xAABBCCDD11223344, priority: 5, mask: 0 };
        let a2 = Anchor { pattern: 0xAABBCCDD11223344, priority: 10, mask: 0 };
        let detector = ProtocolDetector { anchors: vec![a1.clone(), a2.clone()] };

        let data = a1.pattern.to_be_bytes().to_vec();

        let simd_choice = detect_with_policy(&detector.anchors, &data);
        assert!(simd_choice.is_some());
        // expect the anchor with higher priority (a2)
        assert_eq!(simd_choice.unwrap().priority, a2.priority);
    }

    // Deterministic parity test: ensure simd path (forced via env var) matches scalar path
    #[test]
    fn simd_scalar_parity_randomized() {
        let mut seed: u64 = 0x1234_5678_9ABC_DEF0;

        fn xorshift64(seed: &mut u64) -> u64 {
            let mut x = *seed;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            *seed = x;
            x
        }

        for _ in 0..200 {
            // create 3 anchors with random patterns/priorities/masks
            let mut anchors = Vec::new();
            for i in 0..3u8 {
                let pat = xorshift64(&mut seed);
                let pr = (xorshift64(&mut seed) % 16) as u8;
                let mask = if (xorshift64(&mut seed) & 1) == 0 { 0 } else { 0x0000_0000_FFFF_FFFFu128 };
                anchors.push(Anchor { pattern: pat, priority: pr.wrapping_add(i), mask });
            }

            // craft data: insert anchor windows at random offsets
            let mut data = vec![0u8; 32];
            let pos1 = (xorshift64(&mut seed) as usize) % 8;
            let pos2 = ((xorshift64(&mut seed) as usize) % 8) + 8;
            let pos3 = ((xorshift64(&mut seed) as usize) % 8) + 16;
            data.splice(pos1..pos1+8, anchors[0].pattern.to_be_bytes().iter().cloned());
            data.splice(pos2..pos2+8, anchors[1].pattern.to_be_bytes().iter().cloned());
            data.splice(pos3..pos3+8, anchors[2].pattern.to_be_bytes().iter().cloned());

            // Force scalar path
            std::env::set_var("BETANET_FORCE_AVX2", "0");
            let scalar = ProtocolDetector::new(anchors.clone()).detect(&data);

            // Force simd path
            std::env::set_var("BETANET_FORCE_AVX2", "1");
            let simd = detect_with_policy(&anchors, &data);

            assert_eq!(scalar.map(|a| a.pattern), simd.map(|a| a.pattern));
        }
    }
}
