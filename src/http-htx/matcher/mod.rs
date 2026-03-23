//! Speculative Matcher - Fanout/Fanin for HTTP protocol detection
//!
//! Multiple parsers run in parallel. Longest match wins.
//!
//! This module CANNOT see listener, reactor, timer, handler.

use std::time::{Duration, Instant};

pub type Confidence = u8;

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub bytes_matched: usize,
    pub confidence: Confidence,
    pub complete: bool,
    pub protocol: &'static str,
    pub version: Option<(u8, u8)>,
    pub elapsed: Duration,
}

impl MatchResult {
    pub fn score(&self) -> u64 {
        let complete_bonus = if self.complete { 1_000_000_000u64 } else { 0 };
        let bytes_score = self.bytes_matched as u64 * 1000;
        let confidence_score = self.confidence as u64;
        complete_bonus + bytes_score + confidence_score
    }
}

pub struct SpeculativeMatcher {
    timeout: Duration,
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

    pub fn match_speculative<F>(&self, parsers: Vec<F>, input: &[u8]) -> Option<MatchResult>
    where
        F: FnOnce(&[u8]) -> MatchResult,
    {
        let start = Instant::now();
        let mut results: Vec<MatchResult> = Vec::with_capacity(parsers.len());

        for parser in parsers {
            let result = parser(input);
            results.push(result);

            if let Some(r) = results.last() {
                if r.complete && r.confidence >= 95 {
                    return Some(r.clone());
                }
            }

            if start.elapsed() > self.timeout {
                break;
            }
        }

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

pub fn http1_parser(input: &[u8]) -> MatchResult {
    let start = Instant::now();

    if let Ok(text) = std::str::from_utf8(&input[..input.len().min(1024)]) {
        if text.starts_with("HTTP/") {
            let parts: Vec<&str> = text.split_whitespace().collect();
            if parts.len() >= 2 {
                let complete = text.contains("\r\n\r\n") || input.len() >= 64;
                return MatchResult {
                    bytes_matched: input.len().min(1024),
                    confidence: if complete { 100 } else { 60 },
                    complete,
                    protocol: "HTTP/1.x",
                    version: Some((1, 1)),
                    elapsed: start.elapsed(),
                };
            }
        }

        let methods = [
            "GET ", "POST ", "PUT ", "DELETE ", "HEAD ", "OPTIONS ", "PATCH ", "CONNECT ", "TRACE ",
        ];
        for method in methods {
            if text.starts_with(method) {
                let complete = text.contains("\r\n\r\n") || input.len() >= 64;
                return MatchResult {
                    bytes_matched: text.find(' ').unwrap_or(4),
                    confidence: if complete { 100 } else { 70 },
                    complete,
                    protocol: "HTTP/1.x",
                    version: Some((1, 1)),
                    elapsed: start.elapsed(),
                };
            }
        }
    }

    MatchResult {
        bytes_matched: 0,
        confidence: 0,
        complete: false,
        protocol: "HTTP/1.x",
        version: None,
        elapsed: start.elapsed(),
    }
}

pub fn http2_parser(input: &[u8]) -> MatchResult {
    let start = Instant::now();

    if input.len() >= 24 && &input[0..24] == b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n" {
        return MatchResult {
            bytes_matched: 24,
            confidence: 100,
            complete: true,
            protocol: "HTTP/2",
            version: Some((2, 0)),
            elapsed: start.elapsed(),
        };
    }

    if input.len() >= 9 {
        let frame_type = input[3];
        if frame_type == 0x4 || frame_type == 0x1 {
            return MatchResult {
                bytes_matched: 9,
                confidence: 90,
                complete: false,
                protocol: "HTTP/2",
                version: Some((2, 0)),
                elapsed: start.elapsed(),
            };
        }
    }

    MatchResult {
        bytes_matched: 0,
        confidence: 0,
        complete: false,
        protocol: "HTTP/2",
        version: None,
        elapsed: start.elapsed(),
    }
}

pub fn http3_parser(input: &[u8]) -> MatchResult {
    let start = Instant::now();

    if input.len() >= 3 {
        if input[0] == 0x0 || input[0] == 0x1 {
            return MatchResult {
                bytes_matched: 3,
                confidence: 50,
                complete: false,
                protocol: "HTTP/3",
                version: Some((3, 0)),
                elapsed: start.elapsed(),
            };
        }
    }

    MatchResult {
        bytes_matched: 0,
        confidence: 0,
        complete: false,
        protocol: "HTTP/3",
        version: None,
        elapsed: start.elapsed(),
    }
}

pub mod fiber {
    use super::{Confidence, MatchResult};

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
            self.update(bytes, confidence);
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
                version: None,
                elapsed: std::time::Duration::default(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http1_parser_get() {
        let input = b"GET / HTTP/1.1\r\n\r\n";
        let result = http1_parser(input);
        assert!(result.confidence > 0);
        assert_eq!(result.protocol, "HTTP/1.x");
    }

    #[test]
    fn test_http2_parser_preface() {
        let preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
        let result = http2_parser(preface);
        assert_eq!(result.confidence, 100);
        assert!(result.complete);
    }

    #[test]
    fn test_http1_parser_rejects_non_http() {
        let input = b"\x00\x01\x02\x03";
        let result = http1_parser(input);
        assert_eq!(result.confidence, 0);
    }
}
