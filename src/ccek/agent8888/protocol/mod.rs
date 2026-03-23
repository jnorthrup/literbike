//! Agent8888 Protocol Detection - Top level module (always enabled)
//!
//! This module CANNOT see matcher, listener, reactor, etc.
//! It only knows about itself and the core traits.

use crate::core::{Element, Key};
use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicU32, Ordering};

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
    Socks5,
    WebSocket,
    Pac,
    Wpad,
    Upnp,
    Unknown,
}

/// Detect protocol from the first bytes read off the wire
pub fn detect_protocol(buf: &[u8]) -> ProtocolDetection {
    if buf.is_empty() {
        return ProtocolDetection::Unknown;
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
    fn test_detect_unknown() {
        assert_eq!(
            detect_protocol(&[0x00, 0xFF, 0xAA]),
            ProtocolDetection::Unknown
        );
        assert_eq!(detect_protocol(&[]), ProtocolDetection::Unknown);
    }
}
