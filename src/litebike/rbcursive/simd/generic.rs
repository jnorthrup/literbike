// Generic SIMD scanner - optimized scalar implementation with compiler auto-vectorization hints
// Professional implementation focused on correctness and performance

use crate::rbcursive::scanner::{SimdScanner, ScannerCapabilities};

/// Generic scanner with auto-vectorization hints for any platform
pub struct GenericScanner;

impl GenericScanner {
    pub fn new() -> Self {
        Self
    }
}

impl SimdScanner for GenericScanner {
    fn scan_bytes(&self, data: &[u8], targets: &[u8]) -> Vec<usize> {
        if targets.len() == 1 {
            self.scan_single_byte(data, targets[0])
        } else {
            self.scan_multiple_bytes(data, targets)
        }
    }
    
    fn scan_structural(&self, data: &[u8]) -> Vec<usize> {
        // JSON/HTTP structural characters
        const STRUCTURAL: &[u8] = b"{}[](),:;\" \t\r\n";
        self.scan_multiple_bytes(data, STRUCTURAL)
    }
    
    fn scan_quotes(&self, data: &[u8]) -> Vec<usize> {
        self.scan_single_byte(data, b'"')
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
        bitmap.iter().map(|x| x.count_ones()).sum()
    }
    
    fn capabilities(&self) -> ScannerCapabilities {
        ScannerCapabilities {
            name: "Generic",
            vector_bits: 0,
            estimated_throughput_gbps: 0.1,
            supports_gather: true,
            supports_popcount: true,
        }
    }
}

impl GenericScanner {
    /// Optimized single byte search with auto-vectorization hints
    fn scan_single_byte(&self, data: &[u8], target: u8) -> Vec<usize> {
        let mut positions = Vec::new();
        
        // Process data in chunks to hint at vectorization opportunities
        const CHUNK_SIZE: usize = 64;
        let mut i = 0;
        
        while i + CHUNK_SIZE <= data.len() {
            let chunk = &data[i..i + CHUNK_SIZE];
            
            // Compiler should auto-vectorize this loop
            for (j, &byte) in chunk.iter().enumerate() {
                if byte == target {
                    positions.push(i + j);
                }
            }
            
            i += CHUNK_SIZE;
        }
        
        // Handle remaining bytes
        for (j, &byte) in data[i..].iter().enumerate() {
            if byte == target {
                positions.push(i + j);
            }
        }
        
        positions
    }
    
    /// Optimized multiple byte search using lookup table
    fn scan_multiple_bytes(&self, data: &[u8], targets: &[u8]) -> Vec<usize> {
        // Create lookup table for O(1) membership testing
        let mut lookup = [false; 256];
        for &target in targets {
            lookup[target as usize] = true;
        }
        
        let mut positions = Vec::new();
        
        // Process in chunks with auto-vectorization hints
        const CHUNK_SIZE: usize = 64;
        let mut i = 0;
        
        while i + CHUNK_SIZE <= data.len() {
            let chunk = &data[i..i + CHUNK_SIZE];
            
            // This should auto-vectorize well with lookup table
            for (j, &byte) in chunk.iter().enumerate() {
                if lookup[byte as usize] {
                    positions.push(i + j);
                }
            }
            
            i += CHUNK_SIZE;
        }
        
        // Handle remaining bytes
        for (j, &byte) in data[i..].iter().enumerate() {
            if lookup[byte as usize] {
                positions.push(i + j);
            }
        }
        
        positions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generic_scanner_basic() {
        let scanner = GenericScanner::new();
        let data = b"GET /test HTTP/1.1\r\nHost: example.com\r\n\r\n";
        
        let spaces = scanner.scan_bytes(data, &[b' ']);
        assert!(spaces.len() >= 2);
        
        let structural = scanner.scan_structural(data);
        assert!(structural.len() > 0);
        
        let quotes = scanner.scan_quotes(data);
        // No quotes in this HTTP request
        assert_eq!(quotes.len(), 0);
    }

    #[test]
    fn test_generic_scanner_json() {
        let scanner = GenericScanner::new();
        let data = b"{\"key\": \"value\", \"array\": [1, 2, 3]}";
        
        let quotes = scanner.scan_quotes(data);
        assert_eq!(quotes.len(), 6); // 6 quote characters
        
        let braces = scanner.scan_bytes(data, &[b'{', b'}']);
        assert_eq!(braces.len(), 2);
        
        let brackets = scanner.scan_bytes(data, &[b'[', b']']);
        assert_eq!(brackets.len(), 2);
    }

    #[test]
    fn test_gather_operation() {
        let scanner = GenericScanner::new();
        let data = b"abcdefghij";
        let positions = vec![0, 2, 4, 6, 8];
        
        let gathered = scanner.gather_bytes(data, &positions);
        assert_eq!(gathered, b"acegi");
    }

    #[test]
    fn test_popcount() {
        let scanner = GenericScanner::new();
        let bitmap = vec![0xFFFFFFFF, 0x00000000, 0x12345678];
        
        let count = scanner.popcount(&bitmap);
        let expected = 32 + 0 + 0x12345678u32.count_ones();
        assert_eq!(count, expected);
    }
}