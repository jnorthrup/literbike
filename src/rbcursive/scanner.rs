// RBCursive SIMD Scanner - Core scanning trait and implementations
// Port of BBCursive SIMD scanning functionality to Rust

// Remove unused import

/// Core SIMD scanner trait - BBCursive-style pattern scanning
pub trait SimdScanner: Send + Sync {
    /// Scan for all occurrences of a single byte
    fn scan_bytes(&self, data: &[u8], targets: &[u8]) -> Vec<usize>;
    
    /// Scan for structural characters (JSON/HTTP delimiters)
    fn scan_structural(&self, data: &[u8]) -> Vec<usize>;
    
    /// Scan for quote characters
    fn scan_quotes(&self, data: &[u8]) -> Vec<usize>;
    
    /// Scan for any of multiple target bytes
    fn scan_any_byte(&self, data: &[u8], targets: &[u8]) -> Vec<usize>;
    
    /// Gather bytes at specific positions (SIMD gather operation)
    fn gather_bytes(&self, data: &[u8], positions: &[usize]) -> Vec<u8>;
    
    /// Population count (count set bits in bitmap)
    fn popcount(&self, bitmap: &[u32]) -> u32;
    
    /// Get scanner capabilities
    fn capabilities(&self) -> ScannerCapabilities;
}

/// Scanner capabilities and performance characteristics
#[derive(Debug, Clone)]
pub struct ScannerCapabilities {
    pub name: &'static str,
    pub vector_bits: u32,
    pub estimated_throughput_gbps: f64,
    pub supports_gather: bool,
    pub supports_popcount: bool,
}

/// Scalar fallback scanner - pure Rust implementation
pub struct ScalarScanner;

impl ScalarScanner {
    pub fn new() -> Self {
        Self
    }
}

impl SimdScanner for ScalarScanner {
    fn scan_bytes(&self, data: &[u8], targets: &[u8]) -> Vec<usize> {
        let mut positions = Vec::new();
        
        if targets.len() == 1 {
            let target = targets[0];
            for (i, &byte) in data.iter().enumerate() {
                if byte == target {
                    positions.push(i);
                }
            }
        } else {
            for (i, &byte) in data.iter().enumerate() {
                if targets.contains(&byte) {
                    positions.push(i);
                }
            }
        }
        
        positions
    }
    
    fn scan_structural(&self, data: &[u8]) -> Vec<usize> {
        // JSON/HTTP structural characters: {}[](),:;"<space><tab><cr><lf>
        const STRUCTURAL: &[u8] = b"{}[](),:;\" \t\r\n";
        self.scan_bytes(data, STRUCTURAL)
    }
    
    fn scan_quotes(&self, data: &[u8]) -> Vec<usize> {
        self.scan_bytes(data, &[b'"'])
    }
    
    fn scan_any_byte(&self, data: &[u8], targets: &[u8]) -> Vec<usize> {
        self.scan_bytes(data, targets)
    }
    
    fn gather_bytes(&self, data: &[u8], positions: &[usize]) -> Vec<u8> {
        positions.iter()
            .filter_map(|&pos| data.get(pos))
            .copied()
            .collect()
    }
    
    fn popcount(&self, bitmap: &[u32]) -> u32 {
        bitmap.iter().map(|x| x.count_ones()).sum()
    }
    
    fn capabilities(&self) -> ScannerCapabilities {
        ScannerCapabilities {
            name: "Scalar",
            vector_bits: 0,
            estimated_throughput_gbps: 0.05, // ~50 MB/s
            supports_gather: true,
            supports_popcount: true,
        }
    }
}

/// Auto-vectorization scanner - relies on compiler optimization
pub struct AutovecScanner;

impl AutovecScanner {
    pub fn new() -> Self {
        Self
    }
}

impl SimdScanner for AutovecScanner {
    fn scan_bytes(&self, data: &[u8], targets: &[u8]) -> Vec<usize> {
        let mut positions = Vec::new();
        
        if targets.len() == 1 {
            let target = targets[0];
            // Compiler should auto-vectorize this loop
            for (i, &byte) in data.iter().enumerate() {
                if byte == target {
                    positions.push(i);
                }
            }
        } else {
            // For multiple targets, use a lookup table approach that's vectorizable
            let mut lookup = [false; 256];
            for &target in targets {
                lookup[target as usize] = true;
            }
            
            for (i, &byte) in data.iter().enumerate() {
                if lookup[byte as usize] {
                    positions.push(i);
                }
            }
        }
        
        positions
    }
    
    fn scan_structural(&self, data: &[u8]) -> Vec<usize> {
        // Use lookup table for better auto-vectorization
        let mut is_structural = [false; 256];
        for &ch in b"{}[](),:;\" \t\r\n" {
            is_structural[ch as usize] = true;
        }
        
        let mut positions = Vec::new();
        for (i, &byte) in data.iter().enumerate() {
            if is_structural[byte as usize] {
                positions.push(i);
            }
        }
        
        positions
    }
    
    fn scan_quotes(&self, data: &[u8]) -> Vec<usize> {
        self.scan_bytes(data, &[b'"'])
    }
    
    fn scan_any_byte(&self, data: &[u8], targets: &[u8]) -> Vec<usize> {
        self.scan_bytes(data, targets)
    }
    
    fn gather_bytes(&self, data: &[u8], positions: &[usize]) -> Vec<u8> {
        // Auto-vectorizable gather
        positions.iter()
            .filter_map(|&pos| data.get(pos))
            .copied()
            .collect()
    }
    
    fn popcount(&self, bitmap: &[u32]) -> u32 {
        // Should auto-vectorize with SIMD popcount instructions
        bitmap.iter().map(|x| x.count_ones()).sum()
    }
    
    fn capabilities(&self) -> ScannerCapabilities {
        ScannerCapabilities {
            name: "Autovec",
            vector_bits: 128, // Assume 128-bit vectors
            estimated_throughput_gbps: 0.5, // ~500 MB/s with good auto-vectorization
            supports_gather: true,
            supports_popcount: true,
        }
    }
}

/// Benchmarking utilities for scanner performance
pub struct ScannerBenchmark {
    pub scanner: Box<dyn SimdScanner>,
    pub data_size_mb: f64,
}

impl ScannerBenchmark {
    pub fn new(scanner: Box<dyn SimdScanner>, data: &[u8]) -> Self {
        Self {
            scanner,
            data_size_mb: data.len() as f64 / 1024.0 / 1024.0,
        }
    }
    
    pub fn benchmark_structural_scan(&self, data: &[u8], iterations: usize) -> BenchmarkResult {
        let start = std::time::Instant::now();
        
        for _ in 0..iterations {
            let _positions = self.scanner.scan_structural(data);
        }
        
        let elapsed = start.elapsed();
        let throughput_gbps = (self.data_size_mb * iterations as f64) / elapsed.as_secs_f64() / 1024.0;
        
        BenchmarkResult {
            operation: "Structural Scan".to_string(),
            iterations,
            elapsed,
            throughput_gbps,
            capabilities: self.scanner.capabilities(),
        }
    }
    
    pub fn benchmark_quote_scan(&self, data: &[u8], iterations: usize) -> BenchmarkResult {
        let start = std::time::Instant::now();
        
        for _ in 0..iterations {
            let _positions = self.scanner.scan_quotes(data);
        }
        
        let elapsed = start.elapsed();
        let throughput_gbps = (self.data_size_mb * iterations as f64) / elapsed.as_secs_f64() / 1024.0;
        
        BenchmarkResult {
            operation: "Quote Scan".to_string(),
            iterations,
            elapsed,
            throughput_gbps,
            capabilities: self.scanner.capabilities(),
        }
    }
}

#[derive(Debug)]
pub struct BenchmarkResult {
    pub operation: String,
    pub iterations: usize,
    pub elapsed: std::time::Duration,
    pub throughput_gbps: f64,
    pub capabilities: ScannerCapabilities,
}

impl BenchmarkResult {
    pub fn print_summary(&self) {
        println!("=== {} Benchmark ===", self.operation);
        println!("Scanner: {}", self.capabilities.name);
        println!("Vector bits: {}", self.capabilities.vector_bits);
        println!("Iterations: {}", self.iterations);
        println!("Elapsed: {:?}", self.elapsed);
        println!("Throughput: {:.2} GB/s", self.throughput_gbps);
        println!("Estimated: {:.2} GB/s", self.capabilities.estimated_throughput_gbps);
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_scanner() {
        let scanner = ScalarScanner::new();
        let data = b"GET /test HTTP/1.1\r\nHost: example.com\r\n\r\n";
        
        let quotes = scanner.scan_quotes(data);
        assert_eq!(quotes.len(), 0); // No quotes in this data
        
        let structural = scanner.scan_structural(data);
        assert!(structural.len() > 0); // Should find spaces, colons, etc.
        
        let spaces = scanner.scan_bytes(data, &[b' ']);
        assert!(spaces.len() >= 2); // At least 2 spaces in HTTP request
    }

    #[test]
    fn test_autovec_scanner() {
        let scanner = AutovecScanner::new();
        let data = b"POST /api HTTP/1.1\r\nContent-Type: application/json\r\n\r\n{\"key\": \"value\"}";
        
        let quotes = scanner.scan_quotes(data);
        assert!(quotes.len() >= 4); // At least 4 quotes in JSON
        
        let structural = scanner.scan_structural(data);
        assert!(structural.len() > 0); // Should find various structural chars
        
        let braces = scanner.scan_bytes(data, &[b'{', b'}']);
        assert_eq!(braces.len(), 2); // One opening, one closing brace
    }

    #[test]
    fn test_gather_operation() {
        let scanner = ScalarScanner::new();
        let data = b"abcdefghij";
        let positions = vec![0, 2, 4, 6, 8];
        
        let gathered = scanner.gather_bytes(data, &positions);
        assert_eq!(gathered, b"acegi");
    }

    #[test]
    fn test_benchmark() {
        let scanner = Box::new(ScalarScanner::new());
    let mut data = Vec::new();
    for _ in 0..1000 { data.extend_from_slice(b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n"); }
    let data = data;
        
        let benchmark = ScannerBenchmark::new(scanner, &data);
        let result = benchmark.benchmark_structural_scan(&data, 10);
        
        result.print_summary();
        assert!(result.throughput_gbps > 0.0);
    }
}

/// Create SIMD scanner for given strategy
pub fn create_simd_scanner(strategy: ScanStrategy) -> Box<dyn SimdScanner> {
    match strategy {
        ScanStrategy::Scalar => Box::new(ScalarScanner::new()),
        ScanStrategy::Autovec => Box::new(AutovecScanner::new()),
        ScanStrategy::Simd => {
            // Will be implemented with actual SIMD when simd module is available
            Box::new(AutovecScanner::new())
        }
    }
}

/// SIMD strategy selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanStrategy {
    /// Pure scalar implementation - no SIMD
    Scalar,
    /// SIMD intrinsics (NEON on ARM, AVX2 on x86)
    Simd,
    /// Compiler auto-vectorization (default)
    Autovec,
}