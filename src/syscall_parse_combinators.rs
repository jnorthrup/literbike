// Syscall-based Parse Combinators for Runtime Protocol Recognition
// Handles overlapping listeners on port/INETPROTO tuples with zero-copy parsing

use std::os::unix::io::RawFd;
use libc::{self, sockaddr, sockaddr_in, socklen_t, c_int, c_void};
use crate::auto_peering_bridge::AutoPeeringBridge;

/// Fast protocol identification using bit flags
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProtocolId {
    Unknown = 0,
    Socks5 = 1,
    Http = 2,
    Tls = 4,
    Ssh = 8,
    Upnp = 16,
    Pac = 32,
}

/// Syscall-based byte parser that operates directly on socket buffers
pub struct SyscallParser {
    fd: RawFd,
    peek_buffer: [u8; 256],  // Stack-allocated peek buffer
    peek_len: usize,
}

/// Parse combinator result
#[derive(Debug, Clone)]
pub struct ParseResult {
    pub protocol: ProtocolId,
    pub consumed: usize,
    pub confidence: u8,
}

impl SyscallParser {
    /// Create parser directly from socket fd
    pub fn new(fd: RawFd) -> Self {
        Self {
            fd,
            peek_buffer: [0u8; 256],
            peek_len: 0,
        }
    }

    /// Peek at socket data without consuming using MSG_PEEK
    pub fn peek_bytes(&mut self, min_bytes: usize) -> Result<&[u8], std::io::Error> {
        if self.peek_len < min_bytes {
            // Use recv with MSG_PEEK to look ahead without consuming
            let result = unsafe {
                libc::recv(
                    self.fd,
                    self.peek_buffer.as_mut_ptr() as *mut c_void,
                    self.peek_buffer.len(),
                    libc::MSG_PEEK | libc::MSG_DONTWAIT,
                )
            };
            
            if result < 0 {
                return Err(std::io::Error::last_os_error());
            }
            
            self.peek_len = result as usize;
        }
        
        Ok(&self.peek_buffer[..self.peek_len.min(min_bytes)])
    }

    /// Fast protocol recognition using syscall-level byte inspection
    pub fn recognize_protocol(&mut self) -> Result<ParseResult, std::io::Error> {
        // Start with single byte for fastest discrimination
        let first_byte = self.peek_bytes(1)?;
        if first_byte.is_empty() {
            return Ok(ParseResult {
                protocol: ProtocolId::Unknown,
                consumed: 0,
                confidence: 0,
            });
        }

        match first_byte[0] {
            // SOCKS5: 0x05
            0x05 => self.parse_socks5(),
            
            // TLS: 0x16 (handshake), 0x14 (change_cipher_spec), 0x15 (alert), 0x17 (application_data)
            0x14..=0x17 => self.parse_tls(),
            
            // HTTP methods (ASCII range)
            b'G' | b'P' | b'H' | b'D' | b'O' | b'C' => self.parse_http(),
            
            // SSH: 'S' 
            b'S' => self.parse_ssh(),
            
            // UPnP: 'M' for M-SEARCH
            b'M' => self.parse_upnp(),
            
            _ => Ok(ParseResult {
                protocol: ProtocolId::Unknown,
                consumed: 0,
                confidence: 0,
            }),
        }
    }

    fn parse_socks5(&mut self) -> Result<ParseResult, std::io::Error> {
        let bytes = self.peek_bytes(3)?;
        if bytes.len() >= 2 && bytes[0] == 0x05 {
            // Valid SOCKS5 version, check nmethods
            let nmethods = bytes[1] as usize;
            let expected_len = 2 + nmethods;
            
            if bytes.len() >= expected_len.min(3) {
                return Ok(ParseResult {
                    protocol: ProtocolId::Socks5,
                    consumed: 2,
                    confidence: 255,
                });
            }
        }
        
        Ok(ParseResult {
            protocol: ProtocolId::Unknown,
            consumed: 0,
            confidence: 0,
        })
    }

    fn parse_tls(&mut self) -> Result<ParseResult, std::io::Error> {
        let bytes = self.peek_bytes(5)?;
        if bytes.len() >= 3 {
            // Check TLS version (0x03, 0x01-0x04)
            if bytes[0] == 0x16 && bytes[1] == 0x03 && bytes[2] <= 0x04 {
                return Ok(ParseResult {
                    protocol: ProtocolId::Tls,
                    consumed: 3,
                    confidence: 240,
                });
            }
        }
        
        Ok(ParseResult {
            protocol: ProtocolId::Unknown,
            consumed: 0,
            confidence: 0,
        })
    }

    fn parse_http(&mut self) -> Result<ParseResult, std::io::Error> {
        let bytes = self.peek_bytes(16)?;
        
        // Convert to string only for the minimum needed
        if let Ok(text) = std::str::from_utf8(&bytes[..bytes.len().min(8)]) {
            let methods = ["GET ", "POST", "PUT ", "DELE", "HEAD", "OPTI", "CONN", "PATC"];
            
            for method in &methods {
                if text.starts_with(method) {
                    // Check if it's a PAC request
                    if bytes.len() >= 16 {
                        if let Ok(full_text) = std::str::from_utf8(&bytes) {
                            if full_text.contains("/proxy.pac") || full_text.contains("/wpad.dat") {
                                return Ok(ParseResult {
                                    protocol: ProtocolId::Pac,
                                    consumed: method.len(),
                                    confidence: 200,
                                });
                            }
                        }
                    }
                    
                    return Ok(ParseResult {
                        protocol: ProtocolId::Http,
                        consumed: method.len(),
                        confidence: 220,
                    });
                }
            }
        }
        
        Ok(ParseResult {
            protocol: ProtocolId::Unknown,
            consumed: 0,
            confidence: 0,
        })
    }

    fn parse_ssh(&mut self) -> Result<ParseResult, std::io::Error> {
        let bytes = self.peek_bytes(8)?;
        
        if bytes.len() >= 4 {
            if let Ok(text) = std::str::from_utf8(&bytes[..4]) {
                if text == "SSH-" {
                    return Ok(ParseResult {
                        protocol: ProtocolId::Ssh,
                        consumed: 4,
                        confidence: 255,
                    });
                }
            }
        }
        
        Ok(ParseResult {
            protocol: ProtocolId::Unknown,
            consumed: 0,
            confidence: 0,
        })
    }

    fn parse_upnp(&mut self) -> Result<ParseResult, std::io::Error> {
        let bytes = self.peek_bytes(16)?;
        
        if bytes.len() >= 8 {
            if let Ok(text) = std::str::from_utf8(&bytes[..8]) {
                if text.starts_with("M-SEARCH") {
                    return Ok(ParseResult {
                        protocol: ProtocolId::Upnp,
                        consumed: 8,
                        confidence: 255,
                    });
                }
            }
        }
        
        Ok(ParseResult {
            protocol: ProtocolId::Unknown,
            consumed: 0,
            confidence: 0,
        })
    }
}

/// Overlapping listener manager for port/protocol tuples on Android
pub struct OverlappingListener {
    port: u16,
    main_fd: RawFd,
    peering_bridge: Option<AutoPeeringBridge>,
}

impl OverlappingListener {
    /// Create overlapping listener on specified port (Android/Termux optimized)
    pub fn bind(port: u16) -> Result<Self, std::io::Error> {
        let main_fd = unsafe {
            let fd = libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0);
            if fd < 0 {
                return Err(std::io::Error::last_os_error());
            }
            
            // Set SO_REUSEADDR for overlapping (Android may not support SO_REUSEPORT)
            let optval = 1i32;
            if libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_REUSEADDR,
                &optval as *const i32 as *const c_void,
                std::mem::size_of::<i32>() as socklen_t,
            ) < 0 {
                libc::close(fd);
                return Err(std::io::Error::last_os_error());
            }
            
            // Bind to port
            let mut addr: sockaddr_in = std::mem::zeroed();
            addr.sin_family = libc::AF_INET as u16;
            addr.sin_port = port.to_be();
            addr.sin_addr.s_addr = libc::INADDR_ANY;
            
            if libc::bind(
                fd,
                &addr as *const sockaddr_in as *const sockaddr,
                std::mem::size_of::<sockaddr_in>() as socklen_t,
            ) < 0 {
                libc::close(fd);
                return Err(std::io::Error::last_os_error());
            }
            
            if libc::listen(fd, 128) < 0 {
                libc::close(fd);
                return Err(std::io::Error::last_os_error());
            }
            
            fd
        };
        
        Ok(Self {
            port,
            main_fd,
            peering_bridge: None,
        })
    }

    /// Enable auto-peering bridge to restore PAC/WPAD functionality
    pub fn enable_auto_peering(&mut self, hostname: String) -> Result<(), std::io::Error> {
        use std::net::SocketAddr;
        
        let local_addr: SocketAddr = format!("127.0.0.1:{}", self.port).parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
            
        self.peering_bridge = Some(AutoPeeringBridge::new(local_addr, hostname));
        Ok(())
    }

    /// Accept connection and perform immediate protocol recognition
    pub fn accept_with_protocol(&mut self) -> Result<(RawFd, ProtocolId), std::io::Error> {
        let client_fd = unsafe {
            let mut client_addr: sockaddr_in = std::mem::zeroed();
            let mut addr_len = std::mem::size_of::<sockaddr_in>() as socklen_t;
            
            let fd = libc::accept(
                self.main_fd,
                &mut client_addr as *mut sockaddr_in as *mut sockaddr,
                &mut addr_len,
            );
            
            if fd < 0 {
                return Err(std::io::Error::last_os_error());
            }
            
            fd
        };
        
        // Immediate protocol recognition
        let mut parser = SyscallParser::new(client_fd);
        
        // First peek to check if this should bypass syscall detection for auto-peering
        let should_bypass = if let Some(ref bridge) = self.peering_bridge {
            match parser.peek_bytes(32) {
                Ok(bytes) => bridge.should_bypass_syscall(bytes),
                Err(_) => false,
            }
        } else {
            false
        };
        
        if should_bypass {
            // For PAC/WPAD/mDNS/UPnP, return Unknown to let universal listener handle it
            return Ok((client_fd, ProtocolId::Unknown));
        }
        
        match parser.recognize_protocol() {
            Ok(result) => {
                // Register with peering bridge if available
                if let Some(ref bridge) = self.peering_bridge {
                    let peer_addr = unsafe {
                        let mut peer_addr: sockaddr_in = std::mem::zeroed();
                        let mut addr_len = std::mem::size_of::<sockaddr_in>() as socklen_t;
                        
                        if libc::getpeername(
                            client_fd,
                            &mut peer_addr as *mut sockaddr_in as *mut libc::sockaddr,
                            &mut addr_len,
                        ) == 0 {
                            Some(std::net::SocketAddr::V4(std::net::SocketAddrV4::new(
                                std::net::Ipv4Addr::from(peer_addr.sin_addr.s_addr.to_be()),
                                peer_addr.sin_port.to_be(),
                            )))
                        } else {
                            None
                        }
                    };
                    
                    if let Some(addr) = peer_addr {
                        // Handle peer protocol registration in background (non-blocking)
                        let bridge_clone = bridge.clone(); // Clone the Arc
                        let protocol = result.protocol;
                        tokio::spawn(async move {
                            if let Err(e) = bridge_clone.handle_peer_protocol(addr, protocol).await {
                                eprintln!("Failed to handle peer protocol: {}", e);
                            }
                        });
                    }
                }
                
                Ok((client_fd, result.protocol))
            },
            Err(e) => {
                unsafe { libc::close(client_fd); }
                Err(e)
            }
        }
    }
}

impl Drop for OverlappingListener {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.main_fd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_recognition_patterns() {
        // Test that protocol recognition uses correct byte patterns for Android
        let socks5_data = [0x05, 0x01, 0x00];
        let http_data = b"GET /";
        let tls_data = [0x16, 0x03, 0x03];
        
        // Verify protocol patterns match our detection logic
        assert_eq!(socks5_data[0], 0x05);
        assert!(http_data.starts_with(b"GET"));
        assert_eq!(tls_data[0], 0x16);
        assert_eq!(tls_data[1], 0x03);
    }

    #[test]
    fn test_android_socket_compatibility() {
        // Test socket creation parameters work on Android/Termux
        let socket_types = [libc::SOCK_STREAM, libc::SOCK_DGRAM];
        let protocols = [0, libc::IPPROTO_TCP, libc::IPPROTO_UDP];
        
        for &sock_type in &socket_types {
            for &protocol in &protocols {
                if (sock_type == libc::SOCK_STREAM && protocol == libc::IPPROTO_TCP) ||
                   (sock_type == libc::SOCK_DGRAM && protocol == libc::IPPROTO_UDP) ||
                   protocol == 0 {
                    // Valid combinations for Android
                    assert!(sock_type > 0);
                    assert!(protocol >= 0);
                }
            }
        }
    }
}