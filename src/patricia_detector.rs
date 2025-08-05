// High-Performance Compile-Time Combinatorial Protocol Detector
// Zero-runtime allocation protocol detection using invariant byte analysis
// Optimized for Android/Termux syscall-only environments

#[derive(Debug, Clone, PartialEq)]
pub enum Protocol {
    Http,
    Socks5,
    Tls,
    WebSocket,
    ProxyProtocol,  // HAProxy PROXY protocol
    Http2,          // HTTP/2 preface
    Unknown,
}

// Compile-time protocol pattern constants
const SOCKS5_PATTERN: &[u8] = &[0x05];

const TLS_SSL30_PATTERN: &[u8] = &[0x16, 0x03, 0x00];
const TLS_10_PATTERN: &[u8] = &[0x16, 0x03, 0x01];
const TLS_11_PATTERN: &[u8] = &[0x16, 0x03, 0x02];
const TLS_12_PATTERN: &[u8] = &[0x16, 0x03, 0x03];
const TLS_13_PATTERN: &[u8] = &[0x16, 0x03, 0x04];

const HTTP_GET_PATTERN: &[u8] = b"GET ";
const HTTP_POST_PATTERN: &[u8] = b"POST ";
const HTTP_PUT_PATTERN: &[u8] = b"PUT ";
const HTTP_DELETE_PATTERN: &[u8] = b"DELETE ";
const HTTP_HEAD_PATTERN: &[u8] = b"HEAD ";
const HTTP_OPTIONS_PATTERN: &[u8] = b"OPTIONS ";
const HTTP_CONNECT_PATTERN: &[u8] = b"CONNECT ";
const HTTP_PATCH_PATTERN: &[u8] = b"PATCH ";
const HTTP_TRACE_PATTERN: &[u8] = b"TRACE ";

const PROXY_V1_PATTERN: &[u8] = b"PROXY ";
const PROXY_V2_PATTERN: &[u8] = &[0x0D, 0x0A, 0x0D, 0x0A, 0x00, 0x0D, 0x0A, 0x51, 0x55, 0x49, 0x54, 0x0A];

const HTTP2_PREFACE_PATTERN: &[u8] = b"PRI * HTTP/2.0\r\n";

// Efficient const pattern matching helper
#[inline]
const fn starts_with_pattern(data: &[u8], pattern: &[u8]) -> bool {
    if data.len() < pattern.len() {
        return false;
    }
    
    let mut i = 0;
    while i < pattern.len() {
        if data[i] != pattern[i] {
            return false;
        }
        i += 1;
    }
    true
}

// Zero-allocation combinatorial protocol detector
// Uses rarity-ordered byte analysis for maximum performance
#[inline]
pub const fn detect_protocol_combinatorial(data: &[u8]) -> (Protocol, usize) {
    if data.is_empty() {
        return (Protocol::Unknown, 0);
    }

    // First-level dispatch by first byte (ordered by rarity for fastest rejection)
    match data[0] {
        // SOCKS5 - Extremely rare first byte (0x05)
        0x05 => {
            if starts_with_pattern(data, SOCKS5_PATTERN) {
                (Protocol::Socks5, SOCKS5_PATTERN.len())
            } else {
                (Protocol::Unknown, 0)
            }
        }

        // TLS - Very rare first byte (0x16)
        0x16 => {
            if data.len() >= 3 && data[1] == 0x03 {
                match data[2] {
                    0x00 if starts_with_pattern(data, TLS_SSL30_PATTERN) => 
                        (Protocol::Tls, TLS_SSL30_PATTERN.len()),
                    0x01 if starts_with_pattern(data, TLS_10_PATTERN) => 
                        (Protocol::Tls, TLS_10_PATTERN.len()),
                    0x02 if starts_with_pattern(data, TLS_11_PATTERN) => 
                        (Protocol::Tls, TLS_11_PATTERN.len()),
                    0x03 if starts_with_pattern(data, TLS_12_PATTERN) => 
                        (Protocol::Tls, TLS_12_PATTERN.len()),
                    0x04 if starts_with_pattern(data, TLS_13_PATTERN) => 
                        (Protocol::Tls, TLS_13_PATTERN.len()),
                    _ => (Protocol::Unknown, 0),
                }
            } else {
                (Protocol::Unknown, 0)
            }
        }

        // PROXY v2 - Moderately rare first byte sequence (0x0D)
        0x0D => {
            if starts_with_pattern(data, PROXY_V2_PATTERN) {
                (Protocol::ProxyProtocol, PROXY_V2_PATTERN.len())
            } else {
                (Protocol::Unknown, 0)
            }
        }

        // HTTP methods and other protocols starting with common ASCII letters
        // 'P' - POST, PUT, PATCH, PROXY v1, HTTP/2 preface
        b'P' => {
            if starts_with_pattern(data, HTTP2_PREFACE_PATTERN) {
                (Protocol::Http2, HTTP2_PREFACE_PATTERN.len())
            } else if starts_with_pattern(data, PROXY_V1_PATTERN) {
                (Protocol::ProxyProtocol, PROXY_V1_PATTERN.len())
            } else if starts_with_pattern(data, HTTP_POST_PATTERN) {
                (Protocol::Http, HTTP_POST_PATTERN.len())
            } else if starts_with_pattern(data, HTTP_PUT_PATTERN) {
                (Protocol::Http, HTTP_PUT_PATTERN.len())
            } else if starts_with_pattern(data, HTTP_PATCH_PATTERN) {
                (Protocol::Http, HTTP_PATCH_PATTERN.len())
            } else {
                (Protocol::Unknown, 0)
            }
        }

        // 'G' - GET
        b'G' => {
            if starts_with_pattern(data, HTTP_GET_PATTERN) {
                (Protocol::Http, HTTP_GET_PATTERN.len())
            } else {
                (Protocol::Unknown, 0)
            }
        }

        // 'D' - DELETE
        b'D' => {
            if starts_with_pattern(data, HTTP_DELETE_PATTERN) {
                (Protocol::Http, HTTP_DELETE_PATTERN.len())
            } else {
                (Protocol::Unknown, 0)
            }
        }

        // 'H' - HEAD
        b'H' => {
            if starts_with_pattern(data, HTTP_HEAD_PATTERN) {
                (Protocol::Http, HTTP_HEAD_PATTERN.len())
            } else {
                (Protocol::Unknown, 0)
            }
        }

        // 'O' - OPTIONS
        b'O' => {
            if starts_with_pattern(data, HTTP_OPTIONS_PATTERN) {
                (Protocol::Http, HTTP_OPTIONS_PATTERN.len())
            } else {
                (Protocol::Unknown, 0)
            }
        }

        // 'C' - CONNECT
        b'C' => {
            if starts_with_pattern(data, HTTP_CONNECT_PATTERN) {
                (Protocol::Http, HTTP_CONNECT_PATTERN.len())
            } else {
                (Protocol::Unknown, 0)
            }
        }

        // 'T' - TRACE
        b'T' => {
            if starts_with_pattern(data, HTTP_TRACE_PATTERN) {
                (Protocol::Http, HTTP_TRACE_PATTERN.len())
            } else {
                (Protocol::Unknown, 0)
            }
        }

        // All other bytes - Unknown protocol
        _ => (Protocol::Unknown, 0),
    }
}

// Compatibility wrapper to maintain existing API
pub struct PatriciaDetector;

impl PatriciaDetector {
    #[inline]
    pub const fn new() -> Self {
        PatriciaDetector
    }

    #[inline]
    pub fn detect(&self, buffer: &[u8]) -> Protocol {
        detect_protocol_combinatorial(buffer).0
    }

    #[inline]
    pub fn detect_with_length(&self, buffer: &[u8]) -> (Protocol, usize) {
        detect_protocol_combinatorial(buffer)
    }
}

// Legacy compatibility function - now uses the combinatorial detector
#[inline]
pub fn quick_detect(buffer: &[u8]) -> Option<Protocol> {
    let (protocol, _) = detect_protocol_combinatorial(buffer);
    match protocol {
        Protocol::Unknown => None,
        proto => Some(proto),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_detection() {
        let detector = PatriciaDetector::new();
        
        assert!(matches!(detector.detect(b"GET / HTTP/1.1\r\n"), Protocol::Http));
        assert!(matches!(detector.detect(b"POST /api"), Protocol::Http));
        assert!(matches!(detector.detect(b"CONNECT example.com:443"), Protocol::Http));
    }

    #[test]
    fn test_socks5_detection() {
        let detector = PatriciaDetector::new();
        
        assert!(matches!(detector.detect(&[0x05, 0x01, 0x00]), Protocol::Socks5));
    }

    #[test]
    fn test_tls_detection() {
        let detector = PatriciaDetector::new();
        
        assert!(matches!(detector.detect(&[0x16, 0x03, 0x01]), Protocol::Tls));
        assert!(matches!(detector.detect(&[0x16, 0x03, 0x03]), Protocol::Tls));
    }

    #[test]
    fn test_quick_detect() {
        assert!(matches!(quick_detect(b"GET /"), Some(Protocol::Http)));
        assert!(matches!(quick_detect(&[0x05, 0x01]), Some(Protocol::Socks5)));
        assert!(matches!(quick_detect(&[0x16, 0x03, 0x01]), Some(Protocol::Tls)));
    }
}