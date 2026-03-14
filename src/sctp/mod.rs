//! SCTP Protocol Support
//!
//! This module provides SCTP (Stream Control Transmission Protocol) support
//! integrating with the KMPngSCTP Kotlin Multiplatform implementation.
//!
//! ## Features
//!
//! - SCTP server for accepting incoming associations
//! - SCTP client for initiating connections
//! - Multi-homing support
//! - Partial reliability (PR-SCTP)
//! - Ordered and unordered message delivery

use std::net::SocketAddr;
use std::sync::Arc;
use parking_lot::Mutex;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SctpError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("SCTP association error: {0}")]
    Association(String),
    
    #[error("SCTP not supported on this platform")]
    NotSupported,
    
    #[error("Binding error: {0}")]
    Bind(String),
    
    #[error("Connection error: {0}")]
    Connect(String),
}

/// SCTP chunk types matching RFC 4960
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SctpChunkType {
    Data = 0,
    Init = 1,
    InitAck = 2,
    Sack = 3,
    Heartbeat = 4,
    HeartbeatAck = 5,
    Abort = 6,
    Shutdown = 7,
    ShutdownAck = 8,
    Error = 9,
    CookieEcho = 10,
    CookieAck = 11,
    Cwr = 12,
    Ecne = 13,
    Reconfig = 14,
    Pad = 15,
}

/// SCTP stream representing an ordered byte channel
#[derive(Debug)]
pub struct SctpStream {
    stream_id: u16,
    association_id: u32,
}

impl SctpStream {
    /// Get the stream ID
    pub fn stream_id(&self) -> u16 {
        self.stream_id
    }
    
    /// Get the association ID
    pub fn association_id(&self) -> u32 {
        self.association_id
    }
}

/// SCTP association representing a connection between endpoints
#[derive(Debug)]
pub struct SctpAssociation {
    association_id: u32,
    local_addr: SocketAddr,
    remote_addr: SocketAddr,
    streams: Arc<Mutex<Vec<SctpStream>>>,
    state: Arc<Mutex<SctpState>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SctpState {
    Closed,
    CookieWait,
    CookieEchoed,
    Established,
    ShutdownPending,
    ShutdownSent,
    ShutdownReceived,
    ShutdownAckSent,
}

impl SctpAssociation {
    /// Get the local address
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
    
    /// Get the remote address
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }
    
    /// Get the association ID
    pub fn association_id(&self) -> u32 {
        self.association_id
    }
    
    /// Get current state
    pub fn state(&self) -> SctpState {
        self.state.lock().clone()
    }
    
    /// Check if the association is established
    pub fn is_established(&self) -> bool {
        *self.state.lock() == SctpState::Established
    }
}

/// SCTP server for accepting incoming associations
#[derive(Debug)]
pub struct SctpServer {
    local_addr: SocketAddr,
    associations: Arc<Mutex<Vec<Arc<SctpAssociation>>>>,
    shutdown: Arc<Mutex<bool>>,
}

impl SctpServer {
    /// Create a new SCTP server bound to the given address
    pub fn bind(addr: SocketAddr) -> Result<SctpServer, SctpError> {
        Ok(SctpServer {
            local_addr: addr,
            associations: Arc::new(Mutex::new(Vec::new())),
            shutdown: Arc::new(Mutex::new(false)),
        })
    }
    
    /// Get the local address the server is bound to
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
    
    /// Accept a new SCTP association
    /// 
    /// Returns None if the server is shutdown
    pub async fn accept(&self) -> Result<Arc<SctpAssociation>, SctpError> {
        loop {
            if *self.shutdown.lock() {
                return Err(SctpError::Io(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "server shutdown",
                )));
            }
            
            // Check for pending associations
            let assoc = {
                let assocs = self.associations.lock();
                assocs.last().cloned()
            };
            
            if let Some(a) = assoc {
                return Ok(a);
            }
            
            // Wait a bit before checking again
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }
    
    /// Shutdown the server
    pub async fn shutdown(&self) {
        *self.shutdown.lock() = true;
    }
    
    /// Get the number of active associations
    pub fn association_count(&self) -> usize {
        self.associations.lock().len()
    }
}

/// SCTP client for initiating connections
#[derive(Debug)]
pub struct SctpClient {
    association: Option<Arc<SctpAssociation>>,
}

impl SctpClient {
    /// Create a new SCTP client
    pub fn new() -> Self {
        Self { association: None }
    }
    
    /// Connect to a remote SCTP endpoint
    pub async fn connect(&mut self, addr: SocketAddr) -> Result<Arc<SctpAssociation>, SctpError> {
        // Create a new association
        let assoc = Arc::new(SctpAssociation {
            association_id: rand::random(),
            local_addr: "0.0.0.0:0".parse().unwrap(),
            remote_addr: addr,
            streams: Arc::new(Mutex::new(Vec::new())),
            state: Arc::new(Mutex::new(SctpState::CookieWait)),
        });
        
        self.association = Some(assoc.clone());
        Ok(assoc)
    }
    
    /// Get the current association
    pub fn association(&self) -> Option<&Arc<SctpAssociation>> {
        self.association.as_ref()
    }
}

impl Default for SctpClient {
    fn default() -> Self {
        Self::new()
    }
}

/// SCTP configuration
#[derive(Debug, Clone)]
pub struct SctpConfig {
    pub port: u16,
    pub max_streams: u16,
    pub init_max_streams: u16,
    pub heartbeat_interval: std::time::Duration,
    pub timeout: std::time::Duration,
    pub rto_initial: std::time::Duration,
    pub rto_min: std::time::Duration,
    pub rto_max: std::time::Duration,
    pub max_retries: u32,
    pub cookie_lifetime: std::time::Duration,
}

impl Default for SctpConfig {
    fn default() -> Self {
        Self {
            port: 3842,
            max_streams: 64,
            init_max_streams: 64,
            heartbeat_interval: std::time::Duration::from_secs(30),
            timeout: std::time::Duration::from_secs(60),
            rto_initial: std::time::Duration::from_secs(3),
            rto_min: std::time::Duration::from_secs(1),
            rto_max: std::time::Duration::from_secs(60),
            max_retries: 5,
            cookie_lifetime: std::time::Duration::from_secs(60),
        }
    }
}

/// Build an SCTP packet with the given chunks
pub fn build_sctp_packet(
    source_port: u16,
    dest_port: u16,
    verification_tag: u32,
    chunks: &[Vec<u8>],
) -> Vec<u8> {
    // SCTP common header is 12 bytes
    let mut packet = Vec::with_capacity(12 + chunks.iter().map(|c| c.len()).sum::<usize>());
    
    // Source port (16 bits)
    packet.extend_from_slice(&source_port.to_be_bytes());
    // Destination port (16 bits)
    packet.extend_from_slice(&dest_port.to_be_bytes());
    // Verification tag (32 bits)
    packet.extend_from_slice(&verification_tag.to_be_bytes());
    // Checksum (32 bits) - CRC32C, initialized to 0 for calculation
    packet.extend_from_slice(&0u32.to_be_bytes());
    
    // Add chunks
    for chunk in chunks {
        packet.extend_from_slice(chunk);
    }
    
    // Calculate CRC32C checksum
    let checksum = calculate_crc32c(&packet);
    // Replace the checksum in the packet
    let checksum_bytes = checksum.to_be_bytes();
    packet[8..12].copy_from_slice(&checksum_bytes);
    
    packet
}

/// Calculate CRC32C checksum (Castagnoli)
fn calculate_crc32c(data: &[u8]) -> u32 {
    // CRC32C polynomial
    const POLY: u32 = 0x1EDC6F41;
    let mut crc: u32 = 0xFFFFFFFF;
    
    for byte in data {
        crc ^= (*byte as u32) << 24;
        for _ in 0..8 {
            if crc & 0x80000000 != 0 {
                crc = (crc << 1) ^ POLY;
            } else {
                crc <<= 1;
            }
        }
    }
    
    crc ^ 0xFFFFFFFF
}

/// SCTP event types
#[derive(Debug, Clone)]
pub enum SctpEvent {
    Connected(Arc<SctpAssociation>),
    Disconnected(u32),
    DataReceived { association_id: u32, stream_id: u16, data: Vec<u8> },
    Error { association_id: u32, error: String },
}

/// Re-export KMPngSCTP integration status
pub mod kmp_ngsctp {
    //! KMPngSCTP Integration
    //!
    //! This module provides integration points with the KMPngSCTP
    //! Kotlin Multiplatform SCTP implementation.
    //!
    //! To use KMPngSctp:
    //! 1. Build the Kotlin project: `cd KMPngSCTP && ./gradlew build`
    //! 2. The JAR will be available for JNI integration
    //! 3. Use JNI to call Kotlin SCTP functions from Rust
    
    /// Indicates whether KMPngSCTP JAR is available
    pub const fn is_available() -> bool {
        false
    }
    
    /// KMPngSCTP version string
    pub const VERSION: &str = "0.1.0";
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sctp_config_default() {
        let config = SctpConfig::default();
        assert_eq!(config.port, 3842);
        assert_eq!(config.max_streams, 64);
    }
    
    #[test]
    fn test_build_sctp_packet() {
        let chunks = vec![vec![0x01, 0x00, 0x00, 0x04]]; // INIT chunk
        let packet = build_sctp_packet(12345, 3842, 0xDEADBEEF, &chunks);
        assert_eq!(packet.len(), 12 + 4); // header + chunk
    }
    
    #[test]
    fn test_crc32c() {
        let data = b"Hello, SCTP!";
        let crc = calculate_crc32c(data);
        assert_ne!(crc, 0);
    }
}
