// densifier insight: detection pipeline that prefers SIMD anchors then MLIR compiled matcher then interpreter fallback

use crate::anchor::Anchor;
use crate::simd_match;
use crate::mlir_mock;

#[derive(Debug, PartialEq, Eq)]
pub enum Detection {
    Anchor(Anchor),
    MlirCompiled,
    MlirInterpreted,
}

/// Pipeline: 1) SIMD-first anchor detection, 2) MLIR compiled matcher (if source provided), 3) MLIR interpreter fallback.
pub fn detect_pipeline(anchors: &[Anchor], data: &[u8], mlir_src: Option<&str>) -> Option<Detection> {
    if let Some(a) = simd_match::detect_with_policy(anchors, data) {
        return Some(Detection::Anchor(a));
    }

    if let Some(src) = mlir_src {
        match mlir_mock::compile_mlir(src) {
            Ok(compiled) => {
                if compiled.run(data) {
                    return Some(Detection::MlirCompiled);
                }
            }
            Err(_) => {
                // compile failed; fall back to interpreter
                if mlir_mock::interpret_mlir(src, data) {
                    return Some(Detection::MlirInterpreted);
                }
            }
        }
    }

    None
}

// densifier insight: detector pipeline — MLIR-compile-first, then SIMD/scalar fallback

use crate::anchor::Anchor;
use crate::mlir_mock;
use crate::simd_match;
use crate::capabilities;

/// Detect using the pipeline: if MLIR is available and `mlir_src` is provided,
/// try to compile and run the compiled matcher; on success, return the detected
/// Anchor via the simd detector (keeps Anchor identity). Otherwise fall back to
/// simd/scalar detection.
pub fn detect_pipeline(anchors: &[Anchor], data: &[u8], mlir_src: Option<&str>) -> Option<Anchor> {
    if capabilities::has_mlir() {
        if let Some(src) = mlir_src {
            if let Ok(compiled) = mlir_mock::compile_mlir(src) {
                if compiled.run(data) {
                    // Find anchor using simd detector to return a concrete Anchor
                    return simd_match::detect_with_policy(anchors, data);
                }
            }
            // If compilation failed or didn't run, fall through to simd
        }
    }
    simd_match::detect_with_policy(anchors, data)
}

// densifier insight: pipeline that prefers MLIR-compiled matcher, falls back to interpreter,
// then falls back to SIMD/scalar detection policy. This keeps the TDD chain explicit and
// testable: MLIR acts as an optimization hint but does not change priority resolution.

use crate::anchor::{Anchor, ProtocolDetector};
use crate::mlir_mock::{compile_mlir, interpret_mlir};
use crate::simd_match;
use crate::capabilities;

/// Detects a matching anchor using the pipeline:
/// 1. If `mlir_src` is Some and MLIR is enabled: try to compile; if compiled and matches data,
///    return the priority-resolved anchor from the anchors list. If compile fails, interpret and
///    use that result similarly.
/// 2. Otherwise, use the SIMD/scalar policy detector.
pub fn detect_pipeline(anchors: &[Anchor], data: &[u8], mlir_src: Option<&str>) -> Option<Anchor> {
    if let Some(src) = mlir_src {
        if capabilities::has_mlir() {
            match compile_mlir(src) {
                Ok(compiled) => {
                    if compiled.run(data) {
                        return ProtocolDetector::new(anchors.to_vec()).detect(data);
                    } else {
                        return None;
                    }
                }
                Err(_) => {
                    // Interpreter fallback
                    if interpret_mlir(src, data) {
                        return ProtocolDetector::new(anchors.to_vec()).detect(data);
                    } else {
                        return None;
                    }
                }
            }
        }
    }

    // No MLIR path; use SIMD/scalar policy
    simd_match::detect_with_policy(anchors, data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::Anchor;
    use std::env;

    #[test]
    fn pipeline_anchor_path() {
        env::set_var("BETANET_FORCE_AVX2", "1");
        let anchor = Anchor { pattern: 0x0102_0304_0506_0708, priority: 5, mask: 0 };
        let anchors = vec![anchor.clone()];
        let data = vec![0, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0];
        let det = detect_pipeline(&anchors, &data, None).expect("should detect anchor");
        assert_eq!(det, Detection::Anchor(anchor));
    }

    #[test]
    fn pipeline_mlir_compiled() {
        env::set_var("BETANET_FORCE_AVX2", "0");
        let anchors: Vec<Anchor> = vec![];
        let data = vec![0x11,0x22,0x33,0x44,0x55,0x66,0x77,0x88];
        let src = "// mlir: compile_ok pattern 0x1122334455667788";
        let det = detect_pipeline(&anchors, &data, Some(src)).expect("should detect mlir compiled");
        assert_eq!(det, Detection::MlirCompiled);
    }

    #[test]
    fn pipeline_mlir_interpret() {
        env::set_var("BETANET_FORCE_AVX2", "0");
        let anchors: Vec<Anchor> = vec![];
        let data = vec![0x11,0x22,0x33,0x44,0x55,0x66,0x77,0x88];
        let src = "// mlir: some_uncompilable pattern 0x1122334455667788";
        let det = detect_pipeline(&anchors, &data, Some(src)).expect("should detect mlir interpreted");
        assert_eq!(det, Detection::MlirInterpreted);
    }

    #[test]
    fn pipeline_no_match() {
        env::set_var("BETANET_FORCE_AVX2", "0");
        let anchors: Vec<Anchor> = vec![];
        let data = vec![0u8; 4];
        let det = detect_pipeline(&anchors, &data, Some("// mlir: nopattern"));
        assert!(det.is_none());
    }

    #[test]
    fn mlir_compile_then_detect() {
        let a1 = Anchor { pattern: 0x1122_3344_5566_7788, priority: 5, mask: 0 };
        let a2 = Anchor { pattern: 0x0102_0304_0506_0708, priority: 1, mask: 0 };
        let anchors = vec![a2.clone(), a1.clone()];
        let data = vec![0x11,0x22,0x33,0x44,0x55,0x66,0x77,0x88];

        env::set_var("BETANET_FORCE_MLIR", "1");
        let mlir_src = "// mlir: compile_ok pattern 0x1122334455667788";
        let detected = detect_pipeline(&anchors, &data, Some(mlir_src)).expect("should detect");
        assert_eq!(detected, a1);
    }

    #[test]
    fn mlir_unavailable_uses_simd() {
        let a1 = Anchor { pattern: 0x1122_3344_5566_7788, priority: 5, mask: 0 };
        let anchors = vec![a1.clone()];
        let data = vec![0x11,0x22,0x33,0x44,0x55,0x66,0x77,0x88];

        env::set_var("BETANET_FORCE_MLIR", "0");
        env::set_var("BETANET_FORCE_AVX2", "1");
        let detected = detect_pipeline(&anchors, &data, Some("// mlir: compile_ok"));
        assert_eq!(detected.unwrap(), a1);
    }

    #[test]
    fn mlir_compile_failure_falls_back() {
        let a1 = Anchor { pattern: 0x1122_3344_5566_7788, priority: 5, mask: 0 };
        let anchors = vec![a1.clone()];
        let data = vec![0x11,0x22,0x33,0x44,0x55,0x66,0x77,0x88];

        env::set_var("BETANET_FORCE_MLIR", "1");
        // source without compile_ok will fail to compile in mock
        let detected = detect_pipeline(&anchors, &data, Some("// mlir: badcode 0x1122334455667788"));
        // fallback simd detection should pick it up
        assert_eq!(detected.unwrap(), a1);
    }

    #[test]
    fn mlir_compiled_path() {
        env::set_var("BETANET_FORCE_MLIR", "1");
        // anchors
        let a_low = Anchor { pattern: 0x0102_0304_0506_0708, priority: 1, mask: 0 };
        let a_high = Anchor { pattern: 0x1122_3344_5566_7788, priority: 10, mask: 0 };
        let anchors = vec![a_low.clone(), a_high.clone()];

        let data = vec![0x11,0x22,0x33,0x44,0x55,0x66,0x77,0x88];
        let mlir_src = "// mlir: compile_ok pattern 0x1122334455667788";

        let matched = detect_pipeline(&anchors, &data, Some(mlir_src)).expect("should match");
        assert_eq!(matched, a_high);
    }

    #[test]
    fn mlir_compile_failure_interpreter_fallback() {
        env::set_var("BETANET_FORCE_MLIR", "1");
        let a_high = Anchor { pattern: 0x1122_3344_5566_7788, priority: 10, mask: 0 };
        let anchors = vec![a_high.clone()];
        let data = vec![0x11,0x22,0x33,0x44,0x55,0x66,0x77,0x88];
        // No `compile_ok` token -> compile_mlir returns Err, interpreter should find pattern
        let mlir_src = "// mlir: some_uncompilable_code pattern 0x1122334455667788";
        let matched = detect_pipeline(&anchors, &data, Some(mlir_src)).expect("interpreter should match");
        assert_eq!(matched, a_high);
    }

    #[test]
    fn no_mlir_uses_simd_policy() {
        env::set_var("BETANET_FORCE_MLIR", "0");
        // Force SIMD path
        env::set_var("BETANET_FORCE_AVX2", "1");
        let a_low = Anchor { pattern: 0x0102_0304_0506_0708, priority: 1, mask: 0 };
        let a_high = Anchor { pattern: 0x1122_3344_5566_7788, priority: 10, mask: 0 };
        let anchors = vec![a_low.clone(), a_high.clone()];
        let data = vec![0,0x11,0x22,0x33,0x44,0x55,0x66,0x77,0x88,0];
        let matched = detect_pipeline(&anchors, &data, None).expect("simd path should match");
        assert_eq!(matched, a_high);
    }
}
