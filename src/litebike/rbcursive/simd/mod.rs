// SIMD implementations for different architectures
// Port of BBCursive SIMD capabilities to Rust

#[cfg(target_arch = "aarch64")]
pub mod neon;

#[cfg(target_arch = "x86_64")]
pub mod avx2;

#[cfg(target_arch = "x86_64")]
pub mod sse2;

pub mod generic;

use crate::rbcursive::scanner::SimdScanner;

/// Create the best SIMD scanner for the current platform
pub fn create_optimal_scanner() -> Box<dyn SimdScanner> {
    #[cfg(target_arch = "aarch64")]
    {
        // ARM64/Apple Silicon - use NEON
        Box::new(neon::NeonScanner::new())
    }
    
    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    {
        // x86-64 - check for AVX2, fallback to SSE2
        if std::arch::is_x86_feature_detected!("avx2") {
            Box::new(avx2::Avx2Scanner::new())
        } else if std::arch::is_x86_feature_detected!("sse2") {
            Box::new(sse2::Sse2Scanner::new())
        } else {
            Box::new(generic::GenericScanner::new())
        }
    }
    
    #[cfg(not(any(target_arch = "aarch64", all(target_arch = "x86_64", target_os = "linux"))))]
    {
        // Generic fallback for other platforms
        Box::new(generic::GenericScanner::new())
    }
}

/// SIMD feature detection for runtime capability discovery
pub struct SimdCapabilities {
    pub has_neon: bool,
    pub has_avx2: bool,
    pub has_sse2: bool,
    pub max_vector_bits: u32,
}

impl SimdCapabilities {
    pub fn detect() -> Self {
        let mut caps = Self {
            has_neon: false,
            has_avx2: false,
            has_sse2: false,
            max_vector_bits: 0,
        };
        
        #[cfg(target_arch = "aarch64")]
        {
            // ARM64 always has NEON
            caps.has_neon = true;
            caps.max_vector_bits = 128;
        }
        
        #[cfg(target_arch = "x86_64")]
        {
            if std::arch::is_x86_feature_detected!("avx2") {
                caps.has_avx2 = true;
                caps.max_vector_bits = 256;
            } else if std::arch::is_x86_feature_detected!("sse2") {
                caps.has_sse2 = true;
                caps.max_vector_bits = 128;
            }
        }
        
        caps
    }
    
    pub fn best_scanner_name(&self) -> &'static str {
        if self.has_avx2 {
            "AVX2"
        } else if self.has_neon {
            "NEON"
        } else if self.has_sse2 {
            "SSE2"
        } else {
            "Generic"
        }
    }
    
    pub fn estimated_throughput_gbps(&self) -> f64 {
        match self.max_vector_bits {
            256 => 3.0,  // AVX2 - ~3 GB/s
            128 if self.has_neon => 4.0,  // NEON on Apple Silicon - ~4 GB/s
            128 => 1.5,  // SSE2 - ~1.5 GB/s
            _ => 0.1,    // Generic - ~100 MB/s
        }
    }
}

/// Benchmark all available SIMD scanners
pub fn benchmark_all_scanners(data: &[u8]) -> Vec<(String, f64)> {
    let mut results = Vec::new();
    
    // Test generic scanner
    let generic = generic::GenericScanner::new();
    let generic_throughput = benchmark_scanner(&generic, data);
    results.push(("Generic".to_string(), generic_throughput));
    
    #[cfg(target_arch = "aarch64")]
    {
        let neon = neon::NeonScanner::new();
        let neon_throughput = benchmark_scanner(&neon, data);
        results.push(("NEON".to_string(), neon_throughput));
    }
    
    #[cfg(target_arch = "x86_64")]
    {
        if std::arch::is_x86_feature_detected!("sse2") {
            let sse2 = sse2::Sse2Scanner::new();
            let sse2_throughput = benchmark_scanner(&sse2, data);
            results.push(("SSE2".to_string(), sse2_throughput));
        }
        
        if std::arch::is_x86_feature_detected!("avx2") {
            let avx2 = avx2::Avx2Scanner::new();
            let avx2_throughput = benchmark_scanner(&avx2, data);
            results.push(("AVX2".to_string(), avx2_throughput));
        }
    }
    
    results
}

fn benchmark_scanner(scanner: &dyn SimdScanner, data: &[u8]) -> f64 {
    let iterations = 1000;
    let start = std::time::Instant::now();
    
    for _ in 0..iterations {
        let _positions = scanner.scan_structural(data);
    }
    
    let elapsed = start.elapsed();
    let data_size_mb = data.len() as f64 / 1024.0 / 1024.0;
    (data_size_mb * iterations as f64) / elapsed.as_secs_f64() / 1024.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_capabilities() {
        let caps = SimdCapabilities::detect();
        println!("SIMD Capabilities:");
        println!("  NEON: {}", caps.has_neon);
        println!("  AVX2: {}", caps.has_avx2);
        println!("  SSE2: {}", caps.has_sse2);
        println!("  Max vector bits: {}", caps.max_vector_bits);
        println!("  Best scanner: {}", caps.best_scanner_name());
        println!("  Estimated throughput: {:.1} GB/s", caps.estimated_throughput_gbps());
    }

    #[test]
    fn test_optimal_scanner() {
        let scanner = create_optimal_scanner();
        let caps = scanner.capabilities();
        
        println!("Optimal scanner: {}", caps.name);
        println!("Vector bits: {}", caps.vector_bits);
        println!("Estimated throughput: {:.1} GB/s", caps.estimated_throughput_gbps);
        
        // Test basic functionality
        let data = b"GET /test HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let structural = scanner.scan_structural(data);
        assert!(structural.len() > 0);
    }

    #[test]
    #[ignore] // Expensive benchmark test
    fn test_benchmark_all_scanners() {
        let data = b"GET /api/v1/test HTTP/1.1\r\nHost: example.com\r\nContent-Type: application/json\r\n\r\n{\"key\": \"value\"}" * 1000;
        
        let results = benchmark_all_scanners(&data);
        
        println!("Scanner benchmark results:");
        for (name, throughput) in results {
            println!("  {}: {:.2} GB/s", name, throughput);
        }
    }
}