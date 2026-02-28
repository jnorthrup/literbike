//! HTTP/1.1 Header Parser - relaxfactory Rfc822HeaderState pattern
//!
//! Zero-copy header parsing from ByteBuffer-like buffer
//! Parses only what's necessary, defers expensive string operations

use std::collections::HashMap;
use std::str;

/// HTTP/1.1 constants
pub const HTTP_1_1: &str = "HTTP/1.1";
pub const HTTP_1_0: &str = "HTTP/1.0";
pub const CRLF: &[u8] = b"\r\n";
pub const CR: u8 = b'\r';
pub const LF: u8 = b'\n';
pub const COLON: u8 = b':';
pub const SPACE: u8 = b' ';

/// HTTP methods
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    HEAD,
    OPTIONS,
    PATCH,
    UNKNOWN,
}

impl HttpMethod {
    pub fn from_bytes(s: &[u8]) -> Self {
        match s {
            b"GET" => HttpMethod::GET,
            b"POST" => HttpMethod::POST,
            b"PUT" => HttpMethod::PUT,
            b"DELETE" => HttpMethod::DELETE,
            b"HEAD" => HttpMethod::HEAD,
            b"OPTIONS" => HttpMethod::OPTIONS,
            b"PATCH" => HttpMethod::PATCH,
            _ => HttpMethod::UNKNOWN,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::GET => "GET",
            HttpMethod::POST => "POST",
            HttpMethod::PUT => "PUT",
            HttpMethod::DELETE => "DELETE",
            HttpMethod::HEAD => "HEAD",
            HttpMethod::OPTIONS => "OPTIONS",
            HttpMethod::PATCH => "PATCH",
            HttpMethod::UNKNOWN => "UNKNOWN",
        }
    }
}

/// HTTP status codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum HttpStatus {
    Status200 = 200,
    Status201 = 201,
    Status204 = 204,
    Status301 = 301,
    Status302 = 302,
    Status304 = 304,
    Status400 = 400,
    Status401 = 401,
    Status403 = 403,
    Status404 = 404,
    Status405 = 405,
    Status500 = 500,
    Status502 = 502,
    Status503 = 503,
}

impl HttpStatus {
    pub fn from_u16(code: u16) -> Self {
        match code {
            200 => HttpStatus::Status200,
            201 => HttpStatus::Status201,
            204 => HttpStatus::Status204,
            301 => HttpStatus::Status301,
            302 => HttpStatus::Status302,
            304 => HttpStatus::Status304,
            400 => HttpStatus::Status400,
            401 => HttpStatus::Status401,
            403 => HttpStatus::Status403,
            404 => HttpStatus::Status404,
            405 => HttpStatus::Status405,
            500 => HttpStatus::Status500,
            502 => HttpStatus::Status502,
            503 => HttpStatus::Status503,
            _ => HttpStatus::Status500,
        }
    }

    pub fn as_u16(&self) -> u16 {
        *self as u16
    }

    pub fn reason_phrase(&self) -> &'static str {
        match self {
            HttpStatus::Status200 => "OK",
            HttpStatus::Status201 => "Created",
            HttpStatus::Status204 => "No Content",
            HttpStatus::Status301 => "Moved Permanently",
            HttpStatus::Status302 => "Found",
            HttpStatus::Status304 => "Not Modified",
            HttpStatus::Status400 => "Bad Request",
            HttpStatus::Status401 => "Unauthorized",
            HttpStatus::Status403 => "Forbidden",
            HttpStatus::Status404 => "Not Found",
            HttpStatus::Status405 => "Method Not Allowed",
            HttpStatus::Status500 => "Internal Server Error",
            HttpStatus::Status502 => "Bad Gateway",
            HttpStatus::Status503 => "Service Unavailable",
        }
    }
}

/// Common HTTP headers (like one.xio.HttpHeaders)
pub mod headers {
    pub const CONTENT_LENGTH: &str = "Content-Length";
    pub const CONTENT_TYPE: &str = "Content-Type";
    pub const TRANSFER_ENCODING: &str = "Transfer-Encoding";
    pub const CONNECTION: &str = "Connection";
    pub const HOST: &str = "Host";
    pub const COOKIE: &str = "Cookie";
    pub const LOCATION: &str = "Location";
    pub const AUTHORIZATION: &str = "Authorization";
    pub const USER_AGENT: &str = "User-Agent";
    pub const ACCEPT: &str = "Accept";
    pub const ACCEPT_ENCODING: &str = "Accept-Encoding";
}

/// MIME types (like one.xio.MimeType)
pub mod mime {
    pub const TEXT_HTML: &str = "text/html";
    pub const TEXT_PLAIN: &str = "text/plain";
    pub const TEXT_CSS: &str = "text/css";
    pub const TEXT_JAVASCRIPT: &str = "text/javascript";
    pub const APPLICATION_JSON: &str = "application/json";
    pub const APPLICATION_OCTET_STREAM: &str = "application/octet-stream";
}

/// Header parser state - holds buffer and parsed state
/// Like Rfc822HeaderState, parses lazily and caches results
pub struct HeaderParser {
    /// Raw header buffer (like Rfc822HeaderState.headerBuf)
    buffer: Vec<u8>,
    
    /// Parsed headers (lazy, populated on demand)
    headers: HashMap<String, String>,
    
    /// Request line components
    method: Option<HttpMethod>,
    path: Option<String>,
    protocol: Option<String>,
    
    /// Response line components
    status: Option<HttpStatus>,
    status_text: Option<String>,
    
    /// Parse state
    header_complete: bool,
    content_length: Option<usize>,
}

impl HeaderParser {
    /// Create new empty header parser
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(512),
            headers: HashMap::new(),
            method: None,
            path: None,
            protocol: None,
            status: None,
            status_text: None,
            header_complete: false,
            content_length: None,
        }
    }

    /// Create with pre-allocated buffer capacity
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(cap),
            headers: HashMap::new(),
            method: None,
            path: None,
            protocol: None,
            status: None,
            status_text: None,
            header_complete: false,
            content_length: None,
        }
    }

    /// Clear parser for reuse (like Rfc822HeaderState.clear)
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.headers.clear();
        self.method = None;
        self.path = None;
        self.protocol = None;
        self.status = None;
        self.status_text = None;
        self.header_complete = false;
        self.content_length = None;
    }

    /// Get mutable reference to header buffer for reading
    pub fn buffer_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buffer
    }

    /// Get header buffer (like Rfc822HeaderState.headerBuf())
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    /// Set buffer (for buffer swapping like relaxfactory)
    pub fn set_buffer(&mut self, buf: Vec<u8>) {
        self.buffer = buf;
        self.header_complete = false;
        self.headers.clear();
        self.method = None;
        self.path = None;
        self.protocol = None;
    }

    /// Append data to buffer
    pub fn append(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
        self.header_complete = false;
    }

    /// Check if headers are complete (ends with \r\n\r\n)
    pub fn headers_complete(&self) -> bool {
        if self.header_complete {
            return true;
        }
        // Check for \r\n\r\n
        if self.buffer.len() >= 4 {
            let len = self.buffer.len();
            self.buffer[len-4..] == [CR, LF, CR, LF]
        } else {
            false
        }
    }

    /// Parse headers if complete (like Tx.readHttpHeaders)
    pub fn parse(&mut self) -> Result<bool, HttpParseError> {
        if self.header_complete {
            return Ok(true);
        }

        // Find header terminator
        let terminator = [CR, LF, CR, LF];
        if let Some(pos) = self.buffer.windows(4).position(|w| w == &terminator[..]) {
            // Parse request/status line
            let header_end = pos;

            // Find first line ending and copy data to avoid borrow conflicts
            if let Some(first_line_end) = self.buffer[..header_end].windows(2).position(|w| w == &CRLF[..]) {
                // Copy first line data before calling mutable methods
                let first_line_data = self.buffer[..first_line_end].to_vec();
                let first_line = first_line_data.as_slice();

                // Check if this is a request (starts with method) or response (starts with HTTP)
                if first_line.starts_with(b"HTTP/") {
                    self.parse_status_line(first_line)?;
                } else {
                    self.parse_request_line(first_line)?;
                }

                // Copy header lines data before parsing
                let header_start = first_line_end + 2;
                let header_lines_data = self.buffer[header_start..header_end].to_vec();

                for line in header_lines_data.split(|b| *b == LF) {
                    if line.is_empty() || line == &[CR] {
                        continue;
                    }

                    // Remove leading/trailing CR
                    let line = if line.starts_with(&[CR]) { &line[1..] } else { line };
                    let line = if line.ends_with(&[CR]) { &line[..line.len()-1] } else { line };

                    if let Some(colon_pos) = line.iter().position(|&b| b == COLON) {
                        let name = String::from_utf8_lossy(&line[..colon_pos]).trim().to_string();
                        let value = String::from_utf8_lossy(&line[colon_pos+1..]).trim().to_string();

                        // Track Content-Length specially
                        if name.eq_ignore_ascii_case(headers::CONTENT_LENGTH) {
                            self.content_length = value.parse().ok();
                        }

                        self.headers.insert(name, value);
                    }
                }
            }

            self.header_complete = true;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Parse request line: "GET /path HTTP/1.1"
    fn parse_request_line(&mut self, line: &[u8]) -> Result<(), HttpParseError> {
        let parts: Vec<&[u8]> = line.split(|&b| b == SPACE).collect();
        if parts.len() >= 2 {
            self.method = Some(HttpMethod::from_bytes(parts[0]));
            self.path = Some(String::from_utf8_lossy(parts[1]).to_string());
            if parts.len() >= 3 {
                self.protocol = Some(String::from_utf8_lossy(parts[2]).to_string());
            }
        }
        Ok(())
    }

    /// Parse status line: "HTTP/1.1 200 OK"
    fn parse_status_line(&mut self, line: &[u8]) -> Result<(), HttpParseError> {
        let parts: Vec<&[u8]> = line.split(|&b| b == SPACE).collect();
        if parts.len() >= 2 {
            self.protocol = Some(String::from_utf8_lossy(parts[0]).to_string());
            if let Ok(code) = str::from_utf8(parts[1]).unwrap_or("").parse::<u16>() {
                self.status = Some(HttpStatus::from_u16(code));
            }
            if parts.len() >= 3 {
                self.status_text = Some(String::from_utf8_lossy(parts[2]).to_string());
            }
        }
        Ok(())
    }

    /// Get HTTP method (request only)
    pub fn method(&self) -> Option<HttpMethod> {
        self.method
    }

    /// Get request path
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// Get protocol version
    pub fn protocol(&self) -> Option<&str> {
        self.protocol.as_deref()
    }

    /// Get response status
    pub fn status(&self) -> Option<HttpStatus> {
        self.status
    }

    /// Set response status (for building responses)
    pub fn set_status(&mut self, status: HttpStatus) {
        self.status = Some(status);
    }

    /// Get header value
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(|s| s.as_str())
    }

    /// Get all headers
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    /// Set header (for building responses)
    pub fn set_header(&mut self, name: &str, value: &str) {
        self.headers.insert(name.to_string(), value.to_string());
    }

    /// Get Content-Length
    pub fn content_length(&self) -> Option<usize> {
        self.content_length
    }

    /// Get body start position (after headers)
    pub fn body_offset(&self) -> Option<usize> {
        if !self.header_complete {
            return None;
        }
        
        let terminator = [CR, LF, CR, LF];
        self.buffer.windows(4).position(|w| *w == terminator)
            .map(|pos| pos + 4)
    }

    /// Build response header bytes (like Rfc822HeaderState.HttpResponse.asByteBuffer)
    pub fn build_response(&self, body_len: usize) -> Vec<u8> {
        let mut buf = Vec::with_capacity(256);
        
        // Status line
        let status = self.status.unwrap_or(HttpStatus::Status200);
        buf.extend_from_slice(b"HTTP/1.1 ");
        buf.extend_from_slice(status.as_u16().to_string().as_bytes());
        buf.extend_from_slice(b" ");
        buf.extend_from_slice(status.reason_phrase().as_bytes());
        buf.extend_from_slice(CRLF);
        
        // Headers
        buf.extend_from_slice(headers::CONTENT_LENGTH.as_bytes());
        buf.extend_from_slice(b": ");
        buf.extend_from_slice(body_len.to_string().as_bytes());
        buf.extend_from_slice(CRLF);
        
        for (name, value) in &self.headers {
            if name.eq_ignore_ascii_case(headers::CONTENT_LENGTH) {
                continue; // Already set
            }
            buf.extend_from_slice(name.as_bytes());
            buf.extend_from_slice(b": ");
            buf.extend_from_slice(value.as_bytes());
            buf.extend_from_slice(CRLF);
        }
        
        // Empty line
        buf.extend_from_slice(CRLF);
        
        buf
    }

    /// Build simple response (status + content-type + body)
    pub fn build_simple_response(&self, status: HttpStatus, content_type: &str, body: &[u8]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(256 + body.len());
        
        // Status line
        buf.extend_from_slice(b"HTTP/1.1 ");
        buf.extend_from_slice(status.as_u16().to_string().as_bytes());
        buf.extend_from_slice(b" ");
        buf.extend_from_slice(status.reason_phrase().as_bytes());
        buf.extend_from_slice(CRLF);
        
        // Headers
        buf.extend_from_slice(headers::CONTENT_TYPE.as_bytes());
        buf.extend_from_slice(b": ");
        buf.extend_from_slice(content_type.as_bytes());
        buf.extend_from_slice(CRLF);
        
        buf.extend_from_slice(headers::CONTENT_LENGTH.as_bytes());
        buf.extend_from_slice(b": ");
        buf.extend_from_slice(body.len().to_string().as_bytes());
        buf.extend_from_slice(CRLF);
        
        buf.extend_from_slice(b"Connection: close\r\n");
        buf.extend_from_slice(CRLF);
        
        // Body
        buf.extend_from_slice(body);
        
        buf
    }
}

impl Default for HeaderParser {
    fn default() -> Self {
        Self::new()
    }
}

/// HTTP parse error
#[derive(Debug, Clone)]
pub enum HttpParseError {
    InvalidRequestLine,
    InvalidStatusLine,
    InvalidHeader,
    BufferOverflow,
}

impl std::fmt::Display for HttpParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpParseError::InvalidRequestLine => write!(f, "Invalid request line"),
            HttpParseError::InvalidStatusLine => write!(f, "Invalid status line"),
            HttpParseError::InvalidHeader => write!(f, "Invalid header"),
            HttpParseError::BufferOverflow => write!(f, "Buffer overflow"),
        }
    }
}

impl std::error::Error for HttpParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_get_request() {
        let mut parser = HeaderParser::new();
        let request = b"GET /path?query=1 HTTP/1.1\r\nHost: example.com\r\nAccept: */*\r\n\r\n";
        parser.append(request);
        
        assert!(parser.parse().unwrap());
        assert_eq!(parser.method(), Some(HttpMethod::GET));
        assert_eq!(parser.path(), Some("/path?query=1"));
        assert_eq!(parser.protocol(), Some("HTTP/1.1"));
        assert_eq!(parser.header("Host"), Some("example.com"));
        assert_eq!(parser.header("Accept"), Some("*/*"));
    }

    #[test]
    fn test_parse_response() {
        let mut parser = HeaderParser::new();
        let response = b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 13\r\n\r\nHello, World!";
        parser.append(response);
        
        assert!(parser.parse().unwrap());
        assert_eq!(parser.status(), Some(HttpStatus::Status200));
        assert_eq!(parser.header("Content-Type"), Some("text/html"));
        assert_eq!(parser.content_length(), Some(13));
        assert_eq!(parser.body_offset(), Some(response.len() - 13));
    }

    #[test]
    fn test_build_response() {
        let mut parser = HeaderParser::new();
        parser.set_status(HttpStatus::Status200);
        parser.set_header("Content-Type", "text/plain");
        
        let body = b"Hello";
        let response = parser.build_simple_response(HttpStatus::Status200, "text/plain", body);
        let response_str = String::from_utf8_lossy(&response);
        
        assert!(response_str.starts_with("HTTP/1.1 200 OK"));
        assert!(response_str.contains("Content-Type: text/plain"));
        assert!(response_str.contains("Content-Length: 5"));
        assert!(response_str.ends_with("Hello"));
    }

    #[test]
    fn test_http_method_from_bytes() {
        assert_eq!(HttpMethod::from_bytes(b"GET"), HttpMethod::GET);
        assert_eq!(HttpMethod::from_bytes(b"POST"), HttpMethod::POST);
        assert_eq!(HttpMethod::from_bytes(b"PUT"), HttpMethod::PUT);
        assert_eq!(HttpMethod::from_bytes(b"DELETE"), HttpMethod::DELETE);
        assert_eq!(HttpMethod::from_bytes(b"UNKNOWN"), HttpMethod::UNKNOWN);
    }

    #[test]
    fn test_http_status() {
        assert_eq!(HttpStatus::Status200.as_u16(), 200);
        assert_eq!(HttpStatus::Status200.reason_phrase(), "OK");
        assert_eq!(HttpStatus::Status404.reason_phrase(), "Not Found");
        assert_eq!(HttpStatus::from_u16(200), HttpStatus::Status200);
        assert_eq!(HttpStatus::from_u16(404), HttpStatus::Status404);
    }
}
