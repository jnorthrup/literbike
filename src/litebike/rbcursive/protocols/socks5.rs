// SOCKS5 protocol parser using RBCursive combinators
// Binary protocol parsing with precise byte handling

use crate::rbcursive::{
    scanner::SimdScanner,
    combinators::*,
    simd::create_optimal_scanner,
};
use super::{Socks5Handshake, Socks5Connect, Socks5AuthMethod};

/// SOCKS5 protocol parser
pub struct Socks5Parser {
    _scanner: Box<dyn SimdScanner>,
}

impl Socks5Parser {
    pub fn new() -> Self {
        Self {
            _scanner: create_optimal_scanner(),
        }
    }
    
    /// Parse SOCKS5 handshake request
    pub fn parse_handshake<'a>(&self, input: &'a [u8]) -> ParseResult< Socks5Handshake, ParseError> {
        if input.len() < 3 {
            return ParseResult::Incomplete(input.len());
        }
        
        // SOCKS5 version must be 0x05
        if input[0] != 0x05 {
            return ParseResult::Error(ParseError::InvalidProtocol, 0);
        }
        
        let version = input[0];
        let method_count = input[1] as usize;
        
        if input.len() < 2 + method_count {
            return ParseResult::Incomplete(input.len());
        }
        
        let mut methods = Vec::with_capacity(method_count);
        for i in 0..method_count {
            let method_byte = input[2 + i];
            let method = match method_byte {
                0x00 => Socks5AuthMethod::NoAuth,
                0x01 => Socks5AuthMethod::GssApi,
                0x02 => Socks5AuthMethod::UserPass,
                0xFF => Socks5AuthMethod::NoAcceptable,
                _ => {
                    // Unknown method - for now, we'll treat as NoAcceptable
                    Socks5AuthMethod::NoAcceptable
                }
            };
            methods.push(method);
        }
        
        ParseResult::Complete(
            Socks5Handshake { version, methods },
            2 + method_count
        )
    }
    
    /// Parse SOCKS5 connect request
    pub fn parse_connect<'a>(&self, input: &'a [u8]) -> ParseResult< Socks5Connect<'a>, ParseError> {
        if input.len() < 4 {
            return ParseResult::Incomplete(input.len());
        }
        
        // Check SOCKS5 version
        if input[0] != 0x05 {
            return ParseResult::Error(ParseError::InvalidProtocol, 0);
        }
        
        let version = input[0];
        let command = input[1];
        let _reserved = input[2]; // Should be 0x00, but we don't enforce
        let address_type = input[3];
        
        let (address, address_len) = match address_type {
            0x01 => {
                // IPv4 address (4 bytes)
                if input.len() < 4 + 4 + 2 {
                    return ParseResult::Incomplete(input.len());
                }
                (&input[4..8], 4)
            }
            0x03 => {
                // Domain name
                if input.len() < 5 {
                    return ParseResult::Incomplete(input.len());
                }
                let domain_len = input[4] as usize;
                if input.len() < 5 + domain_len + 2 {
                    return ParseResult::Incomplete(input.len());
                }
                (&input[5..5 + domain_len], domain_len + 1) // +1 for length byte
            }
            0x04 => {
                // IPv6 address (16 bytes)
                if input.len() < 4 + 16 + 2 {
                    return ParseResult::Incomplete(input.len());
                }
                (&input[4..20], 16)
            }
            _ => {
                return ParseResult::Error(ParseError::InvalidInput, 3);
            }
        };
        
        let port_offset = 4 + address_len;
        if input.len() < port_offset + 2 {
            return ParseResult::Incomplete(input.len());
        }
        
        let port = u16::from_be_bytes([
            input[port_offset],
            input[port_offset + 1]
        ]);
        
        ParseResult::Complete(
            Socks5Connect {
                version,
                command,
                address_type,
                address,
                port,
            },
            port_offset + 2
        )
    }
    
    /// Detect if data looks like SOCKS5 protocol
    pub fn is_socks5(&self, input: &[u8]) -> bool {
        if input.len() < 3 {
            return false;
        }
        
        // Check version byte
        if input[0] != 0x05 {
            return false;
        }
        
        // Check method count is reasonable
        let method_count = input[1] as usize;
        if method_count == 0 || method_count > 255 {
            return false;
        }
        
        // Check we have enough bytes for the methods
        if input.len() < 2 + method_count {
            return false;
        }
        
        // Basic validation passed
        true
    }
    
    /// Parse either handshake or connect request
    pub fn parse_request<'a>(&self, input: &'a [u8]) -> ParseResult< Socks5Request<'a>, ParseError> {
        if input.len() < 3 {
            return ParseResult::Incomplete(input.len());
        }
        
        if input[0] != 0x05 {
            return ParseResult::Error(ParseError::InvalidProtocol, 0);
        }
        
        // Heuristic: if second byte is a small number (< 10), it's likely method count (handshake)
        // If it's 0x01 (CONNECT), it's likely a connect request
        let second_byte = input[1];
        
        if second_byte == 0x01 && input.len() >= 4 && input[2] == 0x00 {
            // Looks like CONNECT request (cmd=0x01, reserved=0x00)
            match self.parse_connect(input) {
                ParseResult::Complete(connect, consumed) => {
                    ParseResult::Complete(Socks5Request::Connect(connect), consumed)
                }
                ParseResult::Incomplete(c) => ParseResult::Incomplete(c),
                ParseResult::Error(e, c) => ParseResult::Error(e, c),
            }
        } else if second_byte > 0 && second_byte < 10 {
            // Looks like handshake with method count
            match self.parse_handshake(input) {
                ParseResult::Complete(handshake, consumed) => {
                    ParseResult::Complete(Socks5Request::Handshake(handshake), consumed)
                }
                ParseResult::Incomplete(c) => ParseResult::Incomplete(c),
                ParseResult::Error(e, c) => ParseResult::Error(e, c),
            }
        } else {
            ParseResult::Error(ParseError::InvalidInput, 1)
        }
    }
}

/// SOCKS5 request variants
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Socks5Request<'a> {
    Handshake(Socks5Handshake),
    Connect(Socks5Connect<'a>),
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socks5_handshake_parsing() {
        let parser = Socks5Parser::new(ScanStrategy::Scalar);
        
        // SOCKS5 handshake: version=5, method_count=2, methods=[NoAuth, UserPass]
        let input = &[0x05, 0x02, 0x00, 0x02];
        
        match parser.parse_handshake(input) {
            ParseResult::Complete(handshake, consumed) => {
                assert_eq!(handshake.version, 0x05);
                assert_eq!(handshake.methods.len(), 2);
                assert_eq!(handshake.methods[0], Socks5AuthMethod::NoAuth);
                assert_eq!(handshake.methods[1], Socks5AuthMethod::UserPass);
                assert_eq!(consumed, 4);
            }
            other => panic!("Expected successful handshake parse, got {:?}", other),
        }
    }

    #[test]
    fn test_socks5_connect_ipv4() {
        let parser = Socks5Parser::new(ScanStrategy::Scalar);
        
        // SOCKS5 connect: version=5, cmd=1(CONNECT), reserved=0, atyp=1(IPv4)
        // address=192.168.1.1, port=80
        let input = &[
            0x05, 0x01, 0x00, 0x01,  // header
            192, 168, 1, 1,           // IPv4 address
            0x00, 0x50                // port 80
        ];
        
        match parser.parse_connect(input) {
            ParseResult::Complete(connect, consumed) => {
                assert_eq!(connect.version, 0x05);
                assert_eq!(connect.command, 0x01);
                assert_eq!(connect.address_type, 0x01);
                assert_eq!(connect.address, &[192, 168, 1, 1]);
                assert_eq!(connect.port, 80);
                assert_eq!(consumed, 10);
            }
            other => panic!("Expected successful connect parse, got {:?}", other),
        }
    }

    #[test]
    fn test_socks5_connect_domain() {
        let parser = Socks5Parser::new(ScanStrategy::Scalar);
        
        // SOCKS5 connect with domain name "example.com"
        let domain = b"example.com";
        let mut input = vec![
            0x05, 0x01, 0x00, 0x03,  // header with domain type
            domain.len() as u8        // domain length
        ];
        input.extend_from_slice(domain);
        input.extend_from_slice(&[0x01, 0xBB]); // port 443
        
        match parser.parse_connect(&input) {
            ParseResult::Complete(connect, consumed) => {
                assert_eq!(connect.version, 0x05);
                assert_eq!(connect.command, 0x01);
                assert_eq!(connect.address_type, 0x03);
                assert_eq!(connect.address, domain);
                assert_eq!(connect.port, 443);
                assert_eq!(consumed, input.len());
            }
            other => panic!("Expected successful domain connect parse, got {:?}", other),
        }
    }

    #[test]
    fn test_socks5_detection() {
        let parser = Socks5Parser::new(ScanStrategy::Scalar);
        
        // Valid SOCKS5 handshake
        let valid_socks5 = &[0x05, 0x01, 0x00];
        assert!(parser.is_socks5(valid_socks5));
        
        // Invalid version
        let invalid_version = &[0x04, 0x01, 0x00];
        assert!(!parser.is_socks5(invalid_version));
        
        // Invalid method count
        let invalid_methods = &[0x05, 0x00, 0x00];
        assert!(!parser.is_socks5(invalid_methods));
        
        // Too short
        let too_short = &[0x05];
        assert!(!parser.is_socks5(too_short));
    }

    #[test]
    fn test_socks5_incomplete() {
        let parser = Socks5Parser::new(ScanStrategy::Scalar);
        
        // Incomplete handshake
        let incomplete = &[0x05, 0x02]; // Missing method bytes
        match parser.parse_handshake(incomplete) {
            ParseResult::Incomplete(_) => {
                // Expected
            }
            other => panic!("Expected incomplete result, got {:?}", other),
        }
        
        // Incomplete connect
        let incomplete_connect = &[0x05, 0x01, 0x00, 0x01, 192]; // Missing address bytes
        match parser.parse_connect(incomplete_connect) {
            ParseResult::Incomplete(_) => {
                // Expected
            }
            other => panic!("Expected incomplete result, got {:?}", other),
        }
    }

    #[test]
    fn test_socks5_request_parsing() {
        let parser = Socks5Parser::new(ScanStrategy::Scalar);
        
        // Test handshake detection
        let handshake_data = &[0x05, 0x01, 0x00];
        match parser.parse_request(handshake_data) {
            ParseResult::Complete(Socks5Request::Handshake(_), _) => {
                // Expected
            }
            other => panic!("Expected handshake request, got {:?}", other),
        }
        
        // Test connect detection
        let connect_data = &[
            0x05, 0x01, 0x00, 0x01,  // CONNECT to IPv4
            127, 0, 0, 1,             // localhost
            0x00, 0x50                // port 80
        ];
        match parser.parse_request(connect_data) {
            ParseResult::Complete(Socks5Request::Connect(_), _) => {
                // Expected
            }
            other => panic!("Expected connect request, got {:?}", other),
        }
    }
}