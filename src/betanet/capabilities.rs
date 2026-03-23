// densifier insight: runtime capability probe for SIMD/MLIR/eBPF gating
// - prefers native detection on supported arches
// - allows overrides via environment variables for CI and testing

/// Check runtime capability overrides via env vars first.
fn env_override(key: &str) -> Option<bool> {
    match std::env::var(key) {
        Ok(v) => match v.as_str() {
            "1" | "true" | "yes" => Some(true),
            "0" | "false" | "no" => Some(false),
            _ => None,
        },
        Err(_) => None,
    }
}

/// Returns true if AVX2 is available (or forced by env var `BETANET_FORCE_AVX2`).
pub fn has_avx2() -> bool {
    if let Some(o) = env_override("BETANET_FORCE_AVX2") {
        return o;
    }
    // On x86/x86_64 use std macro; otherwise assume false.
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        std::is_x86_feature_detected!("avx2")
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        false
    }
}

/// Returns true if MLIR JIT is available (placeholder; env override `BETANET_FORCE_MLIR`).
pub fn has_mlir() -> bool {
    if let Some(o) = env_override("BETANET_FORCE_MLIR") {
        return o;
    }
    // MLIR runtime detection is environment-specific; default to false here.
    false
}

/// Returns true if eBPF offload is available (placeholder; env override `BETANET_FORCE_EBPF`).
pub fn has_ebpf() -> bool {
    if let Some(o) = env_override("BETANET_FORCE_EBPF") {
        return o;
    }
    // Kernel eBPF is Linux-only; on non-Linux default false.
    #[cfg(target_os = "linux")]
    {
        // In a real implementation we'd probe /sys or attempt a verifier query.
        // For TDD, default to false.
        false
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_env_overrides() {
        env::set_var("BETANET_FORCE_AVX2", "1");
        assert!(has_avx2());
        env::set_var("BETANET_FORCE_AVX2", "0");
        assert!(!has_avx2());

        env::set_var("BETANET_FORCE_MLIR", "true");
        assert!(has_mlir());
        env::set_var("BETANET_FORCE_MLIR", "false");
        assert!(!has_mlir());

        env::set_var("BETANET_FORCE_EBPF", "yes");
        assert!(has_ebpf());
        env::set_var("BETANET_FORCE_EBPF", "no");
        assert!(!has_ebpf());
    }
}
