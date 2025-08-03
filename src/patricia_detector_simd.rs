// SIMD-optimized Patricia Trie Protocol Detector
// Uses register-width operations for faster protocol detection

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;
#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

#[derive(Debug, Clone)]
pub enum Protocol {
    Http,
    Socks5,
    Tls,
    WebSocket,
    ProxyProtocol,
    Http2,
    Unknown,
}

#[cfg(target_arch = "x86_64")]
pub struct SimdDetector {
    // Pre-computed masks for common protocols
    http_masks: Vec<__m128i>,
    tls_mask: __m128i,
    socks5_mask: u8,
}

#[cfg(target_arch = "aarch64")]
pub struct SimdDetector {
    socks5_mask: u8,
    // Pre-computed NEON vectors for common protocols
    http_vectors: Vec<uint8x16_t>,
    tls_pattern: [u8; 3],
}

#[cfg(target_arch = "x86_64")]
impl SimdDetector {
    pub fn new() -> Self {
        unsafe {
            SimdDetector {
                // HTTP methods as 128-bit masks
                http_masks: vec![
                    _mm_set_epi8(0,0,0,0,0,0,0,0,0,0,0,0,b' ' as i8,b'T' as i8,b'E' as i8,b'G' as i8), // "GET "
                    _mm_set_epi8(0,0,0,0,0,0,0,0,0,0,0,b' ' as i8,b'T' as i8,b'S' as i8,b'O' as i8,b'P' as i8), // "POST "
                    _mm_set_epi8(0,0,0,0,0,0,0,0,0,0,0,0,b' ' as i8,b'T' as i8,b'U' as i8,b'P' as i8), // "PUT "
                    _mm_set_epi8(0,0,0,0,0,0,0,0,0,b' ' as i8,b'E' as i8,b'T' as i8,b'E' as i8,b'L' as i8,b'E' as i8,b'D' as i8), // "DELETE "
                ],
                // TLS handshake start
                tls_mask: _mm_set_epi8(0,0,0,0,0,0,0,0,0,0,0,0,0,0x03,0x00,0x16),
                socks5_mask: 0x05,
            }
        }
    }
}

#[cfg(target_arch = "aarch64")]
impl SimdDetector {
    pub fn new() -> Self {
        unsafe {
            SimdDetector {
                socks5_mask: 0x05,
                http_vectors: vec![
                    vld1q_u8(b"GET             ".as_ptr()),
                    vld1q_u8(b"POST            ".as_ptr()),
                    vld1q_u8(b"PUT             ".as_ptr()),
                    vld1q_u8(b"DELETE          ".as_ptr()),
                    vld1q_u8(b"HEAD            ".as_ptr()),
                    vld1q_u8(b"CONNECT         ".as_ptr()),
                ],
                tls_pattern: [0x16, 0x03, 0x00],
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    pub fn detect_simd(&self, buffer: &[u8]) -> Protocol {
        if buffer.is_empty() {
            return Protocol::Unknown;
        }

        // Fast path for single-byte protocols
        if buffer[0] == self.socks5_mask {
            return Protocol::Socks5;
        }

        // Need at least 16 bytes for SIMD
        if buffer.len() < 16 {
            return self.detect_scalar(buffer);
        }

        unsafe {
            // Load first 16 bytes into SIMD register
            let data = _mm_loadu_si128(buffer.as_ptr() as *const __m128i);
            
            // Check TLS (0x16 0x03 0x??)
            let tls_cmp = _mm_cmpeq_epi8(_mm_and_si128(data, _mm_set_epi8(
                0,0,0,0,0,0,0,0,0,0,0,0,0,0xFF,0xFF,0xFF
            )), self.tls_mask);
            if _mm_movemask_epi8(tls_cmp) & 0x0007 == 0x0007 {
                return Protocol::Tls;
            }

            // Check HTTP methods using SIMD
            for http_mask in &self.http_masks {
                let mask_len = match http_mask {
                    _ if _mm_extract_epi8(*http_mask, 12) != 0 => 4,  // 4-byte methods
                    _ if _mm_extract_epi8(*http_mask, 11) != 0 => 5,  // 5-byte methods
                    _ => 7,  // DELETE is 7 bytes
                };
                
                let cmp = _mm_cmpeq_epi8(data, *http_mask);
                let cmp_mask = _mm_movemask_epi8(cmp);
                
                // Check if rightmost mask_len bits are set
                if cmp_mask & ((1 << mask_len) - 1) == ((1 << mask_len) - 1) {
                    return Protocol::Http;
                }
            }

            // Check PROXY protocol signature
            if buffer.len() >= 6 {
                let proxy_sig = b"PROXY ";
                let cmp = _mm_cmpeq_epi8(data, _mm_set_epi8(
                    0,0,0,0,0,0,0,0,0,0,
                    b' ' as i8,b'Y' as i8,b'X' as i8,b'O' as i8,b'R' as i8,b'P' as i8
                ));
                if _mm_movemask_epi8(cmp) & 0x003F == 0x003F {
                    return Protocol::ProxyProtocol;
                }
            }

            // Check HTTP/2 preface
            if buffer.len() >= 14 {
                let h2_preface = b"PRI * HTTP/2.0";
                let cmp = _mm_cmpeq_epi8(data, _mm_set_epi8(
                    0,0,
                    b'0' as i8,b'.' as i8,b'2' as i8,b'/' as i8,b'P' as i8,b'T' as i8,b'T' as i8,b'H' as i8,
                    b' ' as i8,b'*' as i8,b' ' as i8,b'I' as i8,b'R' as i8,b'P' as i8
                ));
                if _mm_movemask_epi8(cmp) & 0x3FFF == 0x3FFF {
                    return Protocol::Http2;
                }
            }
        }

        Protocol::Unknown
    }
    
    #[cfg(target_arch = "aarch64")]
    #[inline(always)]
    pub fn detect_simd(&self, buffer: &[u8]) -> Protocol {
        if buffer.is_empty() {
            return Protocol::Unknown;
        }

        // Fast path for single-byte protocols (optimized for Samsung S20)
        if buffer[0] == self.socks5_mask {
            return Protocol::Socks5;
        }

        // TLS detection (common on mobile)
        if buffer.len() >= 3 && buffer[0] == self.tls_pattern[0] && buffer[1] == self.tls_pattern[1] {
            return Protocol::Tls;
        }

        // Need at least 16 bytes for NEON
        if buffer.len() < 16 {
            return self.detect_scalar(buffer);
        }

        unsafe {
            // Load first 16 bytes into NEON register
            let data = vld1q_u8(buffer.as_ptr());
            
            // Check HTTP methods using NEON (optimized for Snapdragon 865)
            for (i, &pattern_vec) in self.http_vectors.iter().enumerate() {
                // Use NEON comparison
                let cmp = vceqq_u8(data, pattern_vec);
                
                // Extract comparison results efficiently
                let cmp_low = vget_low_u8(cmp);
                let cmp_result = vreinterpret_u64_u8(cmp_low);
                let result = vget_lane_u64(cmp_result, 0);
                
                // Method lengths: GET=3, POST=4, PUT=3, DELETE=6, HEAD=4, CONNECT=7
                let method_len = match i {
                    0 | 2 => 3,  // GET, PUT
                    1 | 4 => 4,  // POST, HEAD
                    3 => 6,      // DELETE
                    5 => 7,      // CONNECT
                    _ => continue,
                };
                
                let mask = (1u64 << (method_len * 8)) - 1;
                if result & mask == mask {
                    return Protocol::Http;
                }
            }

            // Check PROXY protocol
            if buffer.starts_with(b"PROXY ") {
                return Protocol::ProxyProtocol;
            }

            // Check HTTP/2
            if buffer.len() >= 14 && buffer.starts_with(b"PRI * HTTP/2.0") {
                return Protocol::Http2;
            }
        }

        Protocol::Unknown
    }

    // Scalar fallback for short buffers
    #[inline]
    fn detect_scalar(&self, buffer: &[u8]) -> Protocol {
        if buffer.is_empty() {
            return Protocol::Unknown;
        }

        match buffer[0] {
            0x05 => Protocol::Socks5,
            0x16 if buffer.len() >= 3 && buffer[1] == 0x03 => Protocol::Tls,
            b'G' if buffer.len() >= 4 && &buffer[0..4] == b"GET " => Protocol::Http,
            b'P' if buffer.len() >= 5 => {
                match &buffer[0..5] {
                    b"POST " => Protocol::Http,
                    b"PUT " => Protocol::Http,
                    b"PATCH" if buffer.len() >= 6 && buffer[5] == b' ' => Protocol::Http,
                    b"PROXY" if buffer.len() >= 6 && buffer[5] == b' ' => Protocol::ProxyProtocol,
                    b"PRI *" if buffer.len() >= 14 && &buffer[5..14] == b" HTTP/2.0" => Protocol::Http2,
                    _ => Protocol::Unknown,
                }
            },
            b'H' if buffer.len() >= 5 && &buffer[0..5] == b"HEAD " => Protocol::Http,
            b'D' if buffer.len() >= 7 && &buffer[0..7] == b"DELETE " => Protocol::Http,
            b'O' if buffer.len() >= 8 && &buffer[0..8] == b"OPTIONS " => Protocol::Http,
            b'C' if buffer.len() >= 8 && &buffer[0..8] == b"CONNECT " => Protocol::Http,
            b'T' if buffer.len() >= 6 && &buffer[0..6] == b"TRACE " => Protocol::Http,
            _ => Protocol::Unknown,
        }
    }
}

// ARM64 NEON implementation for Termux
#[cfg(target_arch = "aarch64")]
pub mod neon {
    use super::Protocol;
    use std::arch::aarch64::*;

    pub struct NeonDetector {
        socks5_mask: u8,
    }

    impl NeonDetector {
        pub fn new() -> Self {
            NeonDetector {
                socks5_mask: 0x05,
            }
        }

        #[inline(always)]
        pub fn detect_neon(&self, buffer: &[u8]) -> Protocol {
            if buffer.is_empty() {
                return Protocol::Unknown;
            }

            // Fast path for single-byte protocols
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

                // Check HTTP "GET " using NEON
                let get_pattern = vld1q_u8(b"GET             ".as_ptr());
                let cmp = vceqq_u8(data, get_pattern);
                let cmp_reduced = vget_lane_u32(vreinterpret_u32_u8(vget_low_u8(cmp)), 0);
                if cmp_reduced == 0xFFFFFFFF {
                    return Protocol::Http;
                }

                // More patterns can be added here
            }

            self.detect_scalar(buffer)
        }

        #[inline]
        fn detect_scalar(&self, buffer: &[u8]) -> Protocol {
            // Same scalar implementation as x86
            if buffer.is_empty() {
                return Protocol::Unknown;
            }

            match buffer[0] {
                0x05 => Protocol::Socks5,
                0x16 if buffer.len() >= 3 && buffer[1] == 0x03 => Protocol::Tls,
                b'G' if buffer.len() >= 4 && &buffer[0..4] == b"GET " => Protocol::Http,
                b'P' if buffer.len() >= 5 => {
                    match &buffer[0..5] {
                        b"POST " => Protocol::Http,
                        b"PUT " => Protocol::Http,
                        _ => Protocol::Unknown,
                    }
                },
                _ => Protocol::Unknown,
            }
        }
    }
}

#[cfg(all(test, any(target_arch = "x86_64", target_arch = "aarch64")))]
mod tests {
    use super::*;

    #[test]
    fn test_simd_detection() {
        let detector = SimdDetector::new();
        
        assert!(matches!(detector.detect_simd(b"GET / HTTP/1.1\r\n"), Protocol::Http));
        assert!(matches!(detector.detect_simd(&[0x05, 0x01, 0x00]), Protocol::Socks5));
        assert!(matches!(detector.detect_simd(&[0x16, 0x03, 0x03]), Protocol::Tls));
        assert!(matches!(detector.detect_simd(b"PROXY TCP4"), Protocol::ProxyProtocol));
    }
}