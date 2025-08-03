// ARM64/NEON SIMD-optimized Protocol Detector for Snapdragon
// Optimized for Termux/Android ARM64 processors

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

use super::patricia_detector_simd::Protocol;

/// ARM64 NEON-optimized SIMD detector
#[cfg(target_arch = "aarch64")]
pub struct Arm64SimdDetector {
    socks5_mask: u8,
    http_patterns: Vec<[u8; 16]>,
}

#[cfg(target_arch = "aarch64")]
impl Arm64SimdDetector {
    pub fn new() -> Self {
        Self {
            socks5_mask: 0x05,
            http_patterns: vec![
                // Pad patterns to 16 bytes for NEON
                *b"GET             ",
                *b"POST            ",
                *b"PUT             ",
                *b"DELETE          ",
                *b"HEAD            ",
                *b"OPTIONS         ",
                *b"CONNECT         ",
                *b"PATCH           ",
            ],
        }
    }

    #[inline(always)]
    pub fn detect_simd(&self, buffer: &[u8]) -> Protocol {
        if buffer.is_empty() {
            return Protocol::Unknown;
        }

        // Fast path for SOCKS5
        if buffer[0] == self.socks5_mask {
            return Protocol::Socks5;
        }

        // Need at least 16 bytes for NEON
        if buffer.len() < 16 {
            return self.detect_scalar(buffer);
        }

        unsafe {
            // Load first 16 bytes into NEON register
            let data = vld1q_u8(buffer.as_ptr());
            
            // Check TLS (0x16 0x03 0x??)
            if buffer[0] == 0x16 && buffer[1] == 0x03 {
                return Protocol::Tls;
            }

            // Check HTTP methods using NEON
            for pattern in &self.http_patterns {
                let pattern_vec = vld1q_u8(pattern.as_ptr());
                let cmp = vceqq_u8(data, pattern_vec);
                
                // Extract comparison results
                let cmp_result = vget_lane_u64(vreinterpret_u64_u8(vget_low_u8(cmp)), 0);
                
                // Check if pattern matches (considering actual method length)
                let method_name = std::str::from_utf8(&pattern[..8]).unwrap_or("").trim();
                let method_len = method_name.len();
                
                if method_len > 0 {
                    let mask = (1u64 << (method_len * 8)) - 1;
                    if cmp_result & mask == mask {
                        return Protocol::Http;
                    }
                }
            }

            // Check PROXY protocol using NEON
            if buffer.len() >= 6 {
                let proxy_pattern = *b"PROXY           ";
                let proxy_vec = vld1q_u8(proxy_pattern.as_ptr());
                let cmp = vceqq_u8(data, proxy_vec);
                let cmp_result = vget_lane_u64(vreinterpret_u64_u8(vget_low_u8(cmp)), 0);
                
                // Check first 5 bytes ("PROXY")
                if cmp_result & 0xFF_FF_FF_FF_FF == 0xFF_FF_FF_FF_FF {
                    return Protocol::ProxyProtocol;
                }
            }

            // Check HTTP/2 preface
            if buffer.len() >= 14 {
                let h2_pattern = *b"PRI * HTTP/2.0  ";
                let h2_vec = vld1q_u8(h2_pattern.as_ptr());
                let cmp = vceqq_u8(data, h2_vec);
                
                // Use two 64-bit comparisons for the 14-byte pattern
                let low_cmp = vget_lane_u64(vreinterpret_u64_u8(vget_low_u8(cmp)), 0);
                let high_cmp = vget_lane_u64(vreinterpret_u64_u8(vget_high_u8(cmp)), 0);
                
                if low_cmp == 0xFF_FF_FF_FF_FF_FF_FF_FF && (high_cmp & 0xFF_FF_FF_FF_FF_FF) == 0xFF_FF_FF_FF_FF_FF {
                    return Protocol::Http2;
                }
            }

            // Check WebSocket upgrade
            if buffer.len() >= 3 && buffer[0] == b'G' && buffer[1] == b'E' && buffer[2] == b'T' {
                // Quick check for WebSocket upgrade in HTTP headers
                if Self::contains_websocket_upgrade(buffer) {
                    return Protocol::WebSocket;
                }
            }
        }

        Protocol::Unknown
    }

    #[inline]
    fn detect_scalar(&self, buffer: &[u8]) -> Protocol {
        if buffer.is_empty() {
            return Protocol::Unknown;
        }

        // SOCKS5
        if buffer[0] == 0x05 {
            return Protocol::Socks5;
        }

        // TLS
        if buffer.len() >= 3 && buffer[0] == 0x16 && buffer[1] == 0x03 {
            return Protocol::Tls;
        }

        // HTTP methods (scalar)
        let methods = ["GET ", "POST ", "PUT ", "DELETE ", "HEAD ", "OPTIONS ", "CONNECT ", "PATCH "];
        for method in &methods {
            if buffer.len() >= method.len() && buffer.starts_with(method.as_bytes()) {
                return Protocol::Http;
            }
        }

        // PROXY protocol
        if buffer.len() >= 6 && buffer.starts_with(b"PROXY ") {
            return Protocol::ProxyProtocol;
        }

        // HTTP/2
        if buffer.len() >= 14 && buffer.starts_with(b"PRI * HTTP/2.0") {
            return Protocol::Http2;
        }

        Protocol::Unknown
    }

    #[inline]
    fn contains_websocket_upgrade(buffer: &[u8]) -> bool {
        // Simple check for WebSocket upgrade headers
        if let Ok(s) = std::str::from_utf8(buffer) {
            s.contains("Upgrade: websocket") || s.contains("upgrade: websocket")
        } else {
            false
        }
    }
}

// Optimized protocol detection using ARM64 NEON instructions
#[cfg(target_arch = "aarch64")]
pub fn detect_protocol_arm64(buffer: &[u8]) -> Protocol {
    // For very small buffers, skip SIMD
    if buffer.len() < 4 {
        return match buffer.first() {
            Some(&0x05) => Protocol::Socks5,
            Some(&0x16) if buffer.len() >= 3 && buffer[1] == 0x03 => Protocol::Tls,
            _ => Protocol::Unknown,
        };
    }

    let detector = Arm64SimdDetector::new();
    detector.detect_simd(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_arm64_detection() {
        let detector = Arm64SimdDetector::new();
        
        // Test HTTP
        assert!(matches!(detector.detect_simd(b"GET / HTTP/1.1\r\n"), Protocol::Http));
        assert!(matches!(detector.detect_simd(b"POST /api HTTP/1.1\r\n"), Protocol::Http));
        
        // Test SOCKS5
        assert!(matches!(detector.detect_simd(&[0x05, 0x01, 0x00]), Protocol::Socks5));
        
        // Test TLS
        assert!(matches!(detector.detect_simd(&[0x16, 0x03, 0x03, 0x00, 0x10]), Protocol::Tls));
        
        // Test PROXY protocol
        assert!(matches!(detector.detect_simd(b"PROXY TCP4 192.168.1.1"), Protocol::ProxyProtocol));
        
        // Test HTTP/2
        assert!(matches!(detector.detect_simd(b"PRI * HTTP/2.0\r\n\r\n"), Protocol::Http2));
    }

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_performance_comparison() {
        let detector = Arm64SimdDetector::new();
        let test_data = b"GET /index.html HTTP/1.1\r\nHost: example.com\r\n\r\n";
        
        // Warm up
        for _ in 0..1000 {
            let _ = detector.detect_simd(test_data);
            let _ = detector.detect_scalar(test_data);
        }
        
        // Benchmark SIMD
        let start = std::time::Instant::now();
        for _ in 0..1_000_000 {
            let _ = detector.detect_simd(test_data);
        }
        let simd_time = start.elapsed();
        
        // Benchmark scalar
        let start = std::time::Instant::now();
        for _ in 0..1_000_000 {
            let _ = detector.detect_scalar(test_data);
        }
        let scalar_time = start.elapsed();
        
        println!("ARM64 NEON time: {:?}", simd_time);
        println!("Scalar time: {:?}", scalar_time);
        println!("Speedup: {:.2}x", scalar_time.as_nanos() as f64 / simd_time.as_nanos() as f64);
    }
}