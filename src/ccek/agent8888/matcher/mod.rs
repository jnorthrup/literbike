//! Speculative Matcher - Fanout/Fanin for protocol detection
//!
//! Multiple parsers run in parallel. Longest match wins.
//! Winner determined by: completion OR confidence in partial completion.
//!
//! This module CANNOT see listener, reactor, timer, handler.
//! It only knows about itself and the core traits.

use std::time::{Duration, Instant};

/// Match confidence score (0-100)
pub type Confidence = u8;

/// Match result from a speculative parser
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// How many bytes were consumed
    pub bytes_matched: usize,
    /// Confidence this is the correct protocol (0-100)
    pub confidence: Confidence,
    /// Whether parsing completed successfully
    pub complete: bool,
    /// Protocol identifier
    pub protocol: &'static str,
    /// Time spent parsing
    pub elapsed: Duration,
}

impl MatchResult {
    /// Score for ranking matches (higher = better)
    /// Complete matches score highest, then by bytes matched, then confidence
    pub fn score(&self) -> u64 {
        let complete_bonus = if self.complete { 1_000_000_000u64 } else { 0 };
        let bytes_score = self.bytes_matched as u64 * 1000;
        let confidence_score = self.confidence as u64;
        complete_bonus + bytes_score + confidence_score
    }
}

/// Speculative matcher - runs multiple parsers in parallel
pub struct SpeculativeMatcher {
    /// Maximum time to wait for results
    timeout: Duration,
    /// Minimum confidence to accept a partial match
    min_confidence: Confidence,
}

impl SpeculativeMatcher {
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_millis(10),
            min_confidence: 80,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Run parsers speculatively, return best match
    pub fn match_speculative<F>(&self, parsers: Vec<F>, input: &[u8]) -> Option<MatchResult>
    where
        F: FnOnce(&[u8]) -> MatchResult,
    {
        let start = Instant::now();
        let mut results: Vec<MatchResult> = Vec::with_capacity(parsers.len());

        // Fanout: Run all parsers (in parallel with fibers/threads)
        for parser in parsers {
            let result = parser(input);
            results.push(result);

            // Early exit if we found a complete match with high confidence
            if let Some(r) = results.last() {
                if r.complete && r.confidence >= 95 {
                    return Some(r.clone());
                }
            }

            // Check timeout
            if start.elapsed() > self.timeout {
                break;
            }
        }

        // Fanin: Pick winner by score (longest match)
        results
            .into_iter()
            .filter(|r| r.complete || r.confidence >= self.min_confidence)
            .max_by_key(|r| r.score())
    }
}

impl Default for SpeculativeMatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Fiber/coroutine based speculative execution
pub mod fiber {
    use super::{Confidence, MatchResult};

    /// Fiber handle for speculative parsing
    pub struct ParseFiber {
        pub protocol: &'static str,
        pub bytes_matched: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        pub confidence: std::sync::Arc<std::sync::atomic::AtomicU8>,
        pub complete: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    impl ParseFiber {
        pub fn new(protocol: &'static str) -> Self {
            Self {
                protocol,
                bytes_matched: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                confidence: std::sync::Arc::new(std::sync::atomic::AtomicU8::new(0)),
                complete: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            }
        }

        pub fn update(&self, bytes: usize, confidence: Confidence) {
            self.bytes_matched
                .store(bytes, std::sync::atomic::Ordering::Relaxed);
            self.confidence
                .store(confidence, std::sync::atomic::Ordering::Relaxed);
        }

        pub fn finish(&self, bytes: usize, confidence: Confidence) {
            self.bytes_matched
                .store(bytes, std::sync::atomic::Ordering::Relaxed);
            self.confidence
                .store(confidence, std::sync::atomic::Ordering::Relaxed);
            self.complete
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }

        pub fn to_result(&self) -> MatchResult {
            MatchResult {
                bytes_matched: self
                    .bytes_matched
                    .load(std::sync::atomic::Ordering::Relaxed),
                confidence: self.confidence.load(std::sync::atomic::Ordering::Relaxed),
                complete: self.complete.load(std::sync::atomic::Ordering::Relaxed),
                protocol: self.protocol,
                elapsed: std::time::Duration::default(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http_parser(input: &[u8]) -> MatchResult {
        if input.starts_with(b"GET ") || input.starts_with(b"POST ") {
            let bytes = input.len().min(16);
            let confidence = if input.len() >= 16 { 100 } else { 60 };
            MatchResult {
                bytes_matched: bytes,
                confidence,
                complete: input.len() >= 16,
                protocol: "HTTP",
                elapsed: Duration::default(),
            }
        } else {
            MatchResult {
                bytes_matched: 0,
                confidence: 0,
                complete: false,
                protocol: "HTTP",
                elapsed: Duration::default(),
            }
        }
    }

    fn socks5_parser(input: &[u8]) -> MatchResult {
        if input.first() == Some(&0x05) {
            MatchResult {
                bytes_matched: input.len(),
                confidence: 100,
                complete: input.len() >= 3,
                protocol: "SOCKS5",
                elapsed: Duration::default(),
            }
        } else {
            MatchResult {
                bytes_matched: 0,
                confidence: 0,
                complete: false,
                protocol: "SOCKS5",
                elapsed: Duration::default(),
            }
        }
    }

    #[test]
    fn test_http_wins_over_socks5() {
        let http_result = http_parser(b"GET / HTTP/1.1\r\n");
        let socks_result = socks5_parser(b"GET / HTTP/1.1\r\n");

        assert!(http_result.score() > socks_result.score());
    }

    #[test]
    fn test_socks5_wins_for_socks5_input() {
        let http_result = http_parser(&[0x05, 0x01, 0x00]);
        let socks_result = socks5_parser(&[0x05, 0x01, 0x00]);

        assert!(socks_result.score() > http_result.score());
        assert!(socks_result.complete);
    }

    #[test]
    fn test_fiber_update() {
        let fiber = fiber::ParseFiber::new("TEST");
        fiber.update(10, 50);
        assert_eq!(fiber.to_result().bytes_matched, 10);
        assert_eq!(fiber.to_result().confidence, 50);

        fiber.finish(20, 100);
        assert_eq!(fiber.to_result().bytes_matched, 20);
        assert!(fiber.to_result().complete);
    }
}
