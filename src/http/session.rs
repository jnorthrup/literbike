//! HTTP Session - relaxfactory Tx pattern
//!
//! Manages per-connection HTTP state: header buffer, read/write phases, response building

use super::header_parser::{HeaderParser, HttpMethod, HttpStatus};
use std::io::{self, Read, Write};

/// HTTP session state (like relaxfactory Tx)
pub struct HttpSession {
    /// Header parser (holds header buffer)
    pub parser: HeaderParser,
    
    /// Session state
    pub state: SessionState,
    
    /// Read phase
    pub read_phase: ReadPhase,
    
    /// Write phase  
    pub write_phase: WritePhase,
    
    /// Request body buffer (if Content-Length > 0)
    pub body_buffer: Vec<u8>,
    
    /// Response buffer (being written)
    pub response_buffer: Vec<u8>,
    
    /// Content-Length for request body
    pub expected_body_len: Option<usize>,
    
    /// Keep-alive connection
    pub keep_alive: bool,
}

/// Session state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Reading headers
    ReadingHeaders,
    /// Reading body (if Content-Length)
    ReadingBody,
    /// Processing request
    Processing,
    /// Writing response
    Writing,
    /// Done (close or keep-alive)
    Done,
}

/// Read phase
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadPhase {
    /// Initial read
    Initial,
    /// Reading headers
    Headers,
    /// Reading body
    Body,
}

/// Write phase
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WritePhase {
    /// Nothing to write
    Idle,
    /// Writing headers
    Headers,
    /// Writing body
    Body,
}

impl HttpSession {
    /// Create new HTTP session
    pub fn new() -> Self {
        Self {
            parser: HeaderParser::with_capacity(512),
            state: SessionState::ReadingHeaders,
            read_phase: ReadPhase::Initial,
            write_phase: WritePhase::Idle,
            body_buffer: Vec::new(),
            response_buffer: Vec::new(),
            expected_body_len: None,
            keep_alive: true,
        }
    }

    /// Reset session for keep-alive reuse (like relaxfactory session reset)
    pub fn reset(&mut self) {
        self.parser.clear();
        self.state = SessionState::ReadingHeaders;
        self.read_phase = ReadPhase::Initial;
        self.write_phase = WritePhase::Idle;
        self.body_buffer.clear();
        self.response_buffer.clear();
        self.expected_body_len = None;
        self.keep_alive = true;
    }

    /// Read from socket into header/body buffer
    pub fn read_from_socket<R: Read>(&mut self, reader: &mut R) -> io::Result<usize> {
        match self.state {
            SessionState::ReadingHeaders => {
                let buf = self.parser.buffer_mut();
                let start_len = buf.len();
                
                // Grow buffer if needed
                buf.resize(start_len + 1024, 0);
                
                let n = reader.read(&mut buf[start_len..])?;
                buf.truncate(start_len + n);
                
                if n > 0 {
                    self.read_phase = ReadPhase::Headers;
                }
                
                Ok(n)
            }
            SessionState::ReadingBody => {
                if let Some(expected) = self.expected_body_len {
                    let current_len = self.body_buffer.len();
                    if current_len < expected {
                        let remaining = expected - current_len;
                        self.body_buffer.resize(current_len + remaining, 0);
                        
                        let n = reader.read(&mut self.body_buffer[current_len..])?;
                        self.body_buffer.truncate(current_len + n);
                        
                        self.read_phase = ReadPhase::Body;
                        return Ok(n);
                    }
                }
                Ok(0)
            }
            _ => Ok(0),
        }
    }

    /// Try to parse headers (like Tx.readHttpHeaders)
    pub fn try_parse_headers(&mut self) -> io::Result<bool> {
        if self.state != SessionState::ReadingHeaders {
            return Ok(false);
        }

        match self.parser.parse() {
            Ok(true) => {
                // Headers complete
                self.state = SessionState::ReadingBody;
                
                // Check Content-Length
                if let Some(len) = self.parser.content_length() {
                    self.expected_body_len = Some(len);
                    if len == 0 {
                        self.state = SessionState::Processing;
                    }
                } else {
                    // No Content-Length, proceed to processing
                    self.state = SessionState::Processing;
                }
                
                // Move any body bytes already read into parser buffer over to body_buffer
                if let Some(offset) = self.parser.body_offset() {
                    let buf = self.parser.buffer();
                    if buf.len() > offset {
                        let body_bytes = buf[offset..].to_vec();
                        self.body_buffer.extend_from_slice(&body_bytes);
                    }
                }
                
                // Check Connection header for keep-alive
                if let Some(conn) = self.parser.header("Connection") {
                    self.keep_alive = conn.eq_ignore_ascii_case("keep-alive");
                } else {
                    // HTTP/1.1 defaults to keep-alive
                    self.keep_alive = self.parser.protocol() == Some("HTTP/1.1");
                }
                
                Ok(true)
            }
            Ok(false) => Ok(false),
            Err(e) => Err(io::Error::new(io::ErrorKind::InvalidData, e)),
        }
    }

    /// Check if body reading is complete
    pub fn body_complete(&self) -> bool {
        match (self.state, self.expected_body_len) {
            (SessionState::ReadingBody, Some(expected)) => {
                self.body_buffer.len() >= expected
            }
            _ => true,
        }
    }

    /// Mark body reading complete, transition to processing
    pub fn finish_reading_body(&mut self) {
        if self.body_complete() {
            self.state = SessionState::Processing;
        }
    }

    /// Prepare response for writing
    pub fn prepare_response(&mut self, status: HttpStatus, content_type: &str, body: &[u8]) {
        self.response_buffer = self.parser.build_simple_response(status, content_type, body);
        self.state = SessionState::Writing;
        self.write_phase = WritePhase::Headers;
    }

    /// Write response to socket
    pub fn write_to_socket<W: Write>(&mut self, writer: &mut W) -> io::Result<usize> {
        if self.state != SessionState::Writing {
            return Ok(0);
        }

        let n = writer.write(&self.response_buffer)?;
        
        if n >= self.response_buffer.len() {
            self.response_buffer.clear();
            self.write_phase = WritePhase::Idle;
            
            if self.keep_alive {
                self.reset();
                self.state = SessionState::ReadingHeaders;
            } else {
                self.state = SessionState::Done;
            }
        }
        
        Ok(n)
    }

    /// Get HTTP method
    pub fn method(&self) -> Option<HttpMethod> {
        self.parser.method()
    }

    /// Get request path
    pub fn path(&self) -> Option<&str> {
        self.parser.path()
    }

    /// Get header value
    pub fn header(&self, name: &str) -> Option<&str> {
        self.parser.header(name)
    }

    /// Get request body
    pub fn body(&self) -> &[u8] {
        &self.body_buffer
    }

    /// Check if session is done (can close connection)
    pub fn is_done(&self) -> bool {
        self.state == SessionState::Done
    }

    /// Check if session wants to read
    pub fn wants_read(&self) -> bool {
        matches!(self.state, SessionState::ReadingHeaders | SessionState::ReadingBody)
    }

    /// Check if session wants to write
    pub fn wants_write(&self) -> bool {
        matches!(self.state, SessionState::Writing)
    }
}

impl Default for HttpSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_session_creation() {
        let session = HttpSession::new();
        assert_eq!(session.state, SessionState::ReadingHeaders);
        assert!(!session.wants_write());
        assert!(session.wants_read());
    }

    #[test]
    fn test_read_headers() {
        let mut session = HttpSession::new();
        let request = b"GET /test HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let mut cursor = Cursor::new(request);
        
        let n = session.read_from_socket(&mut cursor).unwrap();
        assert!(n > 0);
        
        let parsed = session.try_parse_headers().unwrap();
        assert!(parsed);
        assert_eq!(session.method(), Some(HttpMethod::GET));
        assert_eq!(session.path(), Some("/test"));
    }

    #[test]
    fn test_read_body() {
        let mut session = HttpSession::new();
        let request = b"POST /test HTTP/1.1\r\nHost: localhost\r\nContent-Length: 11\r\n\r\nHello, World";
        let mut cursor = Cursor::new(request);
        
        // Read headers
        let n = session.read_from_socket(&mut cursor).unwrap();
        assert!(n > 0);
        
        session.try_parse_headers().unwrap();
        assert_eq!(session.state, SessionState::ReadingBody);
        assert_eq!(session.expected_body_len, Some(11));
        
        // Read body
        while !session.body_complete() {
            session.read_from_socket(&mut cursor).unwrap();
        }
        
        session.finish_reading_body();
        assert_eq!(session.state, SessionState::Processing);
        assert_eq!(session.body(), b"Hello, World");
    }

    #[test]
    fn test_prepare_response() {
        let mut session = HttpSession::new();
        session.prepare_response(HttpStatus::Status200, "text/plain", b"Hello");
        
        assert_eq!(session.state, SessionState::Writing);
        assert!(session.wants_write());
        assert!(!session.response_buffer.is_empty());
        
        // Check response format
        let response = String::from_utf8_lossy(&session.response_buffer);
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Content-Type: text/plain"));
        assert!(response.contains("Content-Length: 5"));
    }

    #[test]
    fn test_keep_alive_reset() {
        let mut session = HttpSession::new();
        let request = b"GET /test HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let mut cursor = Cursor::new(request);
        
        session.read_from_socket(&mut cursor).unwrap();
        session.try_parse_headers().unwrap();
        assert!(session.keep_alive);
        
        session.prepare_response(HttpStatus::Status200, "text/plain", b"OK");
        
        // Write response
        let mut output = Vec::new();
        session.write_to_socket(&mut output).unwrap();
        
        // Session should be reset for keep-alive
        assert_eq!(session.state, SessionState::ReadingHeaders);
        assert!(session.wants_read());
    }
}
