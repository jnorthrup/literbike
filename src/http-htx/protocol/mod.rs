//! HTTP-HTX Protocol - HTX internal message representation
//!
//! This module CANNOT see matcher, listener, reactor, timer, handler.
//! It only knows about itself.
//!
//! HTX (HTTP Transfer) is an internal message representation that normalizes
//! all HTTP versions (HTTP/1, HTTP/2, HTTP/3) to a common structured format.
//!
//! Design follows HAProxy's HTX implementation:
//! - Block metadata stored at END of blocks array
//! - Block payloads stored at BEGINNING of blocks array  
//! - Creates circular/ring buffer arrangement
//! - Free space is between payloads end and blocks start

/// HTX block types (matches HAProxy encoding)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HtxBlockType {
    ReqSl = 0,   // Request start-line
    ResSl = 1,   // Response start-line
    Hdr = 2,     // Header name/value
    Eoh = 3,     // End-of-headers
    Data = 4,    // Data block
    Tlr = 5,     // Trailer name/value
    Eot = 6,     // End-of-trailers
    Unused = 15, // Unused/removed block
}

/// HTX start-line flags (matches HAProxy)
#[derive(Debug, Clone, Copy, Default)]
#[repr(transparent)]
pub struct HtxSlFlags(pub u32);

impl HtxSlFlags {
    pub const IS_RESP: u32 = 0x00000001;
    pub const XFER_LEN: u32 = 0x00000002;
    pub const XFER_ENC: u32 = 0x00000004;
    pub const CLEN: u32 = 0x00000008;
    pub const CHNK: u32 = 0x00000010;
    pub const VER_11: u32 = 0x00000020;
    pub const BODYLESS: u32 = 0x00000040;
    pub const HAS_SCHM: u32 = 0x00000080;
    pub const SCHM_HTTP: u32 = 0x00000100;
    pub const SCHM_HTTPS: u32 = 0x00000200;
    pub const HAS_AUTHORITY: u32 = 0x00000400;
    pub const NORMALIZED_URI: u32 = 0x00000800;
    pub const CONN_UPG: u32 = 0x00001000;
    pub const BODYLESS_RESP: u32 = 0x00002000;
    pub const NOT_HTTP: u32 = 0x00004000;
}

/// HTX message flags
#[derive(Debug, Clone, Copy, Default)]
#[repr(transparent)]
pub struct HtxFlags(pub u32);

impl HtxFlags {
    pub const NONE: u32 = 0x00000000;
    pub const PARSING_ERROR: u32 = 0x00000001;
    pub const PROCESSING_ERROR: u32 = 0x00000002;
    pub const FRAGMENTED: u32 = 0x00000004;
    pub const UNORDERED: u32 = 0x00000008;
    pub const EOM: u32 = 0x00000010; // End of message
}

/// HTX block metadata (matches HAProxy struct htx_blk)
/// - addr: relative storage address of payload
/// - info: type (4 bits) + value length + name length
#[derive(Debug, Clone)]
pub struct HtxBlock {
    pub addr: u32, // Relative address of payload
    pub info: u32, // Block info: type(4) | value_len(28)
}

impl HtxBlock {
    /// Create a new block with type and sizes
    pub fn new(block_type: HtxBlockType, name_len: u32, value_len: u32, addr: u32) -> Self {
        let info = (block_type as u32) << 28 | (value_len << 8) | name_len;
        Self { addr, info }
    }

    /// Get the block type
    pub fn block_type(&self) -> HtxBlockType {
        let t = (self.info >> 28) as u8;
        match t {
            0 => HtxBlockType::ReqSl,
            1 => HtxBlockType::ResSl,
            2 => HtxBlockType::Hdr,
            3 => HtxBlockType::Eoh,
            4 => HtxBlockType::Data,
            5 => HtxBlockType::Tlr,
            6 => HtxBlockType::Eot,
            15 => HtxBlockType::Unused,
            _ => HtxBlockType::Unused,
        }
    }

    /// Get the value length
    pub fn value_len(&self) -> u32 {
        self.info & 0x0fffffff
    }

    /// Get the name length (for HDR/TLR blocks)
    pub fn name_len(&self) -> u32 {
        (self.info >> 8) & 0xff
    }
}

/// HTTP method (for requests)
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
    Unknown,
}

impl HttpMethod {
    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        match b {
            b"GET" => Some(HttpMethod::Get),
            b"POST" => Some(HttpMethod::Post),
            b"PUT" => Some(HttpMethod::Put),
            b"DELETE" => Some(HttpMethod::Delete),
            b"HEAD" => Some(HttpMethod::Head),
            b"OPTIONS" => Some(HttpMethod::Options),
            b"CONNECT" => Some(HttpMethod::Connect),
            b"PATCH" => Some(HttpMethod::Patch),
            b"TRACE" => Some(HttpMethod::Trace),
            _ => None,
        }
    }

    pub fn to_bytes(&self) -> &'static [u8] {
        match self {
            HttpMethod::Get => b"GET",
            HttpMethod::Post => b"POST",
            HttpMethod::Put => b"PUT",
            HttpMethod::Delete => b"DELETE",
            HttpMethod::Head => b"HEAD",
            HttpMethod::Options => b"OPTIONS",
            HttpMethod::Connect => b"CONNECT",
            HttpMethod::Patch => b"PATCH",
            HttpMethod::Trace => b"TRACE",
            HttpMethod::Unknown => b"",
        }
    }
}

/// HTTP status codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpStatus {
    Continue = 100,
    Ok = 200,
    Created = 201,
    Accepted = 202,
    NoContent = 204,
    MovedPermanently = 301,
    Found = 302,
    SeeOther = 303,
    NotModified = 304,
    TemporaryRedirect = 307,
    PermanentRedirect = 308,
    BadRequest = 400,
    Unauthorized = 401,
    Forbidden = 403,
    NotFound = 404,
    MethodNotAllowed = 405,
    RequestTimeout = 408,
    PayloadTooLarge = 413,
    UnsupportedMediaType = 415,
    InternalServerError = 500,
    NotImplemented = 501,
    BadGateway = 502,
    ServiceUnavailable = 503,
    GatewayTimeout = 504,
}

/// HTX start-line (matches HAProxy struct htx_sl)
/// For requests: method, uri, version
/// For responses: version, status, reason
#[derive(Debug, Clone)]
pub struct HtxStartLine {
    pub flags: HtxSlFlags,
    pub meth: Option<HttpMethod>, // Request method
    pub status: Option<u16>,      // Response status
    pub uri: Vec<u8>,             // Request URI
    pub version: (u8, u8),        // (major, minor)
    pub reason: Vec<u8>,          // Response reason phrase
}

impl HtxStartLine {
    pub fn new_request(method: HttpMethod, uri: &[u8], major: u8, minor: u8) -> Self {
        Self {
            flags: HtxSlFlags(0),
            meth: Some(method),
            status: None,
            uri: uri.to_vec(),
            version: (major, minor),
            reason: Vec::new(),
        }
    }

    pub fn new_response(status: u16, reason: &[u8], major: u8, minor: u8) -> Self {
        Self {
            flags: HtxSlFlags(HtxSlFlags::IS_RESP | HtxSlFlags::VER_11),
            meth: None,
            status: Some(status),
            uri: Vec::new(),
            version: (major, minor),
            reason: reason.to_vec(),
        }
    }

    pub fn is_request(&self) -> bool {
        self.meth.is_some()
    }
}

/// HTX block - a single element of an HTX message
#[derive(Debug, Clone)]
pub enum HtxBlockData {
    StartLine(HtxStartLine),
    Header { name: Vec<u8>, value: Vec<u8> },
    Data(Vec<u8>),
    Trailer { name: Vec<u8>, value: Vec<u8> },
    EndHeaders,
    EndTrailers,
}

impl HtxBlockData {
    pub fn block_type(&self) -> HtxBlockType {
        match self {
            HtxBlockData::StartLine(sl) if sl.is_request() => HtxBlockType::ReqSl,
            HtxBlockData::StartLine(_) => HtxBlockType::ResSl,
            HtxBlockData::Header { .. } => HtxBlockType::Hdr,
            HtxBlockData::Data(_) => HtxBlockType::Data,
            HtxBlockData::Trailer { .. } => HtxBlockType::Tlr,
            HtxBlockData::EndHeaders => HtxBlockType::Eoh,
            HtxBlockData::EndTrailers => HtxBlockType::Eot,
        }
    }
}

/// HTX message - complete HTTP message in internal format
#[derive(Debug, Clone, Default)]
pub struct HtxMessage {
    pub blocks: Vec<HtxBlockData>,
    pub flags: HtxFlags,
}

impl HtxMessage {
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            flags: HtxFlags(HtxFlags::NONE),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn add_start_line(&mut self, sl: HtxStartLine) {
        self.blocks.push(HtxBlockData::StartLine(sl));
    }

    pub fn add_header(&mut self, name: &[u8], value: &[u8]) {
        self.blocks.push(HtxBlockData::Header {
            name: name.to_vec(),
            value: value.to_vec(),
        });
    }

    pub fn add_data(&mut self, data: &[u8]) {
        self.blocks.push(HtxBlockData::Data(data.to_vec()));
    }

    pub fn add_trailer(&mut self, name: &[u8], value: &[u8]) {
        self.blocks.push(HtxBlockData::Trailer {
            name: name.to_vec(),
            value: value.to_vec(),
        });
    }

    pub fn add_end_headers(&mut self) {
        self.blocks.push(HtxBlockData::EndHeaders);
    }

    pub fn add_end_trailers(&mut self) {
        self.blocks.push(HtxBlockData::EndTrailers);
    }

    pub fn set_eom(&mut self) {
        self.flags = HtxFlags(HtxFlags::EOM);
    }

    pub fn start_line(&self) -> Option<&HtxStartLine> {
        for blk in &self.blocks {
            if let HtxBlockData::StartLine(sl) = blk {
                return Some(sl);
            }
        }
        None
    }

    pub fn headers(&self) -> impl Iterator<Item = (&[u8], &[u8])> {
        self.blocks.iter().filter_map(|b| {
            if let HtxBlockData::Header { name, value } = b {
                Some((name.as_slice(), value.as_slice()))
            } else {
                None
            }
        })
    }
}

/// HtxKey - Root of HTTP-HTX hierarchy
pub struct HtxKey;

impl HtxKey {
    pub const FACTORY: fn() -> HtxElement = HtxElement::new;
}

/// HtxElement - HTTP-HTX operational state
/// 
/// Tracks HTTP parsing metrics across all versions (HTTP/1, HTTP/2, HTTP/3)
pub struct HtxElement {
    pub version: u32,
    http1_count: std::sync::atomic::AtomicU64,
    http2_count: std::sync::atomic::AtomicU64,
    http3_count: std::sync::atomic::AtomicU64,
    bytes_parsed: std::sync::atomic::AtomicU64,
    errors_count: std::sync::atomic::AtomicU64,
    active_requests: std::sync::atomic::AtomicU32,
}

impl HtxElement {
    pub fn new() -> Self {
        Self {
            version: 1,
            http1_count: std::sync::atomic::AtomicU64::new(0),
            http2_count: std::sync::atomic::AtomicU64::new(0),
            http3_count: std::sync::atomic::AtomicU64::new(0),
            bytes_parsed: std::sync::atomic::AtomicU64::new(0),
            errors_count: std::sync::atomic::AtomicU64::new(0),
            active_requests: std::sync::atomic::AtomicU32::new(0),
        }
    }

    /// Record HTTP/1.x message parsed
    pub fn record_http1(&self, bytes: u64) {
        self.http1_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.bytes_parsed.fetch_add(bytes, std::sync::atomic::Ordering::Relaxed);
    }

    /// Record HTTP/2 message parsed
    pub fn record_http2(&self, bytes: u64) {
        self.http2_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.bytes_parsed.fetch_add(bytes, std::sync::atomic::Ordering::Relaxed);
    }

    /// Record HTTP/3 message parsed
    pub fn record_http3(&self, bytes: u64) {
        self.http3_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.bytes_parsed.fetch_add(bytes, std::sync::atomic::Ordering::Relaxed);
    }

    /// Record parse error
    pub fn record_error(&self) {
        self.errors_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Increment active requests
    pub fn request_start(&self) {
        self.active_requests.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Decrement active requests
    pub fn request_end(&self) {
        self.active_requests.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Get HTTP/1.x count
    pub fn http1_count(&self) -> u64 {
        self.http1_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get HTTP/2 count
    pub fn http2_count(&self) -> u64 {
        self.http2_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get HTTP/3 count
    pub fn http3_count(&self) -> u64 {
        self.http3_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get total bytes parsed
    pub fn bytes_parsed(&self) -> u64 {
        self.bytes_parsed.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get error count
    pub fn errors(&self) -> u64 {
        self.errors_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get active request count
    pub fn active_requests(&self) -> u32 {
        self.active_requests.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get total messages parsed
    pub fn total_messages(&self) -> u64 {
        self.http1_count() + self.http2_count() + self.http3_count()
    }
}

impl std::fmt::Debug for HtxElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HtxElement")
            .field("version", &self.version)
            .field("http1", &self.http1_count())
            .field("http2", &self.http2_count())
            .field("http3", &self.http3_count())
            .field("bytes_parsed", &self.bytes_parsed())
            .field("errors", &self.errors())
            .field("active_requests", &self.active_requests())
            .finish()
    }
}

impl Clone for HtxElement {
    fn clone(&self) -> Self {
        Self {
            version: self.version,
            http1_count: std::sync::atomic::AtomicU64::new(self.http1_count()),
            http2_count: std::sync::atomic::AtomicU64::new(self.http2_count()),
            http3_count: std::sync::atomic::AtomicU64::new(self.http3_count()),
            bytes_parsed: std::sync::atomic::AtomicU64::new(self.bytes_parsed()),
            errors_count: std::sync::atomic::AtomicU64::new(self.errors()),
            active_requests: std::sync::atomic::AtomicU32::new(self.active_requests()),
        }
    }
}

// CCEK integration - implement Key/Element traits when ccek feature is enabled
#[cfg(feature = "ccek")]
impl ccek_core::Key for HtxKey {
    type Element = HtxElement;
    const FACTORY: fn() -> Self::Element = HtxElement::new;
}

#[cfg(feature = "ccek")]
impl ccek_core::Element for HtxElement {
    fn key_type(&self) -> std::any::TypeId {
        std::any::TypeId::of::<HtxKey>()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ============================================================================
// HTTP/1 Parser
// ============================================================================

/// Parse HTTP/1.x text format into HTX blocks
pub fn parse_http1(input: &[u8]) -> Option<HtxMessage> {
    let input_str = std::str::from_utf8(input).ok()?;

    let mut msg = HtxMessage::new();
    let mut state = ParseState::RequestLine;
    let _current_header_name: Option<Vec<u8>> = None;

    for line in input_str.lines() {
        let line = line.trim_end_matches('\r');

        match state {
            ParseState::RequestLine => {
                // Parse request or status line
                if let Some((method, uri, version)) = parse_request_line(line) {
                    msg.add_start_line(HtxStartLine::new_request(
                        method,
                        uri.as_bytes(),
                        version.0,
                        version.1,
                    ));
                    state = ParseState::Headers;
                } else if let Some((status, reason, version)) = parse_status_line(line) {
                    msg.add_start_line(HtxStartLine::new_response(
                        status,
                        reason.as_bytes(),
                        version.0,
                        version.1,
                    ));
                    state = ParseState::Headers;
                } else {
                    return None;
                }
            }
            ParseState::Headers => {
                if line.is_empty() {
                    msg.add_end_headers();
                    state = ParseState::Body;
                    continue;
                }

                if let Some((name, value)) = parse_header(line) {
                    msg.add_header(name.as_bytes(), value.as_bytes());
                }
            }
            ParseState::Body => {
                // Remaining lines are body data
                if !line.is_empty() {
                    msg.add_data(line.as_bytes());
                }
            }
        }
    }

    msg.set_eom();
    Some(msg)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ParseState {
    RequestLine,
    Headers,
    Body,
}

fn parse_request_line(line: &str) -> Option<(HttpMethod, &str, (u8, u8))> {
    let mut parts = line.splitn(3, ' ');
    let method_str = parts.next()?;
    let uri = parts.next()?;
    let version_str = parts.next()?;

    let method = HttpMethod::from_bytes(method_str.as_bytes())?;
    let version = parse_version(version_str)?;

    Some((method, uri, version))
}

fn parse_status_line(line: &str) -> Option<(u16, &str, (u8, u8))> {
    let mut parts = line.splitn(3, ' ');
    let version_str = parts.next()?;
    let status_str = parts.next()?;
    let reason = parts.next().unwrap_or("");

    if !version_str.starts_with("HTTP/") {
        return None;
    }

    let status: u16 = status_str.parse().ok()?;
    let version = parse_version(version_str)?;

    Some((status, reason, version))
}

fn parse_version(s: &str) -> Option<(u8, u8)> {
    if !s.starts_with("HTTP/") {
        return None;
    }
    let rest = &s[5..];
    let mut parts = rest.splitn(2, '.');
    let major: u8 = parts.next()?.parse().ok()?;
    let minor: u8 = parts.next()?.parse().ok()?;
    Some((major, minor))
}

fn parse_header(line: &str) -> Option<(String, String)> {
    let colon_pos = line.find(':')?;
    let name = line[..colon_pos].trim().to_string();
    let value = line[colon_pos + 1..].trim().to_string();
    Some((name, value))
}

/// Normalize input bytes to HTX representation
pub fn normalize_to_htx(input: &[u8], _protocol_hint: &[u8]) -> HtxMessage {
    // Check for HTTP/1.x by looking for HTTP/ prefix or methods
    if let Ok(text) = std::str::from_utf8(&input[..input.len().min(1024)]) {
        if text.starts_with("HTTP/") {
            if let Some(msg) = parse_http1(input) {
                return msg;
            }
        }

        // Check if starts with a known HTTP method
        let methods = [
            "GET ", "POST ", "PUT ", "DELETE ", "HEAD ", "OPTIONS ", "PATCH ", "CONNECT ", "TRACE ",
        ];
        for method in methods {
            if text.starts_with(method) {
                if let Some(msg) = parse_http1(input) {
                    return msg;
                }
                break;
            }
        }
    }

    // HTTP/2 detection (starts with connection preface or frames)
    if input.len() >= 24 && &input[0..24] == b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n" {
        // TODO: Implement HTTP/2 frame parsing
    }

    HtxMessage::new()
}

/// Parse bytes into HTX message (convenience wrapper)
pub fn parse_htx(input: &[u8]) -> Option<HtxMessage> {
    parse_http1(input)
}

// Aliases for backward compatibility (exported for convenience)
#[allow(unused_imports)]
pub use HtxBlockData as Block;
#[allow(unused_imports)]
pub use HtxBlockType as BlockType;
#[allow(unused_imports)]
pub use HtxMessage as Message;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_http1_get() {
        let input = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let msg = parse_http1(input).unwrap();
        assert!(!msg.blocks.is_empty());

        let sl = msg.start_line().unwrap();
        assert!(sl.is_request());
        assert_eq!(sl.meth, Some(HttpMethod::Get));
    }

    #[test]
    fn test_parse_http1_post() {
        let input = b"POST /api HTTP/1.0\r\nContent-Length: 5\r\n\r\nhello";
        let msg = parse_http1(input).unwrap();
        assert!(!msg.blocks.is_empty());

        let sl = msg.start_line().unwrap();
        assert_eq!(sl.meth, Some(HttpMethod::Post));
    }

    #[test]
    fn test_parse_http1_response() {
        let input = b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n";
        let msg = parse_http1(input).unwrap();

        let sl = msg.start_line().unwrap();
        assert!(!sl.is_request());
        assert_eq!(sl.status, Some(200));
    }

    #[test]
    fn test_http_method_parsing() {
        assert_eq!(HttpMethod::from_bytes(b"GET"), Some(HttpMethod::Get));
        assert_eq!(HttpMethod::from_bytes(b"POST"), Some(HttpMethod::Post));
        assert_eq!(HttpMethod::from_bytes(b"INVALID"), None);
    }

    #[test]
    fn test_normalize_to_htx() {
        let input = b"GET /test HTTP/1.1\r\n\r\n";
        let msg = normalize_to_htx(input, b"");
        assert!(!msg.blocks.is_empty());
    }

    #[test]
    fn test_htx_message_build() {
        let mut msg = HtxMessage::new();
        msg.add_start_line(HtxStartLine::new_request(HttpMethod::Get, b"/", 1, 1));
        msg.add_header(b"Host", b"example.com");
        msg.add_end_headers();
        msg.add_data(b"Hello");

        assert_eq!(msg.len(), 4);
        assert!(msg.start_line().is_some());

        let headers: Vec<_> = msg.headers().collect();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0], (b"Host" as &[u8], b"example.com" as &[u8]));
    }
}
