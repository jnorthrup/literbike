use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::net::TcpStream;
use log::{info, error, warn};
use crate::types::{BitFlags, ShadowsocksMethod};

use crate::types::{
    ProtocolType, TargetAddress, ConnectionState, ProtocolDetectionResult
};

pub trait SocketListener: Send + Sync {
    fn protocol(&self) -> ProtocolType;
    fn bind_address(&self) -> SocketAddr;
    async fn accept(&mut self) -> io::Result<Box<dyn UniversalStream>>;
    async fn handle_connection(&mut self, stream: Box<dyn UniversalStream>) -> io::Result<()>;
}

pub trait UniversalStream: AsyncRead + AsyncWrite + Send + Sync + Unpin {
    fn peer_addr(&self) -> io::Result<SocketAddr>;
    fn local_addr(&self) -> io::Result<SocketAddr>;
    fn set_state(&mut self, state: ConnectionState);
    fn get_state(&self) -> ConnectionState;
    fn get_flags(&self) -> BitFlags;
    fn set_flags(&mut self, flags: BitFlags);
    fn protocol_type(&self) -> ProtocolType;
}

pub trait ProtocolDetector: Send + Sync {
    fn detect(&self, buffer: &[u8]) -> Vec<ProtocolDetectionResult>;
    fn confidence_threshold(&self) -> u8;
    fn required_bytes(&self) -> usize;
}

pub trait ProtocolHandler: Send + Sync {
    fn protocol(&self) -> ProtocolType;
    async fn handle<S>(&self, stream: S, detection: ProtocolDetectionResult) -> io::Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send;
    async fn validate_handshake(&self, buffer: &[u8]) -> bool;
}

pub trait CryptoAbstraction: Send + Sync {
    fn encrypt(&self, plaintext: &[u8], key: &[u8], nonce: &[u8]) -> io::Result<Vec<u8>>;
    fn decrypt(&self, ciphertext: &[u8], key: &[u8], nonce: &[u8]) -> io::Result<Vec<u8>>;
    fn key_length(&self) -> usize;
    fn nonce_length(&self) -> usize;
    fn tag_length(&self) -> usize;
}

pub trait BitBangInterface: Send + Sync {
    fn extract_bits(&self, data: &[u8], bit_offset: usize, bit_count: usize) -> u64;
    fn set_bits(&self, data: &mut [u8], bit_offset: usize, bit_count: usize, value: u64);
    fn flip_bits(&self, data: &mut [u8], bit_mask: &[u8]);
    fn count_set_bits(&self, data: &[u8]) -> usize;
    fn find_bit_pattern(&self, data: &[u8], pattern: &[u8], mask: &[u8]) -> Option<usize>;
}

#[derive(Debug)]
pub struct UniversalConnection {
    inner: TcpStream,
    state: ConnectionState,
    flags: BitFlags,
    protocol: ProtocolType,
    buffer: Vec<u8>,
    detection_result: Option<ProtocolDetectionResult>,
}

impl UniversalConnection {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            inner: stream,
            state: ConnectionState::Idle,
            flags: BitFlags::NONE,
            protocol: ProtocolType::Raw,
            buffer: Vec::with_capacity(4096),
            detection_result: None,
        }
    }

    // Temporarily disabled due to trait object compatibility issues
    // pub async fn detect_protocol(&mut self, detectors: &[Box<dyn ProtocolDetector>]) -> io::Result<ProtocolDetectionResult> {
    //     // Implementation temporarily removed - use universal_listener.rs instead
    // }
    
    pub async fn detect_protocol_simple(&mut self) -> io::Result<ProtocolDetectionResult> {
        // Simplified detection without trait objects
        if self.buffer.len() < 16 {
            let mut temp_buf = [0u8; 1024];
            let n = self.inner.read(&mut temp_buf).await?;
            self.buffer.extend_from_slice(&temp_buf[..n]);
        }

        // Basic protocol detection - return Raw for now
        let best_result = ProtocolDetectionResult {
            protocol_name: "raw".to_string(),
            confidence: 50,
            flags: BitFlags::NONE,
            rarity_score: 0.0,
            metadata: None,
        };

        self.detection_result = Some(best_result.clone());
        self.protocol = ProtocolType::Raw;
        Ok(best_result)
    }

    pub fn get_buffered_data(&self) -> &[u8] {
        &self.buffer
    }

    pub fn consume_buffer(&mut self, bytes: usize) {
        if bytes >= self.buffer.len() {
            self.buffer.clear();
        } else {
            self.buffer.drain(..bytes);
        }
    }
}

impl AsyncRead for UniversalConnection {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if !self.buffer.is_empty() {
            let to_copy = std::cmp::min(buf.remaining(), self.buffer.len());
            buf.put_slice(&self.buffer[..to_copy]);
            self.buffer.drain(..to_copy);
            Poll::Ready(Ok(()))
        } else {
            Pin::new(&mut self.inner).poll_read(cx, buf)
        }
    }
}

impl AsyncWrite for UniversalConnection {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl UniversalStream for UniversalConnection {
    fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.inner.peer_addr()
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    fn set_state(&mut self, state: ConnectionState) {
        self.state = state;
    }

    fn get_state(&self) -> ConnectionState {
        self.state
    }

    fn get_flags(&self) -> BitFlags {
        self.flags
    }

    fn set_flags(&mut self, flags: BitFlags) {
        self.flags = flags;
    }

    fn protocol_type(&self) -> ProtocolType {
        self.protocol
    }
}

pub struct HttpDetector;

impl ProtocolDetector for HttpDetector {
    fn detect(&self, buffer: &[u8]) -> Vec<ProtocolDetectionResult> {
        let data = String::from_utf8_lossy(buffer);
        let confidence = if data.starts_with("GET ") || data.starts_with("POST ") ||
                           data.starts_with("PUT ") || data.starts_with("DELETE ") ||
                           data.starts_with("HEAD ") || data.starts_with("OPTIONS ") ||
                           data.starts_with("CONNECT ") || data.starts_with("TRACE ") ||
                           data.starts_with("PATCH ") {
            255
        } else if data.contains("HTTP/1.") {
            200
        } else {
            0
        };

        let protocol_name = if data.starts_with("CONNECT ") {
            "connect".to_string()
        } else if data.contains("/dns-query") {
            "doh".to_string()
        } else {
            "http".to_string()
        };

        vec![ProtocolDetectionResult {
            protocol_name,
            confidence,
            flags: BitFlags::NONE,
            rarity_score: 0.0,
            metadata: Some(buffer.to_vec()),
        }]
    }

    fn confidence_threshold(&self) -> u8 { 150 }
    fn required_bytes(&self) -> usize { 8 }
}

pub struct TlsDetector;

impl ProtocolDetector for TlsDetector {
    fn detect(&self, buffer: &[u8]) -> Vec<ProtocolDetectionResult> {
        let confidence = if buffer.len() >= 6 {
            if buffer[0] == 0x16 && // Handshake
               (buffer[1] == 0x03 && (buffer[2] == 0x01 || buffer[2] == 0x02 || buffer[2] == 0x03 || buffer[2] == 0x04)) {
                255
            } else if buffer[0] == 0x14 || buffer[0] == 0x15 || buffer[0] == 0x17 { // Change cipher, alert, application data
                150
            } else {
                0
            }
        } else {
            0
        };

        vec![ProtocolDetectionResult {
            protocol_name: "tls".to_string(),
            confidence,
            flags: if confidence > 200 { BitFlags::ENCRYPTED } else { BitFlags::NONE },
            rarity_score: 0.0,
            metadata: Some(buffer.to_vec()),
        }]
    }

    fn confidence_threshold(&self) -> u8 { 200 }
    fn required_bytes(&self) -> usize { 6 }
}

pub struct UpnpDetector;

impl ProtocolDetector for UpnpDetector {
    fn detect(&self, buffer: &[u8]) -> Vec<ProtocolDetectionResult> {
        let data = String::from_utf8_lossy(buffer);
        let confidence = if data.starts_with("M-SEARCH ") {
            255
        } else if data.starts_with("NOTIFY ") {
            240
        } else if data.contains("M-SEARCH") || data.contains("NOTIFY") ||
                   data.contains("SUBSCRIBE") || data.contains("UNSUBSCRIBE") {
            200
        } else if data.contains("AddPortMapping") || data.contains("DeletePortMapping") ||
                   data.contains("GetExternalIPAddress") {
            180
        } else if data.contains("urn:schemas-upnp-org") {
            150
        } else {
            0
        };

        vec![ProtocolDetectionResult {
            protocol_name: "upnp".to_string(),
            confidence,
            flags: BitFlags::NONE,
            rarity_score: 0.0,
            metadata: Some(buffer.to_vec()),
        }]
    }

    fn confidence_threshold(&self) -> u8 { 150 }
    fn required_bytes(&self) -> usize { 10 }
}

pub struct ShadowsocksDetector {
    methods: Vec<ShadowsocksMethod>,
    passwords: Vec<String>,
}

impl ShadowsocksDetector {
    pub fn new(methods: Vec<ShadowsocksMethod>, passwords: Vec<String>) -> Self {
        Self { methods, passwords }
    }
}

impl ProtocolDetector for ShadowsocksDetector {
    fn detect(&self, buffer: &[u8]) -> Vec<ProtocolDetectionResult> {
        let confidence = if buffer.len() >= 32 {
            if self.is_likely_encrypted(buffer) {
                100 // Lower confidence since we can't verify without decryption
            } else {
                0
            }
        } else {
            0
        };

        vec![ProtocolDetectionResult {
            protocol_name: "shadowsocks".to_string(),
            confidence,
            flags: BitFlags::ENCRYPTED,
            rarity_score: 0.0,
            metadata: Some(buffer.to_vec()),
        }]
    }

    fn confidence_threshold(&self) -> u8 { 80 }
    fn required_bytes(&self) -> usize { 32 }
}

impl ShadowsocksDetector {
    fn is_likely_encrypted(&self, data: &[u8]) -> bool {
        let mut entropy = 0.0;
        let mut counts = [0u32; 256];
        
        for &byte in data {
            counts[byte as usize] += 1;
        }
        
        for &count in &counts {
            if count > 0 {
                let p = count as f64 / data.len() as f64;
                entropy -= p * p.log2();
            }
        }
        
        entropy > 7.0 // High entropy suggests encryption
    }
}

pub struct BitBangProcessor;

impl BitBangInterface for BitBangProcessor {
    fn extract_bits(&self, data: &[u8], bit_offset: usize, bit_count: usize) -> u64 {
        let mut result = 0u64;
        let mut bits_read = 0;
        let mut current_bit_offset = bit_offset;
        
        while bits_read < bit_count && current_bit_offset / 8 < data.len() {
            let byte_index = current_bit_offset / 8;
            let bit_index = current_bit_offset % 8;
            let byte_value = data[byte_index];
            
            if (byte_value >> (7 - bit_index)) & 1 == 1 {
                result |= 1u64 << (bit_count - 1 - bits_read);
            }
            
            bits_read += 1;
            current_bit_offset += 1;
        }
        
        result
    }

    fn set_bits(&self, data: &mut [u8], bit_offset: usize, bit_count: usize, value: u64) {
        let mut bits_written = 0;
        let mut current_bit_offset = bit_offset;
        
        while bits_written < bit_count && current_bit_offset / 8 < data.len() {
            let byte_index = current_bit_offset / 8;
            let bit_index = current_bit_offset % 8;
            let bit_value = (value >> (bit_count - 1 - bits_written)) & 1;
            
            if bit_value == 1 {
                data[byte_index] |= 1 << (7 - bit_index);
            } else {
                data[byte_index] &= !(1 << (7 - bit_index));
            }
            
            bits_written += 1;
            current_bit_offset += 1;
        }
    }

    fn flip_bits(&self, data: &mut [u8], bit_mask: &[u8]) {
        let len = std::cmp::min(data.len(), bit_mask.len());
        for i in 0..len {
            data[i] ^= bit_mask[i];
        }
    }

    fn count_set_bits(&self, data: &[u8]) -> usize {
        data.iter().map(|&byte| byte.count_ones() as usize).sum()
    }

    fn find_bit_pattern(&self, data: &[u8], pattern: &[u8], mask: &[u8]) -> Option<usize> {
        if pattern.is_empty() || data.len() < pattern.len() {
            return None;
        }
        
        for i in 0..=(data.len() - pattern.len()) {
            let mut matches = true;
            for j in 0..pattern.len() {
                let masked_data = data[i + j] & mask.get(j).copied().unwrap_or(0xFF);
                let masked_pattern = pattern[j] & mask.get(j).copied().unwrap_or(0xFF);
                if masked_data != masked_pattern {
                    matches = false;
                    break;
                }
            }
            if matches {
                return Some(i);
            }
        }
        None
    }
}

pub struct UniversalProxy {
    // Temporarily use empty vectors to avoid trait object issues
    // detectors: Vec<Box<dyn ProtocolDetector>>,
    // handlers: Vec<Box<dyn ProtocolHandler>>,
    bitbang: BitBangProcessor,
}

impl UniversalProxy {
    pub fn new() -> Self {
        let mut proxy = Self {
            // detectors: Vec::new(),
            // handlers: Vec::new(),
            bitbang: BitBangProcessor,
        };
        
        proxy.add_default_detectors();
        proxy
    }

    fn add_default_detectors(&mut self) {
        // Temporarily disabled due to trait object compatibility issues
        // self.detectors.push(Box::new(HttpDetector));
        // self.detectors.push(Box::new(Socks5Detector));
        // self.detectors.push(Box::new(TlsDetector));
        // self.detectors.push(Box::new(UpnpDetector));
        // self.detectors.push(Box::new(ShadowsocksDetector::new(
        //     vec![ShadowsocksMethod::Aes256Gcm, ShadowsocksMethod::Chacha20IetfPoly1305],
        //     vec!["default-password".to_string()]
        // )));
    }

    pub async fn handle_connection(&self, mut stream: UniversalConnection) -> io::Result<()> {
        // Simplified implementation - the trait object system is temporarily disabled
        // Real protocol detection happens in universal_listener.rs which works fine
        info!("UniversalProxy: handling connection with bitbang processor");
        
        // Use the bitbang processor for basic operation
        stream.set_state(ConnectionState::Connected);
        
        // For now, just return success - actual protocol handling is done by universal_listener
        Ok(())
    }

    // Temporarily disabled due to trait object compatibility issues
    // pub fn add_detector(&mut self, detector: Box<dyn ProtocolDetector>) {
    //     self.detectors.push(detector);
    // }

    // pub fn add_handler(&mut self, handler: Box<dyn ProtocolHandler>) {
    //     self.handlers.push(handler);
    // }

    pub fn bitbang(&self) -> &BitBangProcessor {
        &self.bitbang
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_http_detection() {
        let detector = HttpDetector;
        let http_request = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let result = detector.detect(http_request).await;
        
        assert_eq!(result.protocol, ProtocolType::Http);
        assert_eq!(result.confidence, 255);
    }

    #[tokio::test]  
    async fn test_socks5_detection() {
        let detector = Socks5Detector;
        let socks5_handshake = &[0x05, 0x01, 0x00]; // SOCKS5, 1 method, no auth
        let result = detector.detect(socks5_handshake).await;
        
        assert_eq!(result.protocol, ProtocolType::Socks5);
        assert_eq!(result.confidence, 255);
    }

    #[test]
    fn test_bitbang_operations() {
        let processor = BitBangProcessor;
        let data = [0b10101010, 0b11110000];
        
        // Test bit extraction
        let bits = processor.extract_bits(&data, 0, 4);
        assert_eq!(bits, 0b1010);
        
        // Test bit counting
        let count = processor.count_set_bits(&data);
        assert_eq!(count, 8);
        
        // Test pattern finding
        let pattern = [0b1010];
        let mask = [0xFF];
        let pos = processor.find_bit_pattern(&data, &pattern, &mask);
        assert_eq!(pos, None); // Exact byte match not found
    }
}