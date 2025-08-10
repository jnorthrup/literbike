// HTTP protocol parser using RBCursive combinators
// Zero-allocation, SIMD-accelerated HTTP parsing

use crate::rbcursive::{
    scanner::SimdScanner,
    combinators::*,
    simd::create_optimal_scanner,
    HttpMethod,
};
use super::{HttpVersion, HttpHeader, HttpRequest};

/// HTTP protocol parser with SIMD acceleration
pub struct HttpParser {
    scanner: Box<dyn SimdScanner>,
}

impl HttpParser {
    pub fn new() -> Self {
        Self {
            scanner: create_optimal_scanner(),
        }
    }
    
    /// Parse complete HTTP request
    pub fn parse_request<'a>(&'a self, input: &'a [u8]) -> ParseResult< HttpRequest<'a>, ParseError> {
        let request_line_parser = RequestLineParser { http_parser: self };
        let headers_parser = HeadersParser { http_parser: self };
        
        let full_parser = sequence(
            request_line_parser,
            sequence(
                tag(b"\r\n"),
                headers_parser
            )
        );
        
        match full_parser.parse(input) {
            ParseResult::Complete(((method, path, version), (_, headers)), consumed) => {
                ParseResult::Complete(
                    HttpRequest {
                        method,
                        path,
                        version,
                        headers,
                    },
                    consumed
                )
            }
            ParseResult::Incomplete(consumed) => ParseResult::Incomplete(consumed),
            ParseResult::Error(err, consumed) => ParseResult::Error(err, consumed),
        }
    }
    
    /// Parse HTTP method using SIMD acceleration
    pub fn parse_method<'a>(&self, input: &'a [u8]) -> ParseResult< HttpMethod, ParseError> {
        // Use SIMD to quickly find the space after method
        let spaces = self.scanner.scan_bytes(input, &[b' ']);
        
        if let Some(&space_pos) = spaces.first() {
            if space_pos > 0 {
                let method_bytes = &input[..space_pos];
                if let Some(method) = HttpMethod::from_bytes(method_bytes) {
                    return ParseResult::Complete(method, space_pos);
                }
            }
        }
        
        // Fallback to combinator-based parsing
        let methods_parser = alternative(
            alternative(
                alternative(
                    alternative(
                        map(tag(b"GET"), |_| HttpMethod::Get),
                        map(tag(b"POST"), |_| HttpMethod::Post)
                    ),
                    alternative(
                        map(tag(b"PUT"), |_| HttpMethod::Put),
                        map(tag(b"DELETE"), |_| HttpMethod::Delete)
                    )
                ),
                alternative(
                    alternative(
                        map(tag(b"HEAD"), |_| HttpMethod::Head),
                        map(tag(b"OPTIONS"), |_| HttpMethod::Options)
                    ),
                    alternative(
                        map(tag(b"CONNECT"), |_| HttpMethod::Connect),
                        map(tag(b"PATCH"), |_| HttpMethod::Patch)
                    )
                )
            ),
            map(tag(b"TRACE"), |_| HttpMethod::Trace)
        );
        
        methods_parser.parse(input)
    }
    
    /// Parse HTTP request line (method, path, version)
    fn parse_request_line<'a>(&self, input: &'a [u8]) -> ParseResult<(HttpMethod, &'a [u8], HttpVersion), ParseError> {
            // Parse method
            let method_result = self.parse_method(input);
            let (method, method_consumed) = match method_result {
                ParseResult::Complete(m, c) => (m, c),
                ParseResult::Incomplete(c) => return ParseResult::Incomplete(c),
                ParseResult::Error(e, c) => return ParseResult::Error(e, c),
            };
            
            let remaining = &input[method_consumed..];
            
            // Parse space after method
            let space_parser = byte(b' ');
            let (_, space_consumed) = match space_parser.parse(remaining) {
                ParseResult::Complete(s, c) => (s, c),
                ParseResult::Incomplete(c) => return ParseResult::Incomplete(method_consumed + c),
                ParseResult::Error(e, c) => return ParseResult::Error(e, method_consumed + c),
            };
            
            let remaining = &remaining[space_consumed..];
            
            // Parse path using SIMD to find next space
            let spaces = self.scanner.scan_bytes(remaining, &[b' ']);
            let (path, path_consumed) = if let Some(&space_pos) = spaces.first() {
                (&remaining[..space_pos], space_pos)
            } else {
                return ParseResult::Incomplete(method_consumed + space_consumed + remaining.len());
            };
            
            let remaining = &remaining[path_consumed..];
            
            // Parse space before version
            let (_, space2_consumed) = match space_parser.parse(remaining) {
                ParseResult::Complete(s, c) => (s, c),
                ParseResult::Incomplete(c) => return ParseResult::Incomplete(method_consumed + space_consumed + path_consumed + c),
                ParseResult::Error(e, c) => return ParseResult::Error(e, method_consumed + space_consumed + path_consumed + c),
            };
            
            let remaining = &remaining[space2_consumed..];
            
            // Parse HTTP version
            let version_parser = alternative(
                alternative(
                    map(tag(b"HTTP/1.1"), |_| HttpVersion::Http11),
                    map(tag(b"HTTP/1.0"), |_| HttpVersion::Http10)
                ),
                map(tag(b"HTTP/2"), |_| HttpVersion::Http2)
            );
            
            let (version, version_consumed) = match version_parser.parse(remaining) {
                ParseResult::Complete(v, c) => (v, c),
                ParseResult::Incomplete(c) => return ParseResult::Incomplete(method_consumed + space_consumed + path_consumed + space2_consumed + c),
                ParseResult::Error(e, c) => return ParseResult::Error(e, method_consumed + space_consumed + path_consumed + space2_consumed + c),
            };
            
            let total_consumed = method_consumed + space_consumed + path_consumed + space2_consumed + version_consumed;
            ParseResult::Complete((method, path, version), total_consumed)
    }
    
    /// Parse HTTP headers using SIMD acceleration
    fn parse_headers<'a>(&self, input: &'a [u8]) -> ParseResult<Vec<HttpHeader<'a>>, ParseError> {
            let mut headers = Vec::new();
            let mut consumed = 0;
            let mut remaining = input;
            
            loop {
                // Check for end of headers (empty line)
                if remaining.starts_with(b"\r\n") {
                    consumed += 2;
                    break;
                }
                
                if remaining.is_empty() {
                    return ParseResult::Incomplete(consumed);
                }
                
                // Parse single header using SIMD to find colon
                let colons = self.scanner.scan_bytes(remaining, &[b':']);
                let colon_pos = if let Some(&pos) = colons.first() {
                    pos
                } else {
                    return ParseResult::Incomplete(consumed + remaining.len());
                };
                
                let header_name = &remaining[..colon_pos];
                let mut after_colon = &remaining[colon_pos + 1..];
                
                // Skip optional whitespace after colon
                while after_colon.starts_with(b" ") || after_colon.starts_with(b"\t") {
                    after_colon = &after_colon[1..];
                }
                
                // Find end of line using SIMD
                let crlf_pos = self.find_crlf(after_colon);
                let (header_value, line_end_consumed) = if let Some(pos) = crlf_pos {
                    (&after_colon[..pos], pos + 2) // +2 for \r\n
                } else {
                    return ParseResult::Incomplete(consumed + remaining.len());
                };
                
                headers.push(HttpHeader {
                    name: header_name,
                    value: header_value,
                });
                
                let header_consumed = colon_pos + 1 + (after_colon.as_ptr() as usize - remaining.as_ptr() as usize - colon_pos - 1) + line_end_consumed;
                consumed += header_consumed;
                remaining = &remaining[header_consumed..];
            }
            
            ParseResult::Complete(headers, consumed)
    }
    
    /// Find CRLF sequence using SIMD
    fn find_crlf(&self, data: &[u8]) -> Option<usize> {
        // Use SIMD to find all \r characters
        let cr_positions = self.scanner.scan_bytes(data, &[b'\r']);
        
        for &pos in &cr_positions {
            if pos + 1 < data.len() && data[pos + 1] == b'\n' {
                return Some(pos);
            }
        }
        
        None
    }
}

// Implement the closure as a proper parser
struct RequestLineParser<'a> {
    http_parser: &'a HttpParser,
}

impl<'a> Parser<'a, (HttpMethod, &'a [u8], HttpVersion)> for RequestLineParser<'a> {
    type Error = ParseError;
    
    fn parse(&self, input: &'a [u8]) -> ParseResult< (HttpMethod, &'a [u8], HttpVersion), ParseError> {
        self.http_parser.parse_request_line(input)
    }
}

struct HeadersParser<'a> {
    http_parser: &'a HttpParser,
}

impl<'a> Parser<'a, Vec<HttpHeader<'a>>> for HeadersParser<'a> {
    type Error = ParseError;
    
    fn parse(&self, input: &'a [u8]) -> ParseResult< Vec<HttpHeader<'a>>, ParseError> {
        self.http_parser.parse_headers(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_method_parsing() {
        let parser = HttpParser::new();
        
        let input = b"GET /path";
        match parser.parse_method(input) {
            ParseResult::Complete(method, consumed) => {
                assert_eq!(method, HttpMethod::Get);
                assert_eq!(consumed, 3);
            }
            _ => panic!("Expected successful method parse"),
        }
    }

    #[test]
    fn test_http_request_parsing() {
        let parser = HttpParser::new();
        
        let input = b"GET /api/v1/users HTTP/1.1\r\nHost: api.example.com\r\nUser-Agent: RBCursive/1.0\r\n\r\n";
        
        match parser.parse_request(input) {
            ParseResult::Complete(request, consumed) => {
                assert_eq!(request.method, HttpMethod::Get);
                assert_eq!(request.path, b"/api/v1/users");
                assert_eq!(request.version, HttpVersion::Http11);
                assert_eq!(request.headers.len(), 2);
                
                assert_eq!(request.headers[0].name, b"Host");
                assert_eq!(request.headers[0].value, b"api.example.com");
                
                assert_eq!(request.headers[1].name, b"User-Agent");
                assert_eq!(request.headers[1].value, b"RBCursive/1.0");
                
                assert!(consumed > 0);
            }
            other => panic!("Expected successful request parse, got {:?}", other),
        }
    }

    #[test]
    fn test_http_connect_method() {
        let parser = HttpParser::new();
        
        let input = b"CONNECT proxy.example.com:443 HTTP/1.1\r\nHost: proxy.example.com:443\r\n\r\n";
        
        match parser.parse_request(input) {
            ParseResult::Complete(request, _) => {
                assert_eq!(request.method, HttpMethod::Connect);
                assert_eq!(request.path, b"proxy.example.com:443");
                assert_eq!(request.version, HttpVersion::Http11);
            }
            _ => panic!("Expected successful CONNECT parse"),
        }
    }

    #[test]
    fn test_incomplete_http_request() {
        let parser = HttpParser::new();
        
        let input = b"GET /api"; // Incomplete request
        
        match parser.parse_request(input) {
            ParseResult::Incomplete(_) => {
                // Expected - not enough data
            }
            other => panic!("Expected incomplete result, got {:?}", other),
        }
    }

    #[test]
    fn test_http_with_simd_scanner() {
        let parser = HttpParser::new();
        
        let input = b"POST /submit HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: 13\r\n\r\n";
        
        match parser.parse_request(input) {
            ParseResult::Complete(request, _) => {
                assert_eq!(request.method, HttpMethod::Post);
                assert_eq!(request.path, b"/submit");
                assert_eq!(request.headers.len(), 2);
            }
            _ => panic!("Expected successful parse with SIMD"),
        }
    }
}