// SSE2 SIMD implementation for x86-64 - baseline 128-bit vector operations

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use crate::rbcursive::scanner::{SimdScanner, ScannerCapabilities};

/// SSE2-accelerated scanner for x86-64 platforms (baseline requirement)
pub struct Sse2Scanner;

impl Sse2Scanner {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(target_arch = "x86_64")]
impl SimdScanner for Sse2Scanner {
    fn scan_bytes(&self, data: &[u8], targets: &[u8]) -> Vec<usize> {
        if targets.len() == 1 {
            self.scan_single_byte_sse2(data, targets[0])
        } else {
            self.scan_multiple_bytes_sse2(data, targets)
        }
    }
    
    fn scan_structural(&self, data: &[u8]) -> Vec<usize> {
        const STRUCTURAL: &[u8] = b"{}[](),:;\" \t\r\n";
        self.scan_multiple_bytes_sse2(data, STRUCTURAL)
    }
    
    fn scan_quotes(&self, data: &[u8]) -> Vec<usize> {
        self.scan_single_byte_sse2(data, b'"')
    }
    
    fn scan_any_byte(&self, data: &[u8], targets: &[u8]) -> Vec<usize> {
        self.scan_bytes(data, targets)
    }
    
    fn gather_bytes(&self, data: &[u8], positions: &[usize]) -> Vec<u8> {
        let mut result = Vec::with_capacity(positions.len());
        for &pos in positions {
            if let Some(&byte) = data.get(pos) {
                result.push(byte);
            }
        }
        result
    }
    
    fn popcount(&self, bitmap: &[u32]) -> u32 {
        unsafe {
            let mut total = 0u32;
            
            // Process 4 u32s at a time using SSE2
            for chunk in bitmap.chunks_exact(4) {
                let v = _mm_loadu_si128(chunk.as_ptr() as *const __m128i);
                total += self.sse2_popcount_epi32(v);
            }
            
            // Handle remaining elements
            for &val in bitmap.chunks_exact(4).remainder() {
                total += val.count_ones();
            }
            
            total
        }
    }
    
    fn capabilities(&self) -> ScannerCapabilities {
        ScannerCapabilities {
            name: "SSE2",
            vector_bits: 128,
            estimated_throughput_gbps: 1.5,
            supports_gather: true,
            supports_popcount: true,
        }
    }
}

#[cfg(target_arch = "x86_64")]
impl Sse2Scanner {
    /// SSE2-optimized single byte scanning (16 bytes at a time)
    fn scan_single_byte_sse2(&self, data: &[u8], target: u8) -> Vec<usize> {
        let mut positions = Vec::new();
        
        unsafe {
            let target_vec = _mm_set1_epi8(target as i8);
            let mut i = 0;
            
            // Process 16 bytes at a time using SSE2
            while i + 16 <= data.len() {
                let chunk = _mm_loadu_si128(data.as_ptr().add(i) as *const __m128i);
                let cmp = _mm_cmpeq_epi8(chunk, target_vec);
                
                // Convert comparison result to bitmask
                let mask = _mm_movemask_epi8(cmp) as u16;
                
                // Extract positions from bitmask
                let mut bit_mask = mask;
                let mut bit_pos = 0;
                while bit_mask != 0 {
                    if (bit_mask & 1) != 0 {
                        positions.push(i + bit_pos);
                    }
                    bit_mask >>= 1;
                    bit_pos += 1;
                }
                
                i += 16;
            }
            
            // Handle remaining bytes
            while i < data.len() {
                if data[i] == target {
                    positions.push(i);
                }
                i += 1;
            }
        }
        
        positions
    }
    
    /// SSE2-optimized multiple byte scanning
    fn scan_multiple_bytes_sse2(&self, data: &[u8], targets: &[u8]) -> Vec<usize> {
        // For small target sets, use parallel comparison
        if targets.len() <= 4 {
            return self.scan_parallel_comparison_sse2(data, targets);
        }
        
        // For larger sets, use lookup table
        self.scan_lookup_table_sse2(data, targets)
    }
    
    /// SSE2 parallel comparison for small target sets
    fn scan_parallel_comparison_sse2(&self, data: &[u8], targets: &[u8]) -> Vec<usize> {
        let mut positions = Vec::new();
        
        unsafe {
            // Create target vectors (up to 4 targets efficiently)
            let target_vecs: Vec<__m128i> = targets.iter().take(4)
                .map(|&t| _mm_set1_epi8(t as i8))
                .collect();
            
            let mut i = 0;
            
            while i + 16 <= data.len() {
                let chunk = _mm_loadu_si128(data.as_ptr().add(i) as *const __m128i);
                let mut combined_mask = 0u16;
                
                // Compare against each target
                for target_vec in &target_vecs {
                    let cmp = _mm_cmpeq_epi8(chunk, *target_vec);
                    let mask = _mm_movemask_epi8(cmp) as u16;
                    combined_mask |= mask;
                }
                
                // Extract positions from combined bitmask
                let mut bit_mask = combined_mask;
                let mut bit_pos = 0;
                while bit_mask != 0 {
                    if (bit_mask & 1) != 0 {
                        positions.push(i + bit_pos);
                    }
                    bit_mask >>= 1;
                    bit_pos += 1;
                }
                
                i += 16;
            }
            
            // Handle remaining bytes
            while i < data.len() {
                if targets.contains(&data[i]) {
                    positions.push(i);
                }
                i += 1;
            }
        }
        
        positions
    }
    
    /// SSE2 lookup table approach
    fn scan_lookup_table_sse2(&self, data: &[u8], targets: &[u8]) -> Vec<usize> {
        // Create lookup table
        let mut lookup = [false; 256];
        for &target in targets {
            lookup[target as usize] = true;
        }
        
        let mut positions = Vec::new();
        
        // Use scalar approach with lookup table for now
        // Full SSE2 table lookup requires more complex implementation
        for (i, &byte) in data.iter().enumerate() {
            if lookup[byte as usize] {
                positions.push(i);
            }
        }
        
        positions
    }
    
    /// SSE2 popcount implementation
    unsafe fn sse2_popcount_epi32(&self, v: __m128i) -> u32 {
        // Extract 4 u32 values and use hardware popcount
        let array: [u32; 4] = std::mem::transmute(v);
        array.iter().map(|x| x.count_ones()).sum()
    }
}

// Stub implementation for non-x86_64 platforms
#[cfg(not(target_arch = "x86_64"))]
impl SimdScanner for Sse2Scanner {
    fn scan_bytes(&self, data: &[u8], targets: &[u8]) -> Vec<usize> {
        crate::rbcursive::scanner::ScalarScanner::new().scan_bytes(data, targets)
    }
    
    fn scan_structural(&self, data: &[u8]) -> Vec<usize> {
        crate::rbcursive::scanner::ScalarScanner::new().scan_structural(data)
    }
    
    fn scan_quotes(&self, data: &[u8]) -> Vec<usize> {
        crate::rbcursive::scanner::ScalarScanner::new().scan_quotes(data)
    }
    
    fn scan_any_byte(&self, data: &[u8], targets: &[u8]) -> Vec<usize> {
        crate::rbcursive::scanner::ScalarScanner::new().scan_any_byte(data, targets)
    }
    
    fn gather_bytes(&self, data: &[u8], positions: &[usize]) -> Vec<u8> {
        crate::rbcursive::scanner::ScalarScanner::new().gather_bytes(data, positions)
    }
    
    fn popcount(&self, bitmap: &[u32]) -> u32 {
        crate::rbcursive::scanner::ScalarScanner::new().popcount(bitmap)
    }
    
    fn capabilities(&self) -> ScannerCapabilities {
        ScannerCapabilities {
            name: "SSE2 (not available)",
            vector_bits: 0,
            estimated_throughput_gbps: 0.05,
            supports_gather: false,
            supports_popcount: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse2_scanner() {
        let scanner = Sse2Scanner::new();
        let data = b"CONNECT proxy.example.com:443 HTTP/1.1\r\nHost: proxy.example.com:443\r\n\r\n";
        
        let spaces = scanner.scan_bytes(data, &[b' ']);
        assert!(spaces.len() >= 2);
        
        let structural = scanner.scan_structural(data);
        assert!(structural.len() > 0);
        
        let colons = scanner.scan_bytes(data, &[b':']);
        assert!(colons.len() >= 2); // Port numbers and Host header
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_sse2_parallel_comparison() {
        if !std::arch::is_x86_feature_detected!("sse2") {
            return;
        }
        
        let scanner = Sse2Scanner::new();
        let data = b"POST /api HTTP/1.1\r\nContent-Type: text/plain\r\n\r\ntest data";
        
        // Test with multiple targets
        let delimiters = scanner.scan_bytes(data, &[b' ', b':', b'\r', b'\n']);
        assert!(delimiters.len() > 0);
        
        // Should find spaces, colons, and line endings
        let expected_chars = [b' ', b':', b'\r', b'\n'];
        for &pos in &delimiters {
            if let Some(&byte) = data.get(pos) {
                assert!(expected_chars.contains(&byte));
            }
        }
    }
}
