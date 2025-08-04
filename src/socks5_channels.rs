// SOCKS5 Channelized Continuation Architecture
// 
// This module implements a channel-based continuation system for SOCKS5 protocol decoding.
// Unlike the monolithic approach in protocol_handlers.rs, this creates discrete testable stages
// where buffer data flows through continuation points that can suspend/resume at any decode step.

use std::io;
use std::collections::VecDeque;
use tokio::sync::mpsc;
use log::{debug, warn};

/// SOCKS5 Protocol State Machine with Continuation Points
#[derive(Debug, Clone, PartialEq)]
pub enum Socks5State {
    /// Initial state - expecting version + nmethods
    ExpectingHandshake,
    /// Waiting for method bytes after nmethods
    ExpectingMethods { nmethods: u8 },
    /// Authentication phase (if method 2 selected)
    ExpectingAuth,
    /// Waiting for auth username/password
    ExpectingAuthData { stage: AuthStage },
    /// Waiting for SOCKS5 request (VER CMD RSV ATYP)
    ExpectingRequest,
    /// Waiting for address data based on ATYP
    ExpectingAddress { atyp: u8, remaining: usize },
    /// Protocol complete - ready for relay
    Connected,
    /// Terminal error state
    Failed { reason: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum AuthStage {
    Username { ulen: u8 },
    Password { plen: u8 },
}

/// Buffer continuation represents a suspend point in decode process
#[derive(Debug)]
pub struct BufferContinuation {
    pub buffer: VecDeque<u8>,
    pub state: Socks5State,
    pub bytes_needed: usize,
    pub metadata: Socks5Metadata,
}

#[derive(Debug, Default)]
pub struct Socks5Metadata {
    pub version: Option<u8>,
    pub selected_method: Option<u8>,
    pub methods: Vec<u8>,
    pub command: Option<u8>,
    pub address_type: Option<u8>,
    pub target_address: Option<String>,
    pub auth_username: Option<String>,
}

/// Decode result from a channel stage
#[derive(Debug)]
pub enum DecodeResult {
    /// Need more bytes to continue
    NeedMoreData { bytes_needed: usize },
    /// Stage completed, advance to next state
    Advance { new_state: Socks5State, bytes_consumed: usize },
    /// Protocol violation detected
    ProtocolError { reason: String },
    /// Authentication required - send response and continue
    AuthRequired { response: Vec<u8> },
    /// Success response needed
    SuccessResponse { response: Vec<u8> },
}

impl BufferContinuation {
    pub fn new() -> Self {
        Self {
            buffer: VecDeque::new(),
            state: Socks5State::ExpectingHandshake,
            bytes_needed: 2, // Start with version + nmethods
            metadata: Socks5Metadata::default(),
        }
    }

    /// Feed bytes into the continuation buffer
    pub fn feed_bytes(&mut self, data: &[u8]) {
        self.buffer.extend(data);
    }

    /// Check if we have enough bytes for current decode stage
    pub fn can_decode(&self) -> bool {
        self.buffer.len() >= self.bytes_needed
    }

    /// Attempt to decode the current stage
    pub fn decode_stage(&mut self) -> DecodeResult {
        if !self.can_decode() {
            return DecodeResult::NeedMoreData { 
                bytes_needed: self.bytes_needed - self.buffer.len() 
            };
        }

        match &self.state {
            Socks5State::ExpectingHandshake => self.decode_handshake(),
            Socks5State::ExpectingMethods { nmethods } => self.decode_methods(*nmethods),
            Socks5State::ExpectingAuth => self.decode_auth_version(),
            Socks5State::ExpectingAuthData { stage } => self.decode_auth_data(stage.clone()),
            Socks5State::ExpectingRequest => self.decode_request(),
            Socks5State::ExpectingAddress { atyp, remaining } => self.decode_address(*atyp, *remaining),
            Socks5State::Connected => DecodeResult::ProtocolError { 
                reason: "Already connected".to_string() 
            },
            Socks5State::Failed { reason } => DecodeResult::ProtocolError { 
                reason: reason.clone() 
            },
        }
    }

    fn decode_handshake(&mut self) -> DecodeResult {
        let version = self.buffer[0];
        let nmethods = self.buffer[1];

        self.metadata.version = Some(version);

        if version != 0x05 {
            return DecodeResult::ProtocolError {
                reason: format!("Unsupported SOCKS version: {:#x}", version),
            };
        }

        if nmethods == 0 || nmethods > 255 {
            return DecodeResult::ProtocolError {
                reason: format!("Invalid methods count: {}", nmethods),
            };
        }

        // Consume version + nmethods
        self.buffer.drain(0..2);

        DecodeResult::Advance {
            new_state: Socks5State::ExpectingMethods { nmethods },
            bytes_consumed: 2,
        }
    }

    fn decode_methods(&mut self, nmethods: u8) -> DecodeResult {
        let nmethods = nmethods as usize;
        
        if self.buffer.len() < nmethods {
            return DecodeResult::NeedMoreData { 
                bytes_needed: nmethods - self.buffer.len() 
            };
        }

        // Extract methods
        let methods: Vec<u8> = self.buffer.drain(0..nmethods).collect();
        self.metadata.methods = methods.clone();

        // Select authentication method
        let selected_method = if methods.contains(&0) {
            0  // No authentication
        } else if methods.contains(&2) {
            2  // Username/password authentication
        } else {
            0xFF  // No acceptable methods
        };

        self.metadata.selected_method = Some(selected_method);

        let response = vec![0x05, selected_method];

        if selected_method == 0xFF {
            return DecodeResult::ProtocolError {
                reason: "No supported authentication methods".to_string(),
            };
        }

        let next_state = if selected_method == 2 {
            Socks5State::ExpectingAuth
        } else {
            Socks5State::ExpectingRequest
        };

        DecodeResult::AuthRequired { response }
    }

    fn decode_auth_version(&mut self) -> DecodeResult {
        let auth_version = self.buffer[0];
        
        if auth_version != 1 {
            return DecodeResult::ProtocolError {
                reason: format!("Invalid auth version: {}", auth_version),
            };
        }

        self.buffer.drain(0..1);

        DecodeResult::Advance {
            new_state: Socks5State::ExpectingAuthData { 
                stage: AuthStage::Username { ulen: 0 } 
            },
            bytes_consumed: 1,
        }
    }

    fn decode_auth_data(&mut self, stage: AuthStage) -> DecodeResult {
        match stage {
            AuthStage::Username { ulen: _ } => {
                // First get username length
                if self.buffer.is_empty() {
                    return DecodeResult::NeedMoreData { bytes_needed: 1 };
                }

                let ulen = self.buffer[0];
                self.buffer.drain(0..1);

                if self.buffer.len() < ulen as usize {
                    return DecodeResult::NeedMoreData { 
                        bytes_needed: ulen as usize - self.buffer.len() 
                    };
                }

                // Extract username
                let username_bytes: Vec<u8> = self.buffer.drain(0..ulen as usize).collect();
                let username = String::from_utf8_lossy(&username_bytes).to_string();
                self.metadata.auth_username = Some(username);

                DecodeResult::Advance {
                    new_state: Socks5State::ExpectingAuthData { 
                        stage: AuthStage::Password { plen: 0 } 
                    },
                    bytes_consumed: 1 + ulen as usize,
                }
            }
            AuthStage::Password { plen: _ } => {
                // Get password length
                if self.buffer.is_empty() {
                    return DecodeResult::NeedMoreData { bytes_needed: 1 };
                }

                let plen = self.buffer[0];
                self.buffer.drain(0..1);

                if self.buffer.len() < plen as usize {
                    return DecodeResult::NeedMoreData { 
                        bytes_needed: plen as usize - self.buffer.len() 
                    };
                }

                // Extract password (we accept any)
                let _password_bytes: Vec<u8> = self.buffer.drain(0..plen as usize).collect();

                // Send auth success
                let response = vec![0x01, 0x00]; // Success

                DecodeResult::SuccessResponse { response }
            }
        }
    }

    fn decode_request(&mut self) -> DecodeResult {
        if self.buffer.len() < 4 {
            return DecodeResult::NeedMoreData { 
                bytes_needed: 4 - self.buffer.len() 
            };
        }

        let version = self.buffer[0];
        let command = self.buffer[1];
        let _reserved = self.buffer[2];
        let atyp = self.buffer[3];

        if version != 0x05 {
            return DecodeResult::ProtocolError {
                reason: format!("Invalid request version: {:#x}", version),
            };
        }

        if command != 0x01 {
            return DecodeResult::ProtocolError {
                reason: format!("Unsupported command: {:#x}", command),
            };
        }

        self.metadata.command = Some(command);
        self.metadata.address_type = Some(atyp);

        // Consume request header
        self.buffer.drain(0..4);

        // Determine bytes needed for address
        let address_bytes = match atyp {
            0x01 => 6, // IPv4 (4 bytes) + port (2 bytes)
            0x03 => {
                // Domain name - need to read length first
                if self.buffer.is_empty() {
                    return DecodeResult::NeedMoreData { bytes_needed: 1 };
                }
                let domain_len = self.buffer[0] as usize;
                1 + domain_len + 2 // length + domain + port
            }
            0x04 => 18, // IPv6 (16 bytes) + port (2 bytes)
            _ => {
                return DecodeResult::ProtocolError {
                    reason: format!("Unsupported address type: {:#x}", atyp),
                };
            }
        };

        DecodeResult::Advance {
            new_state: Socks5State::ExpectingAddress { atyp, remaining: address_bytes },
            bytes_consumed: 4,
        }
    }

    fn decode_address(&mut self, atyp: u8, remaining: usize) -> DecodeResult {
        if self.buffer.len() < remaining {
            return DecodeResult::NeedMoreData { 
                bytes_needed: remaining - self.buffer.len() 
            };
        }

        let target_address = match atyp {
            0x01 => {
                // IPv4
                let ip_bytes: Vec<u8> = self.buffer.drain(0..4).collect();
                let port_bytes: Vec<u8> = self.buffer.drain(0..2).collect();
                let ip = std::net::Ipv4Addr::new(ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]);
                let port = u16::from_be_bytes([port_bytes[0], port_bytes[1]]);
                format!("{}:{}", ip, port)
            }
            0x03 => {
                // Domain name
                let domain_len = self.buffer.pop_front().unwrap() as usize;
                let domain_bytes: Vec<u8> = self.buffer.drain(0..domain_len).collect();
                let port_bytes: Vec<u8> = self.buffer.drain(0..2).collect();
                let domain = String::from_utf8_lossy(&domain_bytes);
                let port = u16::from_be_bytes([port_bytes[0], port_bytes[1]]);
                format!("{}:{}", domain, port)
            }
            0x04 => {
                // IPv6
                let ip_bytes: Vec<u8> = self.buffer.drain(0..16).collect();
                let port_bytes: Vec<u8> = self.buffer.drain(0..2).collect();
                let ip = std::net::Ipv6Addr::from([
                    ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3],
                    ip_bytes[4], ip_bytes[5], ip_bytes[6], ip_bytes[7],
                    ip_bytes[8], ip_bytes[9], ip_bytes[10], ip_bytes[11],
                    ip_bytes[12], ip_bytes[13], ip_bytes[14], ip_bytes[15],
                ]);
                let port = u16::from_be_bytes([port_bytes[0], port_bytes[1]]);
                format!("[{}]:{}", ip, port)
            }
            _ => unreachable!(),
        };

        self.metadata.target_address = Some(target_address);

        DecodeResult::Advance {
            new_state: Socks5State::Connected,
            bytes_consumed: remaining,
        }
    }

    /// Process one complete decode cycle
    pub fn process_cycle(&mut self) -> DecodeResult {
        let result = self.decode_stage();

        match &result {
            DecodeResult::Advance { new_state, .. } => {
                self.state = new_state.clone();
                // Update bytes_needed for new state
                self.bytes_needed = match new_state {
                    Socks5State::ExpectingHandshake => 2,
                    Socks5State::ExpectingMethods { nmethods } => *nmethods as usize,
                    Socks5State::ExpectingAuth => 1,
                    Socks5State::ExpectingAuthData { .. } => 1,
                    Socks5State::ExpectingRequest => 4,
                    Socks5State::ExpectingAddress { remaining, .. } => *remaining,
                    Socks5State::Connected => 0,
                    Socks5State::Failed { .. } => 0,
                };
            }
            DecodeResult::ProtocolError { reason } => {
                self.state = Socks5State::Failed { reason: reason.clone() };
            }
            _ => {}
        }

        result
    }
}

/// Channelized SOCKS5 processor that manages continuation state
pub struct Socks5ChannelProcessor {
    continuation: BufferContinuation,
    response_sender: mpsc::UnboundedSender<Vec<u8>>,
    response_receiver: mpsc::UnboundedReceiver<Vec<u8>>,
}

impl Socks5ChannelProcessor {
    pub fn new() -> Self {
        let (response_sender, response_receiver) = mpsc::unbounded_channel();
        
        Self {
            continuation: BufferContinuation::new(),
            response_sender,
            response_receiver,
        }
    }

    /// Feed data into the processor and get next actions
    pub fn feed_data(&mut self, data: &[u8]) -> Vec<ProcessorAction> {
        self.continuation.feed_bytes(data);
        self.process_available_data()
    }

    /// Process all available data in buffer
    fn process_available_data(&mut self) -> Vec<ProcessorAction> {
        let mut actions = Vec::new();

        loop {
            match self.continuation.process_cycle() {
                DecodeResult::NeedMoreData { bytes_needed } => {
                    actions.push(ProcessorAction::NeedMoreData { bytes_needed });
                    break;
                }
                DecodeResult::Advance { new_state, bytes_consumed } => {
                    actions.push(ProcessorAction::StateAdvanced { 
                        new_state: new_state.clone(), 
                        bytes_consumed 
                    });
                    
                    if matches!(new_state, Socks5State::Connected) {
                        actions.push(ProcessorAction::ReadyForRelay {
                            target: self.continuation.metadata.target_address.clone().unwrap_or_default(),
                        });
                        break;
                    }
                }
                DecodeResult::AuthRequired { response } => {
                    actions.push(ProcessorAction::SendResponse { data: response });
                    
                    // Advance to next state
                    self.continuation.state = if self.continuation.metadata.selected_method == Some(2) {
                        Socks5State::ExpectingAuth
                    } else {
                        Socks5State::ExpectingRequest
                    };
                }
                DecodeResult::SuccessResponse { response } => {
                    actions.push(ProcessorAction::SendResponse { data: response });
                    self.continuation.state = Socks5State::ExpectingRequest;
                }
                DecodeResult::ProtocolError { reason } => {
                    actions.push(ProcessorAction::ProtocolError { reason });
                    break;
                }
            }
        }

        actions
    }

    /// Get current state for inspection
    pub fn current_state(&self) -> &Socks5State {
        &self.continuation.state
    }

    /// Get metadata collected so far
    pub fn metadata(&self) -> &Socks5Metadata {
        &self.continuation.metadata
    }

    /// Get remaining buffer for inspection
    pub fn buffer_len(&self) -> usize {
        self.continuation.buffer.len()
    }
}

/// Actions that the processor wants the handler to take
#[derive(Debug)]
pub enum ProcessorAction {
    /// Need more bytes from network
    NeedMoreData { bytes_needed: usize },
    /// State machine advanced
    StateAdvanced { new_state: Socks5State, bytes_consumed: usize },
    /// Send response to client
    SendResponse { data: Vec<u8> },
    /// Protocol completed successfully, ready for relay
    ReadyForRelay { target: String },
    /// Protocol error occurred
    ProtocolError { reason: String },
}

pub fn create_socks5_success_response(local_addr: std::net::SocketAddr) -> Vec<u8> {
    let mut response = vec![0x05, 0x00, 0x00]; // Success
    
    match local_addr {
        std::net::SocketAddr::V4(addr) => {
            response.push(0x01); // IPv4
            response.extend_from_slice(&addr.ip().octets());
            response.extend_from_slice(&addr.port().to_be_bytes());
        }
        std::net::SocketAddr::V6(addr) => {
            response.push(0x04); // IPv6
            response.extend_from_slice(&addr.ip().octets());
            response.extend_from_slice(&addr.port().to_be_bytes());
        }
    }
    
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handshake_decode() {
        let mut continuation = BufferContinuation::new();
        
        // Feed SOCKS5 handshake: version=5, nmethods=1
        continuation.feed_bytes(&[0x05, 0x01]);
        
        let result = continuation.process_cycle();
        match result {
            DecodeResult::Advance { new_state, bytes_consumed } => {
                assert_eq!(bytes_consumed, 2);
                assert_eq!(new_state, Socks5State::ExpectingMethods { nmethods: 1 });
                assert_eq!(continuation.metadata.version, Some(0x05));
            }
            _ => panic!("Expected Advance result"),
        }
    }

    #[test]
    fn test_methods_decode() {
        let mut continuation = BufferContinuation::new();
        continuation.state = Socks5State::ExpectingMethods { nmethods: 1 };
        continuation.bytes_needed = 1;
        
        // Feed method: no auth
        continuation.feed_bytes(&[0x00]);
        
        let result = continuation.process_cycle();
        match result {
            DecodeResult::AuthRequired { response } => {
                assert_eq!(response, vec![0x05, 0x00]); // SOCKS5, no auth
                assert_eq!(continuation.metadata.methods, vec![0x00]);
                assert_eq!(continuation.metadata.selected_method, Some(0x00));
            }
            _ => panic!("Expected AuthRequired result"),
        }
    }

    #[test]
    fn test_request_decode() {
        let mut continuation = BufferContinuation::new();
        continuation.state = Socks5State::ExpectingRequest;
        continuation.bytes_needed = 4;
        
        // Feed CONNECT to IPv4
        continuation.feed_bytes(&[0x05, 0x01, 0x00, 0x01]);
        
        let result = continuation.process_cycle();
        match result {
            DecodeResult::Advance { new_state, bytes_consumed } => {
                assert_eq!(bytes_consumed, 4);
                assert_eq!(new_state, Socks5State::ExpectingAddress { atyp: 1, remaining: 6 });
                assert_eq!(continuation.metadata.command, Some(0x01));
                assert_eq!(continuation.metadata.address_type, Some(0x01));
            }
            _ => panic!("Expected Advance result"),
        }
    }

    #[test]
    fn test_ipv4_address_decode() {
        let mut continuation = BufferContinuation::new();
        continuation.state = Socks5State::ExpectingAddress { atyp: 1, remaining: 6 };
        continuation.bytes_needed = 6;
        
        // Feed IPv4 address 192.168.1.1:80
        continuation.feed_bytes(&[192, 168, 1, 1, 0, 80]);
        
        let result = continuation.process_cycle();
        match result {
            DecodeResult::Advance { new_state, bytes_consumed } => {
                assert_eq!(bytes_consumed, 6);
                assert_eq!(new_state, Socks5State::Connected);
                assert_eq!(continuation.metadata.target_address, Some("192.168.1.1:80".to_string()));
            }
            _ => panic!("Expected Advance result"),
        }
    }

    #[test]
    fn test_domain_address_decode() {
        let mut continuation = BufferContinuation::new();
        continuation.state = Socks5State::ExpectingAddress { atyp: 3, remaining: 0 };
        continuation.bytes_needed = 0;
        
        // Feed domain name "example.com:443"
        let domain = b"example.com";
        let mut data = vec![domain.len() as u8];
        data.extend_from_slice(domain);
        data.extend_from_slice(&443u16.to_be_bytes());
        
        continuation.feed_bytes(&data);
        
        // Need to recalculate remaining bytes after seeing domain length
        continuation.state = Socks5State::ExpectingAddress { atyp: 3, remaining: data.len() };
        continuation.bytes_needed = data.len();
        
        let result = continuation.process_cycle();
        match result {
            DecodeResult::Advance { new_state, .. } => {
                assert_eq!(new_state, Socks5State::Connected);
                assert_eq!(continuation.metadata.target_address, Some("example.com:443".to_string()));
            }
            _ => panic!("Expected Advance result"),
        }
    }

    #[test]
    fn test_processor_complete_flow() {
        let mut processor = Socks5ChannelProcessor::new();
        
        // Complete SOCKS5 no-auth handshake
        let handshake = [0x05, 0x01, 0x00]; // Version 5, 1 method, no auth
        let actions1 = processor.feed_data(&handshake);
        
        // Should get auth response
        assert!(actions1.iter().any(|a| matches!(a, ProcessorAction::SendResponse { .. })));
        
        // Send request
        let request = [
            0x05, 0x01, 0x00, 0x01,  // SOCKS5 CONNECT IPv4
            192, 168, 1, 1,          // 192.168.1.1
            0, 80,                   // Port 80
        ];
        let actions2 = processor.feed_data(&request);
        
        // Should be ready for relay
        assert!(actions2.iter().any(|a| matches!(a, ProcessorAction::ReadyForRelay { .. })));
        assert_eq!(processor.current_state(), &Socks5State::Connected);
    }

    #[test]
    fn test_incremental_feeding() {
        let mut processor = Socks5ChannelProcessor::new();
        
        // Feed data byte by byte
        let full_handshake = [0x05, 0x01, 0x00, 0x05, 0x01, 0x00, 0x01, 192, 168, 1, 1, 0, 80];
        
        for &byte in &full_handshake {
            let actions = processor.feed_data(&[byte]);
            
            // Should either need more data or advance
            for action in actions {
                match action {
                    ProcessorAction::NeedMoreData { .. } => {
                        // Expected during incremental feeding
                    }
                    ProcessorAction::StateAdvanced { .. } => {
                        // Expected when stage completes
                    }
                    ProcessorAction::SendResponse { .. } => {
                        // Expected for auth response
                    }
                    ProcessorAction::ReadyForRelay { .. } => {
                        // Expected at end
                        assert_eq!(processor.current_state(), &Socks5State::Connected);
                        return; // Test passed
                    }
                    ProcessorAction::ProtocolError { .. } => {
                        panic!("Unexpected protocol error during incremental feeding");
                    }
                }
            }
        }
    }
}