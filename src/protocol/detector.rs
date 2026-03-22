//! Unified protocol detector using userspace network abstractions
//!
//! Provides protocol detection and routing for HTTP, HTTPS, HTTP2, HTTP3,
//! QUIC, SSH, TLS, WebSocket, and SCTP protocols.

use crate::protocol::Protocol;
use std::io;
use std::sync::Arc;

/// Result from protocol handler
pub type HandlerResult = io::Result<()>;

/// Trait for handling detected protocols
pub trait ProtocolHandler: Send + Sync {
    /// Handle incoming data for this protocol
    fn handle(&mut self, data: &[u8]) -> HandlerResult;

    /// Get the protocol this handler supports
    fn protocol(&self) -> Protocol;

    /// Check if handler is ready for more data
    fn is_ready(&self) -> bool {
        true
    }
}

/// Unified protocol detector that wraps userspace::network::protocols::ProtocolDetector
pub struct UnifiedDetector {
    inner: userspace::network::protocols::ProtocolDetector,
    handlers: Arc<parking_lot::Mutex<Vec<Box<dyn ProtocolHandler>>>>,
}

impl UnifiedDetector {
    /// Create a new unified detector
    pub fn new() -> Self {
        Self {
            inner: userspace::network::protocols::ProtocolDetector::new(),
            handlers: Arc::new(parking_lot::Mutex::new(Vec::new())),
        }
    }

    /// Add a protocol handler
    pub fn add_handler(&self, handler: Box<dyn ProtocolHandler>) {
        self.handlers.lock().push(handler);
    }

    /// Feed data to the detector
    pub fn feed(&mut self, data: &[u8]) -> io::Result<Option<Protocol>> {
        self.inner.feed(data);
        Ok(self.inner.protocol())
    }

    /// Get detected protocol
    pub fn protocol(&self) -> Option<Protocol> {
        self.inner.protocol()
    }

    /// Dispatch data to the appropriate handler
    pub fn dispatch(&self, data: &[u8]) -> io::Result<()> {
        let protocol = self
            .protocol()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no protocol detected"))?;

        let mut handlers = self.handlers.lock();
        for handler in handlers.iter_mut() {
            if handler.protocol() == protocol && handler.is_ready() {
                return handler.handle(data);
            }
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no handler for protocol: {:?}", protocol),
        ))
    }

    /// Reset the detector
    pub fn reset(&mut self) {
        self.inner.reset();
    }
}

impl Default for UnifiedDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Detect protocol from byte slice
pub fn detect_protocol(data: &[u8]) -> Protocol {
    userspace::network::protocols::detect_protocol(data)
}

/// Extended protocol detection including SCTP
pub fn detect_protocol_extended(data: &[u8]) -> Protocol {
    // Try standard detection first
    let protocol = detect_protocol(data);

    if protocol != Protocol::Unknown {
        return protocol;
    }

    // Check for SCTP by looking at first 4 bytes
    // SCTP common header: src_port (2) + dst_port (2)
    if data.len() >= 12 {
        // SCTP ports are typically > 1024 and verification tag is non-zero
        let src_port = u16::from_be_bytes([data[0], data[1]]);
        let dst_port = u16::from_be_bytes([data[2], data[3]]);
        let verification_tag = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);

        // Common SCTP port check + verification tag heuristic
        if (src_port > 1024 || dst_port > 1024) && verification_tag != 0 {
            return Protocol::Raw; // SCTP uses raw IP, mapped to Raw
        }
    }

    Protocol::Unknown
}

/// Protocol detection context for stateful detection
pub struct ProtocolContext {
    detector: UnifiedDetector,
    buffer: Vec<u8>,
    max_buffer_size: usize,
}

impl ProtocolContext {
    /// Create a new protocol context
    pub fn new() -> Self {
        Self {
            detector: UnifiedDetector::new(),
            buffer: Vec::with_capacity(4096),
            max_buffer_size: 65536,
        }
    }

    /// Set maximum buffer size
    pub fn with_max_buffer_size(mut self, size: usize) -> Self {
        self.max_buffer_size = size;
        self
    }

    /// Feed data and detect protocol
    pub fn feed(&mut self, data: &[u8]) -> io::Result<Option<Protocol>> {
        if self.buffer.len() + data.len() > self.max_buffer_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "buffer overflow",
            ));
        }

        self.buffer.extend_from_slice(data);
        self.detector.feed(&self.buffer)
    }

    /// Get detected protocol
    pub fn protocol(&self) -> Option<Protocol> {
        self.detector.protocol()
    }

    /// Get buffered data
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    /// Clear buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.detector.reset();
    }

    /// Add handler to internal detector
    pub fn add_handler(&self, handler: Box<dyn ProtocolHandler>) {
        self.detector.add_handler(handler);
    }
}

impl Default for ProtocolContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_detection() {
        let data = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let protocol = detect_protocol(data);
        assert_eq!(protocol, Protocol::Http);
    }

    #[test]
    fn test_ssh_detection() {
        let data = b"SSH-2.0-OpenSSH_8.0\r\n";
        let protocol = detect_protocol(data);
        assert_eq!(protocol, Protocol::Ssh);
    }

    #[test]
    fn test_tls_detection() {
        let data = &[0x16, 0x03, 0x01, 0x00]; // TLS handshake
        let protocol = detect_protocol(data);
        assert_eq!(protocol, Protocol::Tls);
    }

    #[test]
    fn test_quic_detection() {
        let data = &[0xc0]; // QUIC long header
        let protocol = detect_protocol(data);
        assert_eq!(protocol, Protocol::Quic);
    }

    #[test]
    fn test_protocol_context() {
        let mut ctx = ProtocolContext::new();
        let result = ctx.feed(b"GET /");
        assert!(result.is_ok());
    }

    #[test]
    fn test_unified_detector() {
        let mut detector = UnifiedDetector::new();
        let result = detector.feed(b"POST /api HTTP/1.1\r\n");
        assert!(result.is_ok());
        assert_eq!(detector.protocol(), Some(Protocol::Http));
    }
}
