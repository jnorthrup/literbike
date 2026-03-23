//! Agent8888 Protocol Detection - Top level module (always enabled)
//!
//! This is the ROOT of the CCEK protocol hierarchy. It defines cross-cutting
//! protocol Elements (QuicKey, SctpKey, HttpKey, HtxKey, TlsKey, SshKey) that
//! are visible throughout the CCEK workspace.
//!
//! Note: Feature-gated submodules (matcher, listener, reactor, handler, timer)
//! are conditionally compiled and not visible here at the protocol level.
//! This is by design - protocol detection sits at the top, routing to
//! specialized implementations in their respective crates.

use crate::core::{Element, Key};
use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicU32, Ordering};

/// CCEK-compatible detection result
#[derive(Debug, Clone)]
pub struct CcekDetectionResult {
    pub protocol_name: String,
    pub confidence: u8,
    pub rarity_score: f64, // 0.0 (common) to 1.0 (rare)
    pub flags: BitFlags,
    pub metadata: Option<Vec<u8>>,
}

/// Bit flags for protocol detection
#[derive(Debug, Clone, Copy)]
pub struct BitFlags(u8);

impl BitFlags {
    pub const NONE: Self = Self(0);
    pub const HIGH_CONFIDENCE: Self = Self(1 << 0);
    pub const ENCRYPTED: Self = Self(1 << 1);
    pub const BINARY: Self = Self(1 << 2);

    pub fn new(flags: u8) -> Self {
        Self(flags)
    }

    pub fn has_flag(&self, flag: BitFlags) -> bool {
        (self.0 & flag.0) != 0
    }

    pub fn set_flag(&mut self, flag: BitFlags) {
        self.0 |= flag.0;
    }

    pub fn toggle_flag(&mut self, flag: BitFlags) {
        self.0 ^= flag.0;
    }
}

/// CCEK Protocol Detector trait - replaces async_trait pattern
pub trait CcekProtocolDetector: Send + Sync {
    /// Inspect the provided bytes and return a CcekDetectionResult.
    fn detect(&self, data: &[u8]) -> CcekDetectionResult;

    /// Human-readable name of the protocol this detector recognizes.
    fn protocol_name(&self) -> &'static str {
        "unknown"
    }

    /// A heuristic confidence threshold used by tests.
    fn confidence_threshold(&self) -> u8 {
        200
    }

    /// Priority for detection order (higher = checked first)
    fn priority(&self) -> u8 {
        100
    }
}

/// CCEK Protocol Handler trait
pub trait CcekProtocolHandler: Send + Sync {
    /// Handle protocol-specific processing
    fn handle(&self, data: &[u8], detection: &CcekDetectionResult) -> CcekHandlerResult;

    /// Protocol this handler supports
    fn supported_protocol(&self) -> &'static str;
}

/// Result from protocol handling
#[derive(Debug)]
pub enum CcekHandlerResult {
    Handled(usize),      // Bytes consumed
    NeedMoreData,        // Need additional data
    Error(&'static str), // Error occurred
    Unsupported,         // Handler doesn't support this protocol
}

/// CCEK Protocol Registry Key
pub struct CcekProtocolRegistryKey {
    pub name: &'static str,
    pub priority: u8,
}

impl CcekProtocolRegistryKey {
    pub fn new(name: &'static str, priority: u8) -> Self {
        Self { name, priority }
    }

    pub const FACTORY: fn() -> CcekProtocolRegistryElement = || CcekProtocolRegistryElement::new();
}

/// CCEK Protocol Registry Element
pub struct CcekProtocolRegistryElement {
    detectors: Vec<Box<dyn CcekProtocolDetector>>,
    handlers: Vec<Box<dyn CcekProtocolHandler>>,
    total_detections: AtomicU32,
}

impl CcekProtocolRegistryElement {
    pub fn new() -> Self {
        Self {
            detectors: Vec::new(),
            handlers: Vec::new(),
            total_detections: AtomicU32::new(0),
        }
    }

    /// Register a detector with the registry
    pub fn register_detector(&mut self, detector: Box<dyn CcekProtocolDetector>) {
        // Insert in priority order (higher priority first)
        let priority = detector.priority();
        let insert_pos = self
            .detectors
            .iter()
            .position(|d| d.priority() < priority)
            .unwrap_or(self.detectors.len());
        self.detectors.insert(insert_pos, detector);
    }

    /// Register a handler with the registry
    pub fn register_handler(&mut self, handler: Box<dyn CcekProtocolHandler>) {
        self.handlers.push(handler);
    }

    /// Detect protocol using registered detectors
    pub fn detect_protocol(&self, data: &[u8]) -> Option<CcekDetectionResult> {
        for detector in &self.detectors {
            let result = detector.detect(data);
            if result.confidence >= detector.confidence_threshold() {
                self.total_detections.fetch_add(1, Ordering::Relaxed);
                return Some(result);
            }
        }
        None
    }

    /// Handle protocol using registered handlers
    pub fn handle_protocol(
        &self,
        data: &[u8],
        detection: &CcekDetectionResult,
    ) -> CcekHandlerResult {
        for handler in &self.handlers {
            if handler.supported_protocol() == detection.protocol_name {
                return handler.handle(data, detection);
            }
        }
        CcekHandlerResult::Unsupported
    }

    /// Get total detections count
    pub fn total_detections(&self) -> u32 {
        self.total_detections.load(Ordering::Relaxed)
    }
}

impl Element for CcekProtocolRegistryElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<CcekProtocolRegistryKey>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Key for CcekProtocolRegistryKey {
    type Element = CcekProtocolRegistryElement;
    const FACTORY: fn() -> Self::Element = || CcekProtocolRegistryElement::new();
}

/// Agent8888Key - Top level, protocol detection
pub struct Agent8888Key {
    pub port: u16,
    pub factory: fn() -> Agent8888Element,
}

impl Agent8888Key {
    pub const DEFAULT_PORT: u16 = 8888;

    /// Key's factory - Context calls this
    pub const FACTORY: fn() -> Agent8888Element = || Agent8888Element::new(Self::DEFAULT_PORT);

    pub fn new() -> Self {
        Self {
            port: Self::DEFAULT_PORT,
            factory: Self::FACTORY,
        }
    }

    pub fn with_port(port: u16) -> Self {
        Self {
            port,
            factory: Agent8888Key::FACTORY, // Always use the const factory
        }
    }
}

/// Agent8888Element - State container for agent8888
pub struct Agent8888Element {
    pub port: u16,
    pub connections: AtomicU32,
}

impl Agent8888Element {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            connections: AtomicU32::new(0),
        }
    }

    pub fn increment_connections(&self) {
        self.connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn connections(&self) -> u32 {
        self.connections.load(Ordering::Relaxed)
    }
}

impl Element for Agent8888Element {
    fn key_type(&self) -> TypeId {
        TypeId::of::<Agent8888Key>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Key for Agent8888Key {
    type Element = Agent8888Element;
    const FACTORY: fn() -> Self::Element = || Agent8888Element::new(Self::DEFAULT_PORT);
}

/// QuicKey - QUIC protocol
pub struct QuicKey;

impl QuicKey {
    pub const FACTORY: fn() -> QuicElement = || QuicElement::new();
}

pub struct QuicElement {
    pub port: u16,
}

impl QuicElement {
    pub fn new() -> Self {
        Self { port: 443 }
    }
}

impl Element for QuicElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<QuicKey>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Key for QuicKey {
    type Element = QuicElement;
    const FACTORY: fn() -> Self::Element = || QuicElement::new();
}

/// TlsKey - TLS protocol
pub struct TlsKey;

impl TlsKey {
    pub const FACTORY: fn() -> TlsElement = || TlsElement::new();
}

pub struct TlsElement {
    pub version: u16,
}

impl TlsElement {
    pub fn new() -> Self {
        Self { version: 0x0303 } // TLS 1.2
    }
}

impl Element for TlsElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<TlsKey>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Key for TlsKey {
    type Element = TlsElement;
    const FACTORY: fn() -> Self::Element = || TlsElement::new();
}

/// SctpKey - SCTP protocol
pub struct SctpKey;

impl SctpKey {
    pub const FACTORY: fn() -> SctpElement = || SctpElement::new();
}

pub struct SctpElement {
    pub port: u16,
}

impl SctpElement {
    pub fn new() -> Self {
        Self { port: 9899 } // Default SCTP port
    }
}

impl Element for SctpElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<SctpKey>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Key for SctpKey {
    type Element = SctpElement;
    const FACTORY: fn() -> Self::Element = || SctpElement::new();
}

/// HttpKey - HTTP protocol
pub struct HttpKey;

impl HttpKey {
    pub const FACTORY: fn() -> HttpElement = || HttpElement::new();
}

pub struct HttpElement {
    pub port: u16,
}

impl HttpElement {
    pub fn new() -> Self {
        Self { port: 80 }
    }
}

impl Element for HttpElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<HttpKey>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Key for HttpKey {
    type Element = HttpElement;
    const FACTORY: fn() -> Self::Element = || HttpElement::new();
}

/// HtxKey - HTX crypto protocol
pub struct HtxKey;

impl HtxKey {
    pub const FACTORY: fn() -> HtxElement = || HtxElement::new();
}

pub struct HtxElement {
    pub curve: &'static str,
}

impl HtxElement {
    pub fn new() -> Self {
        Self { curve: "X25519" }
    }
}

impl Element for HtxElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<HtxKey>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Key for HtxKey {
    type Element = HtxElement;
    const FACTORY: fn() -> Self::Element = || HtxElement::new();
}

/// SshKey - SSH protocol
pub struct SshKey;

impl SshKey {
    pub const FACTORY: fn() -> SshElement = || SshElement::new();
}

pub struct SshElement {
    pub port: u16,
}

impl SshElement {
    pub fn new() -> Self {
        Self { port: 22 }
    }
}

impl Element for SshElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<SshKey>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Key for SshKey {
    type Element = SshElement;
    const FACTORY: fn() -> Self::Element = || SshElement::new();
}

/// HTTP methods detected by byte-sniff
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
}

/// Protocol detection result from first-read byte sniff
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolDetection {
    Http(HttpMethod),
    Https,
    Quic,
    Tls,
    Sctp,
    Ssh,
    Socks5,
    WebSocket,
    Pac,
    Wpad,
    Upnp,
    Bonjour,
    Unknown,
}

/// Detect protocol from the first bytes read off the wire
pub fn detect_protocol(buf: &[u8]) -> ProtocolDetection {
    if buf.is_empty() {
        return ProtocolDetection::Unknown;
    }

    // TLS: Client Hello starts with 0x16 0x03
    if buf.len() >= 2 && buf[0] == 0x16 && buf[1] == 0x03 {
        return ProtocolDetection::Tls;
    }

    // QUIC: Long header first byte >= 0xc0 (high bit set)
    if buf[0] & 0x80 != 0 {
        return ProtocolDetection::Quic;
    }

    // SCTP: INIT chunk type 1, but check common header
    if buf.len() >= 12 && buf[0] == 0x01 {
        return ProtocolDetection::Sctp;
    }

    // SOCKS5: first byte 0x05
    if buf[0] == 0x05 {
        return ProtocolDetection::Socks5;
    }

    // Text-based protocols
    if let Ok(text) = std::str::from_utf8(&buf[..buf.len().min(512)]) {
        let text_upper = text.to_uppercase();

        let method = if text.starts_with("GET ") {
            Some(HttpMethod::Get)
        } else if text.starts_with("POST ") {
            Some(HttpMethod::Post)
        } else if text.starts_with("PUT ") {
            Some(HttpMethod::Put)
        } else if text.starts_with("DELETE ") {
            Some(HttpMethod::Delete)
        } else if text.starts_with("HEAD ") {
            Some(HttpMethod::Head)
        } else if text.starts_with("OPTIONS ") {
            Some(HttpMethod::Options)
        } else if text.starts_with("CONNECT ") {
            Some(HttpMethod::Connect)
        } else if text.starts_with("PATCH ") {
            Some(HttpMethod::Patch)
        } else {
            None
        };

        if let Some(method) = method {
            if text_upper.contains("UPGRADE: WEBSOCKET") {
                return ProtocolDetection::WebSocket;
            }
            if text.contains("/wpad.dat") {
                return ProtocolDetection::Wpad;
            }
            if text.contains("/proxy.pac") {
                return ProtocolDetection::Pac;
            }
            return ProtocolDetection::Http(method);
        }

        // SSH: starts with "SSH-"
        if text.starts_with("SSH-") {
            return ProtocolDetection::Ssh;
        }

        // UPnP SSDP M-SEARCH / NOTIFY
        if text.starts_with("M-SEARCH ") || text.starts_with("NOTIFY ") {
            return ProtocolDetection::Upnp;
        }
    }

    ProtocolDetection::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_socks5() {
        assert_eq!(
            detect_protocol(&[0x05, 0x01, 0x00]),
            ProtocolDetection::Socks5
        );
    }

    #[test]
    fn test_detect_http_get() {
        let buf = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        assert_eq!(
            detect_protocol(buf),
            ProtocolDetection::Http(HttpMethod::Get)
        );
    }

    #[test]
    fn test_detect_http_connect() {
        let buf = b"CONNECT example.com:443 HTTP/1.1\r\n\r\n";
        assert_eq!(
            detect_protocol(buf),
            ProtocolDetection::Http(HttpMethod::Connect)
        );
    }

    #[test]
    fn test_detect_websocket() {
        let buf = b"GET /ws HTTP/1.1\r\nUpgrade: websocket\r\n\r\n";
        assert_eq!(detect_protocol(buf), ProtocolDetection::WebSocket);
    }

    #[test]
    fn test_detect_pac() {
        let buf = b"GET /proxy.pac HTTP/1.1\r\n\r\n";
        assert_eq!(detect_protocol(buf), ProtocolDetection::Pac);
    }

    #[test]
    fn test_detect_wpad() {
        let buf = b"GET /wpad.dat HTTP/1.1\r\n\r\n";
        assert_eq!(detect_protocol(buf), ProtocolDetection::Wpad);
    }

    #[test]
    fn test_detect_upnp() {
        let buf = b"M-SEARCH * HTTP/1.1\r\n\r\n";
        assert_eq!(detect_protocol(buf), ProtocolDetection::Upnp);
    }

    #[test]
    fn test_detect_tls() {
        let buf = &[0x16, 0x03, 0x01]; // TLS 1.0 Client Hello
        assert_eq!(detect_protocol(buf), ProtocolDetection::Tls);
    }

    #[test]
    fn test_detect_quic() {
        let buf = &[0xc0, 0x00]; // QUIC long header
        assert_eq!(detect_protocol(buf), ProtocolDetection::Quic);
    }

    #[test]
    fn test_detect_sctp() {
        let buf = &[0x01, 0x00, 0x00, 0x00]; // SCTP INIT chunk
        assert_eq!(detect_protocol(buf), ProtocolDetection::Sctp);
    }

    #[test]
    fn test_detect_ssh() {
        let buf = b"SSH-2.0-OpenSSH_8.0\r\n";
        assert_eq!(detect_protocol(buf), ProtocolDetection::Ssh);
    }

    #[test]
    fn test_detect_unknown() {
        assert_eq!(
            detect_protocol(&[0x00, 0xFF, 0xAA]),
            ProtocolDetection::Unknown
        );
        assert_eq!(detect_protocol(&[]), ProtocolDetection::Unknown);
    }
}
