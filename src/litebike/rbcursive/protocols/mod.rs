// Network protocol parsers using RBCursive combinators
// High-performance, zero-allocation protocol parsing

pub mod http;
pub mod socks5;
pub mod json;

pub use http::*;
pub use socks5::*;
pub use json::*;

// Protocol module re-exports

/// Common protocol detection result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolType {
    Http(HttpMethod),
    Socks5,
    Json,
    Unknown,
}

/// HTTP methods supported by the parser
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
    Trace,
}

impl HttpMethod {
    pub fn as_bytes(&self) -> &'static [u8] {
        match self {
            Self::Get => b"GET",
            Self::Post => b"POST",
            Self::Put => b"PUT",
            Self::Delete => b"DELETE",
            Self::Head => b"HEAD",
            Self::Options => b"OPTIONS",
            Self::Connect => b"CONNECT",
            Self::Patch => b"PATCH",
            Self::Trace => b"TRACE",
        }
    }
    
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        match bytes {
            b"GET" => Some(Self::Get),
            b"POST" => Some(Self::Post),
            b"PUT" => Some(Self::Put),
            b"DELETE" => Some(Self::Delete),
            b"HEAD" => Some(Self::Head),
            b"OPTIONS" => Some(Self::Options),
            b"CONNECT" => Some(Self::Connect),
            b"PATCH" => Some(Self::Patch),
            b"TRACE" => Some(Self::Trace),
            _ => None,
        }
    }
}

/// HTTP version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpVersion {
    Http10,
    Http11,
    Http2,
}

impl HttpVersion {
    pub fn as_bytes(&self) -> &'static [u8] {
        match self {
            Self::Http10 => b"HTTP/1.0",
            Self::Http11 => b"HTTP/1.1",
            Self::Http2 => b"HTTP/2",
        }
    }
}

/// HTTP header
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpHeader<'a> {
    pub name: &'a [u8],
    pub value: &'a [u8],
}

/// Complete HTTP request
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest<'a> {
    pub method: HttpMethod,
    pub path: &'a [u8],
    pub version: HttpVersion,
    pub headers: Vec<HttpHeader<'a>>,
}

/// SOCKS5 authentication methods
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Socks5AuthMethod {
    NoAuth = 0x00,
    GssApi = 0x01,
    UserPass = 0x02,
    NoAcceptable = 0xFF,
}

/// SOCKS5 handshake request
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5Handshake {
    pub version: u8,
    pub methods: Vec<Socks5AuthMethod>,
}

/// SOCKS5 connect request
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5Connect<'a> {
    pub version: u8,
    pub command: u8,
    pub address_type: u8,
    pub address: &'a [u8],
    pub port: u16,
}

/// Protocol parser factory
pub struct ProtocolParsers;

impl ProtocolParsers {
    pub fn new() -> Self {
        Self
    }
    
    pub fn http_parser(&self) -> HttpParser {
        HttpParser::new()
    }
    
    pub fn socks5_parser(&self) -> Socks5Parser {
        Socks5Parser::new()
    }
    
    pub fn json_parser(&self) -> JsonParser {
        JsonParser::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_method_conversions() {
        assert_eq!(HttpMethod::Get.as_bytes(), b"GET");
        assert_eq!(HttpMethod::from_bytes(b"POST"), Some(HttpMethod::Post));
        assert_eq!(HttpMethod::from_bytes(b"INVALID"), None);
    }

    #[test]
    fn test_protocol_parsers_creation() {
        let parsers = ProtocolParsers::new(ScanStrategy::Autovec);
        
        let http_parser = parsers.http_parser();
        let socks5_parser = parsers.socks5_parser();
        let json_parser = parsers.json_parser();
        
        // Just verify they can be created
        assert_eq!(http_parser.strategy(), ScanStrategy::Autovec);
        assert_eq!(socks5_parser.strategy(), ScanStrategy::Autovec);
        assert_eq!(json_parser.strategy(), ScanStrategy::Autovec);
    }
}