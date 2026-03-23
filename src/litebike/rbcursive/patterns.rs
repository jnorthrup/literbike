// RBCursive Pattern Matching - Glob and Regex support
// Extends the SIMD scanner with pattern matching capabilities

use std::collections::HashMap;
use regex::Regex;
use glob::Pattern as GlobPattern;

/// Pattern matching trait for extending rbCursive with glob and regex support
pub trait PatternMatcher: Send + Sync {
    /// Match glob patterns against byte data
    fn match_glob(&self, data: &[u8], pattern: &str) -> PatternMatchResult;
    
    /// Match regex patterns against byte data  
    fn match_regex(&self, data: &[u8], pattern: &str) -> Result<PatternMatchResult, PatternError>;
    
    /// Find all glob pattern matches in data
    fn find_all_glob(&self, data: &[u8], pattern: &str) -> Vec<PatternMatch>;
    
    /// Find all regex pattern matches in data
    fn find_all_regex(&self, data: &[u8], pattern: &str) -> Result<Vec<PatternMatch>, PatternError>;
    
    /// Get pattern matcher capabilities
    fn pattern_capabilities(&self) -> PatternCapabilities;
}

/// Pattern matching result
#[derive(Debug, Clone, PartialEq)]
pub struct PatternMatchResult {
    pub matched: bool,
    pub matches: Vec<PatternMatch>,
    pub total_matches: usize,
}

/// Individual pattern match
#[derive(Debug, Clone, PartialEq)]
pub struct PatternMatch {
    pub start: usize,
    pub end: usize,
    pub text: Vec<u8>,
    pub captures: Vec<PatternCapture>,
}

/// Pattern capture group
#[derive(Debug, Clone, PartialEq)]
pub struct PatternCapture {
    pub name: Option<String>,
    pub start: usize,
    pub end: usize,
    pub text: Vec<u8>,
}

/// Pattern matching error types
#[derive(Debug, Clone, PartialEq)]
pub enum PatternError {
    InvalidRegex(String),
    InvalidGlob(String),
    DataTooLarge,
    EncodingError,
}

/// Pattern matcher capabilities
#[derive(Debug, Clone)]
pub struct PatternCapabilities {
    pub supports_glob: bool,
    pub supports_regex: bool,
    pub supports_unicode: bool,
    pub max_pattern_length: usize,
    pub max_data_size: usize,
}

/// SIMD-accelerated pattern matcher implementation
pub struct SimdPatternMatcher {
    regex_cache: HashMap<String, Regex>,
    glob_cache: HashMap<String, GlobPattern>,
    max_cache_size: usize,
}

impl SimdPatternMatcher {
    pub fn new() -> Self {
        Self {
            regex_cache: HashMap::new(),
            glob_cache: HashMap::new(),
            max_cache_size: 1000, // Cache up to 1000 compiled patterns
        }
    }
    
    pub fn with_cache_size(cache_size: usize) -> Self {
        Self {
            regex_cache: HashMap::new(),
            glob_cache: HashMap::new(),
            max_cache_size: cache_size,
        }
    }
    
    /// Get or compile regex pattern with caching
    fn get_regex(&mut self, pattern: &str) -> Result<&Regex, PatternError> {
        if !self.regex_cache.contains_key(pattern) {
            if self.regex_cache.len() >= self.max_cache_size {
                // Simple eviction: clear cache when full
                self.regex_cache.clear();
            }
            
            let regex = Regex::new(pattern)
                .map_err(|e| PatternError::InvalidRegex(e.to_string()))?;
            self.regex_cache.insert(pattern.to_string(), regex);
        }
        
        Ok(self.regex_cache.get(pattern).unwrap())
    }
    
    /// Get or compile glob pattern with caching
    fn get_glob(&mut self, pattern: &str) -> Result<&GlobPattern, PatternError> {
        if !self.glob_cache.contains_key(pattern) {
            if self.glob_cache.len() >= self.max_cache_size {
                // Simple eviction: clear cache when full
                self.glob_cache.clear();
            }
            
            let glob = GlobPattern::new(pattern)
                .map_err(|e| PatternError::InvalidGlob(e.to_string()))?;
            self.glob_cache.insert(pattern.to_string(), glob);
        }
        
        Ok(self.glob_cache.get(pattern).unwrap())
    }
    
    /// Convert bytes to string safely
    fn bytes_to_str(data: &[u8]) -> Result<&str, PatternError> {
        std::str::from_utf8(data).map_err(|_| PatternError::EncodingError)
    }
}

impl PatternMatcher for SimdPatternMatcher {
    fn match_glob(&self, data: &[u8], pattern: &str) -> PatternMatchResult {
        let mut matcher = Self::new();
        
        match Self::bytes_to_str(data) {
            Ok(text) => {
                match matcher.get_glob(pattern) {
                    Ok(glob) => {
                        let matched = glob.matches(text);
                        let matches = if matched {
                            vec![PatternMatch {
                                start: 0,
                                end: data.len(),
                                text: data.to_vec(),
                                captures: vec![],
                            }]
                        } else {
                            vec![]
                        };
                        
                        PatternMatchResult {
                            matched,
                            total_matches: matches.len(),
                            matches,
                        }
                    }
                    Err(_) => PatternMatchResult {
                        matched: false,
                        total_matches: 0,
                        matches: vec![],
                    }
                }
            }
            Err(_) => PatternMatchResult {
                matched: false,
                total_matches: 0,
                matches: vec![],
            }
        }
    }
    
    fn match_regex(&self, data: &[u8], pattern: &str) -> Result<PatternMatchResult, PatternError> {
        let mut matcher = Self::new();
        let text = Self::bytes_to_str(data)?;
        let regex = matcher.get_regex(pattern)?;
        
        let matched = regex.is_match(text);
        let matches = if matched {
            if let Some(capture) = regex.captures(text) {
                let full_match = capture.get(0).unwrap();
                let mut captures = vec![];
                
                // Add named captures
                for name in regex.capture_names().flatten() {
                    if let Some(cap) = capture.name(name) {
                        captures.push(PatternCapture {
                            name: Some(name.to_string()),
                            start: cap.start(),
                            end: cap.end(),
                            text: cap.as_str().as_bytes().to_vec(),
                        });
                    }
                }
                
                // Add numbered captures
                for (i, cap) in capture.iter().enumerate().skip(1) {
                    if let Some(cap) = cap {
                        captures.push(PatternCapture {
                            name: None,
                            start: cap.start(),
                            end: cap.end(),
                            text: cap.as_str().as_bytes().to_vec(),
                        });
                    }
                }
                
                vec![PatternMatch {
                    start: full_match.start(),
                    end: full_match.end(),
                    text: full_match.as_str().as_bytes().to_vec(),
                    captures,
                }]
            } else {
                vec![]
            }
        } else {
            vec![]
        };
        
        Ok(PatternMatchResult {
            matched,
            total_matches: matches.len(),
            matches,
        })
    }
    
    fn find_all_glob(&self, data: &[u8], pattern: &str) -> Vec<PatternMatch> {
        // For glob patterns, we typically match entire strings/paths
        // This is a simplified implementation that treats the entire data as one string
        match self.match_glob(data, pattern) {
            result if result.matched => result.matches,
            _ => vec![],
        }
    }
    
    fn find_all_regex(&self, data: &[u8], pattern: &str) -> Result<Vec<PatternMatch>, PatternError> {
        let mut matcher = Self::new();
        let text = Self::bytes_to_str(data)?;
        let regex = matcher.get_regex(pattern)?;
        
        let mut matches = vec![];
        
        for capture in regex.captures_iter(text) {
            let full_match = capture.get(0).unwrap();
            let mut captures = vec![];
            
            // Add named captures
            for name in regex.capture_names().flatten() {
                if let Some(cap) = capture.name(name) {
                    captures.push(PatternCapture {
                        name: Some(name.to_string()),
                        start: cap.start(),
                        end: cap.end(),
                        text: cap.as_str().as_bytes().to_vec(),
                    });
                }
            }
            
            // Add numbered captures
            for (i, cap) in capture.iter().enumerate().skip(1) {
                if let Some(cap) = cap {
                    captures.push(PatternCapture {
                        name: None,
                        start: cap.start(),
                        end: cap.end(),
                        text: cap.as_str().as_bytes().to_vec(),
                    });
                }
            }
            
            matches.push(PatternMatch {
                start: full_match.start(),
                end: full_match.end(),
                text: full_match.as_str().as_bytes().to_vec(),
                captures,
            });
        }
        
        Ok(matches)
    }
    
    fn pattern_capabilities(&self) -> PatternCapabilities {
        PatternCapabilities {
            supports_glob: true,
            supports_regex: true,
            supports_unicode: true,
            max_pattern_length: 10000,
            max_data_size: 100 * 1024 * 1024, // 100MB max
        }
    }
}

impl Default for SimdPatternMatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Pattern scanner that combines SIMD scanning with pattern matching
pub struct PatternScanner {
    pub simd_scanner: Box<dyn crate::rbcursive::SimdScanner>,
    pub pattern_matcher: Box<dyn PatternMatcher>,
}

impl PatternScanner {
    pub fn new() -> Self {
        Self {
            simd_scanner: crate::rbcursive::simd::create_optimal_scanner(),
            pattern_matcher: Box::new(SimdPatternMatcher::new()),
        }
    }
    
    /// Fast pattern-guided scanning using SIMD acceleration
    pub fn scan_with_pattern(&self, data: &[u8], pattern: &str, pattern_type: PatternType) -> Result<Vec<PatternMatch>, PatternError> {
        match pattern_type {
            PatternType::Glob => Ok(self.pattern_matcher.find_all_glob(data, pattern)),
            PatternType::Regex => self.pattern_matcher.find_all_regex(data, pattern),
        }
    }
    
    /// Use SIMD to pre-filter data before pattern matching (optimization)
    pub fn simd_guided_pattern_scan(&self, data: &[u8], pattern: &str, pattern_type: PatternType) -> Result<Vec<PatternMatch>, PatternError> {
        // Extract potential pattern start characters to accelerate search
        let pattern_hints = self.extract_pattern_hints(pattern, &pattern_type);
        
        if !pattern_hints.is_empty() {
            // Use SIMD to find potential match positions
            let candidates = self.simd_scanner.scan_any_byte(data, &pattern_hints);
            
            // For large datasets, only check regions around candidate positions
            if candidates.len() > 100 && data.len() > 10000 {
                return self.scan_candidate_regions(data, pattern, pattern_type, &candidates);
            }
        }
        
        // Fallback to full scan for small data or few candidates
        self.scan_with_pattern(data, pattern, pattern_type)
    }
    
    /// Extract hint bytes that might indicate pattern start positions
    fn extract_pattern_hints(&self, pattern: &str, pattern_type: &PatternType) -> Vec<u8> {
        match pattern_type {
            PatternType::Glob => {
                // For glob patterns, look for literal characters that aren't wildcards
                pattern.bytes()
                    .filter(|&b| b != b'*' && b != b'?' && b != b'[' && b != b']')
                    .take(5) // Limit to first 5 literal chars
                    .collect()
            }
            PatternType::Regex => {
                // For regex, extract literal prefix characters
                pattern.bytes()
                    .take_while(|&b| {
                        // Stop at regex metacharacters
                        !b"^$.*+?{}[]()\\|".contains(&b)
                    })
                    .take(5) // Limit to first 5 literal chars
                    .collect()
            }
        }
    }
    
    /// Scan regions around candidate positions (optimization for large data)
    fn scan_candidate_regions(&self, data: &[u8], pattern: &str, pattern_type: PatternType, candidates: &[usize]) -> Result<Vec<PatternMatch>, PatternError> {
        let mut all_matches = vec![];
        let region_size = 1024; // Scan 1KB regions around each candidate
        
        for &candidate_pos in candidates {
            let start = candidate_pos.saturating_sub(region_size / 2);
            let end = (candidate_pos + region_size / 2).min(data.len());
            let region = &data[start..end];
            
            let mut region_matches = self.scan_with_pattern(region, pattern, pattern_type)?;
            
            // Adjust match positions to global coordinates
            for match_result in &mut region_matches {
                match_result.start += start;
                match_result.end += start;
                for capture in &mut match_result.captures {
                    capture.start += start;
                    capture.end += start;
                }
            }
            
            all_matches.extend(region_matches);
        }
        
        // Remove duplicates and sort by position
        all_matches.sort_by_key(|m| m.start);
        all_matches.dedup_by_key(|m| m.start);
        
        Ok(all_matches)
    }
}

impl Default for PatternScanner {
    fn default() -> Self {
        Self::new()
    }
}

/// Pattern type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternType {
    Glob,
    Regex,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_pattern_matching() {
        let matcher = SimdPatternMatcher::new();
        let data = b"test.txt";
        
        let result = matcher.match_glob(data, "*.txt");
        assert!(result.matched);
        assert_eq!(result.total_matches, 1);
        
        let result = matcher.match_glob(data, "*.log");
        assert!(!result.matched);
        assert_eq!(result.total_matches, 0);
    }

    #[test]
    fn test_regex_pattern_matching() {
        let matcher = SimdPatternMatcher::new();
        let data = b"GET /api/v1/users/123 HTTP/1.1";
        
        let result = matcher.match_regex(data, r"GET /api/v\d+/users/(\d+)").unwrap();
        assert!(result.matched);
        assert_eq!(result.total_matches, 1);
        
        if let Some(match_result) = result.matches.first() {
            assert!(!match_result.captures.is_empty());
            // Should capture the user ID "123"
            let user_id_capture = &match_result.captures[0];
            assert_eq!(user_id_capture.text, b"123");
        }
    }

    #[test]
    fn test_pattern_scanner() {
        let scanner = PatternScanner::new();
        let data = b"Content-Type: application/json\nContent-Length: 100\nUser-Agent: test";
        
        // Test regex for HTTP headers
        let matches = scanner.scan_with_pattern(data, r"(\w+):\s*([^\n]+)", PatternType::Regex).unwrap();
        assert!(matches.len() >= 2); // Should find multiple headers
        
        // Test glob for file extensions
        let file_data = b"config.json";
        let matches = scanner.scan_with_pattern(file_data, "*.json", PatternType::Glob).unwrap();
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_simd_guided_pattern_scan() {
        let scanner = PatternScanner::new();
        let data = b"GET /test HTTP/1.1\nPOST /api HTTP/1.1\nPUT /data HTTP/1.1";
        
        // Should use SIMD to accelerate finding HTTP methods
        let matches = scanner.simd_guided_pattern_scan(data, r"(GET|POST|PUT)", PatternType::Regex).unwrap();
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_pattern_capabilities() {
        let matcher = SimdPatternMatcher::new();
        let caps = matcher.pattern_capabilities();
        
        assert!(caps.supports_glob);
        assert!(caps.supports_regex);
        assert!(caps.supports_unicode);
        assert!(caps.max_pattern_length > 0);
        assert!(caps.max_data_size > 0);
    }
}