// RBCursive - Rust port of BBCursive for network parser combinators
// High-performance SIMD-accelerated parsing with continuation-based streaming

pub mod simd;
pub mod combinators;
pub mod protocols;
pub mod continuation;
pub mod scanner;
pub mod patterns;

pub use simd::*;
pub use combinators::*;
pub use protocols::*;
pub use continuation::*;
pub use scanner::*;
pub use patterns::*;

// Remove unused import

/// Core RBCursive framework - the main entry point
pub struct RBCursive {
    scanner: Box<dyn SimdScanner>,
    pattern_scanner: PatternScanner,
}

impl RBCursive {
    /// Create new RBCursive instance with optimal SIMD strategy for current platform
    pub fn new() -> Self {
        use crate::rbcursive::simd::create_optimal_scanner;
        
        Self {
            scanner: create_optimal_scanner(),
            pattern_scanner: PatternScanner::new(),
        }
    }
    
    /// Get scanner reference
    pub fn scanner(&self) -> &dyn SimdScanner {
        self.scanner.as_ref()
    }
    
    /// Create HTTP parser using this RBCursive instance
    pub fn http_parser(&self) -> HttpParser {
        HttpParser::new()
    }
    
    /// Create SOCKS5 parser using this RBCursive instance
    pub fn socks5_parser(&self) -> Socks5Parser {
        Socks5Parser::new()
    }
    
    /// Create JSON parser for PAC files
    pub fn json_parser(&self) -> JsonParser {
        JsonParser::new()
    }
    
    /// Detect protocol from data using SIMD scanning
    pub fn detect_protocol(&self, data: &[u8]) -> ProtocolDetection {
        // Use SIMD to quickly scan for protocol markers
        let structural = self.scanner.scan_structural(data);
        let _quotes = self.scanner.scan_quotes(data);
        
        // Analyze patterns to determine protocol
        if data.len() >= 2 && data[0] == 0x05 {
            return ProtocolDetection::Socks5;
        }
        
        // Check for HTTP methods using SIMD-accelerated search
        if let Some(method) = self.detect_http_method(data) {
            return ProtocolDetection::Http(method);
        }
        
        // Check for JSON (PAC files)
        if !structural.is_empty() && data.get(0) == Some(&b'{') {
            return ProtocolDetection::Json;
        }
        
        ProtocolDetection::Unknown
    }
    
    /// Detect HTTP method using SIMD scanning
    fn detect_http_method(&self, data: &[u8]) -> Option<HttpMethod> {
        // SIMD scan for HTTP method terminators (space characters)
        let spaces = self.scanner.scan_bytes(data, &[b' ']);
        
        if let Some(&first_space) = spaces.first() {
            if first_space < data.len() {
                let method_bytes = &data[..first_space];
                return HttpMethod::from_bytes(method_bytes);
            }
        }
        
        None
    }
    
    /// Match glob patterns against data using SIMD acceleration
    pub fn match_glob(&self, data: &[u8], pattern: &str) -> PatternMatchResult {
        self.pattern_scanner.pattern_matcher.match_glob(data, pattern)
    }
    
    /// Match regex patterns against data using SIMD acceleration
    pub fn match_regex(&self, data: &[u8], pattern: &str) -> Result<PatternMatchResult, PatternError> {
        self.pattern_scanner.pattern_matcher.match_regex(data, pattern)
    }
    
    /// Find all glob pattern matches in data
    pub fn find_all_glob(&self, data: &[u8], pattern: &str) -> Vec<PatternMatch> {
        self.pattern_scanner.pattern_matcher.find_all_glob(data, pattern)
    }
    
    /// Find all regex pattern matches in data
    pub fn find_all_regex(&self, data: &[u8], pattern: &str) -> Result<Vec<PatternMatch>, PatternError> {
        self.pattern_scanner.pattern_matcher.find_all_regex(data, pattern)
    }
    
    /// SIMD-accelerated pattern scanning (optimized for large data)
    pub fn scan_with_pattern(&self, data: &[u8], pattern: &str, pattern_type: PatternType) -> Result<Vec<PatternMatch>, PatternError> {
        self.pattern_scanner.simd_guided_pattern_scan(data, pattern, pattern_type)
    }
    
    /// Get pattern matching capabilities
    pub fn pattern_capabilities(&self) -> PatternCapabilities {
        self.pattern_scanner.pattern_matcher.pattern_capabilities()
    }
}

impl Default for RBCursive {
    fn default() -> Self {
        Self::new()
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

/// Protocol detection result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolDetection {
    Http(HttpMethod),
    Socks5,
    Tls,
    Dns,
    WebSocket,
    Json,
    Unknown,
}

/// HTTP methods detected by SIMD scanning
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Head,
    Options,
    Connect,
    Patch,
    Trace,
}

impl HttpMethod {
    /// Convert bytes to HTTP method
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        match bytes {
            b"GET" => Some(Self::Get),
            b"POST" => Some(Self::Post),
            b"PUT" => Some(Self::Put),
            b"DELETE" => Some(Self::Delete),
            b"HEAD" => Some(Self::Head),
            b"OPTIONS" => Some(Self::Options),
            b"CONNECT" => Some(Self::Connect),
            b"PATCH" => Some(Self::Patch),
            b"TRACE" => Some(Self::Trace),
            _ => None,
        }
    }
    
    /// Convert to bytes
    pub fn as_bytes(&self) -> &'static [u8] {
        match self {
            Self::Get => b"GET",
            Self::Post => b"POST",
            Self::Put => b"PUT",
            Self::Delete => b"DELETE",
            Self::Head => b"HEAD",
            Self::Options => b"OPTIONS",
            Self::Connect => b"CONNECT",
            Self::Patch => b"PATCH",
            Self::Trace => b"TRACE",
        }
    }
}

/// Detect optimal SIMD strategy for current platform
pub fn detect_optimal_strategy() -> ScanStrategy {
    #[cfg(target_arch = "aarch64")]
    {
        // Apple Silicon and ARM64 - use NEON
        ScanStrategy::Simd
    }
    
    #[cfg(target_arch = "x86_64")]
    {
        // x86-64 - check for AVX2 support, fallback to autovec
        if std::arch::is_x86_feature_detected!("avx2") {
            ScanStrategy::Simd
        } else {
            ScanStrategy::Autovec
        }
    }
    
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        // Other architectures - use autovec
        ScanStrategy::Autovec
    }
}

/// Create SIMD scanner for given strategy
pub fn create_simd_scanner(strategy: ScanStrategy) -> Box<dyn SimdScanner> {
    match strategy {
        ScanStrategy::Scalar => Box::new(scanner::ScalarScanner::new()),
        ScanStrategy::Simd => {
            #[cfg(target_arch = "aarch64")]
            return Box::new(simd::neon::NeonScanner::new());
            
            #[cfg(target_arch = "x86_64")]
            return Box::new(simd::avx2::Avx2Scanner::new());
            
            #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
            return Box::new(scanner::ScalarScanner::new());
        }
        ScanStrategy::Autovec => Box::new(scanner::AutovecScanner::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rbcursive_creation() {
        let rbcursive = RBCursive::new();
    let _ = rbcursive; // smoke test
    }

    #[test]
    fn test_http_method_detection() {
        let rbcursive = RBCursive::new();
        let data = b"GET /test HTTP/1.1\r\n";
        
        match rbcursive.detect_protocol(data) {
            ProtocolDetection::Http(HttpMethod::Get) => (),
            other => panic!("Expected HTTP GET, got {:?}", other),
        }
    }

    #[test]
    fn test_socks5_detection() {
        let rbcursive = RBCursive::new();
        let data = b"\x05\x01\x00"; // SOCKS5 handshake
        
        match rbcursive.detect_protocol(data) {
            ProtocolDetection::Socks5 => (),
            other => panic!("Expected SOCKS5, got {:?}", other),
        }
    }

    #[test]
    fn test_glob_pattern_matching() {
        let rbcursive = RBCursive::new();
        
        // Test matching file extensions
        let filename = b"config.json";
        let result = rbcursive.match_glob(filename, "*.json");
        assert!(result.matched);
        assert_eq!(result.total_matches, 1);
        
        // Test non-matching pattern
        let result = rbcursive.match_glob(filename, "*.txt");
        assert!(!result.matched);
        assert_eq!(result.total_matches, 0);
    }

    #[test]
    fn test_regex_pattern_matching() {
        let rbcursive = RBCursive::new();
        
        // Test HTTP request parsing
        let http_data = b"GET /api/v1/users/123 HTTP/1.1";
        let result = rbcursive.match_regex(http_data, r"GET /api/v\d+/users/(\d+)").unwrap();
        assert!(result.matched);
        assert_eq!(result.total_matches, 1);
        
        // Verify capture group
        if let Some(match_result) = result.matches.first() {
            assert!(!match_result.captures.is_empty());
        }
    }

    #[test]
    fn test_pattern_scanner_integration() {
        let rbcursive = RBCursive::new();
        
        // Test SIMD-accelerated pattern scanning
        let log_data = b"2024-01-01 10:00:00 INFO Starting server\n2024-01-01 10:00:01 ERROR Failed to connect";
        let matches = rbcursive.scan_with_pattern(log_data, r"\d{4}-\d{2}-\d{2}", PatternType::Regex).unwrap();
        assert!(matches.len() >= 2); // Should find at least 2 dates
        
        // Test glob pattern scanning
        let file_list = b"test.log";
        let matches = rbcursive.scan_with_pattern(file_list, "*.log", PatternType::Glob).unwrap();
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_pattern_capabilities() {
        let rbcursive = RBCursive::new();
        let caps = rbcursive.pattern_capabilities();
        
        assert!(caps.supports_glob);
        assert!(caps.supports_regex);
        assert!(caps.max_pattern_length > 0);
        assert!(caps.max_data_size > 0);
    }
}