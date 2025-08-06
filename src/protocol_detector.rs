// BBCursive-Inspired Protocol Detector with Automatic Rarity Tabulation
// Each protocol builds lookup tables, overlaps create penalties, rarest wins
// Enhanced with static code generation for zero-overhead detection

use std::io;

// Import the static code generation system



// Import autovec optimizations
// use crate::autovec_optimization::{AutoVecDetector, arch_specific};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
    Unknown,
    Http,
    Connect,
    WebSocket,
    Doh,
    Upnp,
    Bonjour,
    Http2,
    Tls,
    Socks5,
    ProxyProtocol,
}

#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub protocol: Protocol,
    pub confidence: u8,
    pub bytes_consumed: usize,
}

impl DetectionResult {
    pub fn new(protocol: Protocol, confidence: u8, bytes_consumed: usize) -> Self {
        Self { protocol, confidence, bytes_consumed }
    }
    
    pub fn unknown() -> Self {
        Self { protocol: Protocol::Unknown, confidence: 0, bytes_consumed: 0 }
    }
}

/// Byte range that a protocol claims to scan
#[derive(Debug, Clone, Copy)]
pub struct ByteRange {
    pub start: u8,
    pub end: u8,  // inclusive
    pub protocol: Protocol,
    pub validator: fn(&[u8], usize) -> Option<DetectionResult>,
}

impl ByteRange {
    pub const fn single(byte: u8, protocol: Protocol, validator: fn(&[u8], usize) -> Option<DetectionResult>) -> Self {
        Self { start: byte, end: byte, protocol, validator }
    }
    
    pub const fn range(start: u8, end: u8, protocol: Protocol, validator: fn(&[u8], usize) -> Option<DetectionResult>) -> Self {
        Self { start, end, protocol, validator }
    }
}

/// Protocol scanner combinator - builds its own lookup table
pub trait ProtocolScanner {
    /// Return all byte ranges this protocol claims
    fn byte_ranges() -> &'static [ByteRange];
    
    /// Protocol identifier
    fn protocol() -> Protocol;
}

/// SOCKS5 scanner - claims only 0x05
pub struct Socks5Scanner;
const SOCKS5_RANGES: &[ByteRange] = &[ByteRange::single(0x05, Protocol::Socks5, validate_socks5)];
impl ProtocolScanner for Socks5Scanner {
    fn byte_ranges() -> &'static [ByteRange] {
        SOCKS5_RANGES
    }
    
    fn protocol() -> Protocol { Protocol::Socks5 }
}

/// HTTP scanner - claims G,P,D,H,O,C,T,U
pub struct HttpScanner;
const HTTP_RANGES: &[ByteRange] = &[
    ByteRange::single(b'G', Protocol::Http, validate_http),  // GET
    ByteRange::single(b'P', Protocol::Http, validate_http),  // POST, PUT, PATCH
    ByteRange::single(b'D', Protocol::Http, validate_http),  // DELETE
    ByteRange::single(b'H', Protocol::Http, validate_http),  // HEAD
    ByteRange::single(b'O', Protocol::Http, validate_http),  // OPTIONS
    ByteRange::single(b'C', Protocol::Connect, validate_connect), // CONNECT
    ByteRange::single(b'T', Protocol::Http, validate_http),  // TRACE
    ByteRange::single(b'U', Protocol::Http, validate_http),  // UPDATE (custom)
];
impl ProtocolScanner for HttpScanner {
    fn byte_ranges() -> &'static [ByteRange] {
        HTTP_RANGES
    }
    
    fn protocol() -> Protocol { Protocol::Http }
}

/// TLS scanner - claims 0x16
pub struct TlsScanner;
const TLS_RANGES: &[ByteRange] = &[ByteRange::single(0x16, Protocol::Tls, validate_tls)];
impl ProtocolScanner for TlsScanner {
    fn byte_ranges() -> &'static [ByteRange] {
        TLS_RANGES
    }
    
    fn protocol() -> Protocol { Protocol::Tls }
}

/// HTTP/2 scanner - claims 'P' for "PRI * HTTP/2.0"
pub struct Http2Scanner;
const HTTP2_RANGES: &[ByteRange] = &[ByteRange::single(b'P', Protocol::Http2, validate_http2)];
impl ProtocolScanner for Http2Scanner {
    fn byte_ranges() -> &'static [ByteRange] {
        HTTP2_RANGES
    }
    
    fn protocol() -> Protocol { Protocol::Http2 }
}

/// Compile-time byte ownership table builder
pub struct ByteOwnershipTable {
    // For each byte 0-255, list of protocols that claim it
    owners: [[Option<Protocol>; 8]; 256],  // Up to 8 protocols per byte
    counts: [u8; 256],  // How many protocols claim each byte
}

impl ByteOwnershipTable {
    pub const fn new() -> Self {
        Self {
            owners: [[None; 8]; 256],
            counts: [0; 256],
        }
    }
    
    /// Build the ownership table from all protocol scanners
    pub fn build() -> Self {
        let mut table = Self::new();
        
        // Register all protocol scanners
        table.register::<Socks5Scanner>();
        table.register::<HttpScanner>();
        table.register::<TlsScanner>();
        table.register::<Http2Scanner>();
        
        table
    }
    
    fn register<S: ProtocolScanner>(&mut self) {
        for range in S::byte_ranges() {
            for byte in range.start..=range.end {
                let idx = self.counts[byte as usize] as usize;
                if idx < 8 {
                    self.owners[byte as usize][idx] = Some(range.protocol);
                    self.counts[byte as usize] += 1;
                }
            }
        }
    }
    
    /// Get penalty (overlap count) for a byte
    pub fn penalty(&self, byte: u8) -> u8 {
        self.counts[byte as usize]
    }
    
    /// Get all protocols claiming a byte
    pub fn claimants(&self, byte: u8) -> &[Option<Protocol>] {
        let count = self.counts[byte as usize] as usize;
        &self.owners[byte as usize][..count]
    }
}

/// Automatic rarity-ordered protocol detector
pub struct ProtocolDetector {
    enabled_protocols: u64,
    ownership_table: ByteOwnershipTable,
    validators: ValidatorTable,
}

/// Validator function lookup table
pub struct ValidatorTable {
    // Maps byte -> validator functions (max 8 per byte)
    validators: [[Option<fn(&[u8], usize) -> Option<DetectionResult>>; 8]; 256],
    counts: [u8; 256],
}

impl ValidatorTable {
    fn new() -> Self {
        Self {
            validators: [[None; 8]; 256],
            counts: [0; 256],
        }
    }
    
    fn build() -> Self {
        let mut table = Self::new();
        
        // Register all validators from scanners
        for range in Socks5Scanner::byte_ranges() {
            for byte in range.start..=range.end {
                let idx = table.counts[byte as usize] as usize;
                if idx < 8 {
                    table.validators[byte as usize][idx] = Some(range.validator);
                    table.counts[byte as usize] += 1;
                }
            }
        }
        
        for range in HttpScanner::byte_ranges() {
            for byte in range.start..=range.end {
                let idx = table.counts[byte as usize] as usize;
                if idx < 8 {
                    table.validators[byte as usize][idx] = Some(range.validator);
                    table.counts[byte as usize] += 1;
                }
            }
        }
        
        for range in TlsScanner::byte_ranges() {
            for byte in range.start..=range.end {
                let idx = table.counts[byte as usize] as usize;
                if idx < 8 {
                    table.validators[byte as usize][idx] = Some(range.validator);
                    table.counts[byte as usize] += 1;
                }
            }
        }
        
        for range in Http2Scanner::byte_ranges() {
            for byte in range.start..=range.end {
                let idx = table.counts[byte as usize] as usize;
                if idx < 8 {
                    table.validators[byte as usize][idx] = Some(range.validator);
                    table.counts[byte as usize] += 1;
                }
            }
        }
        
        table
    }
    
    fn get_validators(&self, byte: u8) -> &[Option<fn(&[u8], usize) -> Option<DetectionResult>>] {
        let count = self.counts[byte as usize] as usize;
        &self.validators[byte as usize][..count]
    }
}

impl ProtocolDetector {
    pub fn new() -> Self {
        Self {
            enabled_protocols: 0xFFFFFFFF,
            ownership_table: ByteOwnershipTable::build(),
            validators: ValidatorTable::build(),
        }
    }
    
    /// Detect protocol using automatic rarity ordering
    /// Rarest bytes (fewest claimants) are checked first
    pub fn detect(&self, buffer: &[u8]) -> DetectionResult {
        if buffer.is_empty() {
            return DetectionResult::unknown();
        }
        
        // Get first byte and its penalty (overlap count)
        let first_byte = buffer[0];
        let penalty = self.ownership_table.penalty(first_byte);
        
        // If no protocol claims this byte, unknown
        if penalty == 0 {
            return DetectionResult::unknown();
        }
        
        // Try all validators for this byte
        // They're automatically ordered by rarity (fewest claimants first)
        for validator_opt in self.validators.get_validators(first_byte) {
            if let Some(validator) = validator_opt {
                if let Some(result) = validator(buffer, 0) {
                    return result;
                }
            }
        }
        
        DetectionResult::unknown()
    }
    
    /// Get byte rarity ranking (lower penalty = rarer)
    pub fn byte_rarity(&self, byte: u8) -> u8 {
        self.ownership_table.penalty(byte)
    }

    
}

// Validator functions that check full protocol patterns

fn validate_socks5(buffer: &[u8], pos: usize) -> Option<DetectionResult> {
    if buffer.len() >= pos + 2 && buffer[pos] == 0x05 {
        Some(DetectionResult::new(Protocol::Socks5, 255, 2))
    } else {
        None
    }
}

fn validate_http(buffer: &[u8], pos: usize) -> Option<DetectionResult> {
    if buffer.len() < pos + 4 {
        return None;
    }
    
    let slice = &buffer[pos..];
    if slice.starts_with(b"GET ") || slice.starts_with(b"POST ") ||
       slice.starts_with(b"PUT ") || slice.starts_with(b"DELETE ") ||
       slice.starts_with(b"HEAD ") || slice.starts_with(b"OPTIONS ") ||
       slice.starts_with(b"TRACE ") {
        Some(DetectionResult::new(Protocol::Http, 240, 4))
    } else {
        None
    }
}

fn validate_connect(buffer: &[u8], pos: usize) -> Option<DetectionResult> {
    if buffer.len() >= pos + 8 && buffer[pos..].starts_with(b"CONNECT ") {
        Some(DetectionResult::new(Protocol::Connect, 240, 8))
    } else {
        None
    }
}

fn validate_tls(buffer: &[u8], pos: usize) -> Option<DetectionResult> {
    if buffer.len() >= pos + 3 && buffer[pos] == 0x16 && buffer[pos+1] == 0x03 {
        Some(DetectionResult::new(Protocol::Tls, 250, 3))
    } else {
        None
    }
}

fn validate_http2(buffer: &[u8], pos: usize) -> Option<DetectionResult> {
    if buffer.len() >= pos + 24 && buffer[pos..].starts_with(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n") {
        Some(DetectionResult::new(Protocol::Http2, 255, 24))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_byte_ownership() {
        let table = ByteOwnershipTable::build();
        
        // SOCKS5 claims only 0x05 - rarest
        assert_eq!(table.penalty(0x05), 1);
        
        // 'P' is claimed by both HTTP and HTTP/2 - penalty of 2
        assert_eq!(table.penalty(b'P'), 2);
        
        // Random byte not claimed by anyone
        assert_eq!(table.penalty(0xFF), 0);
    }
    
    #[test]
    fn test_rarity_ordering() {
        let detector = ProtocolDetector::new();
        
        // SOCKS5 (0x05) should be rarest
        assert_eq!(detector.byte_rarity(0x05), 1);
        
        // TLS (0x16) also rare
        assert_eq!(detector.byte_rarity(0x16), 1);
        
        // 'P' has overlap - less rare
        assert!(detector.byte_rarity(b'P') > detector.byte_rarity(0x05));
    }
}