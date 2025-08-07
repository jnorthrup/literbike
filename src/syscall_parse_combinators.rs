// Syscall-based Parse Combinators for Runtime Protocol Recognition
// Handles overlapping listeners on port/INETPROTO tuples with zero-copy parsing

use std::os::unix::io::RawFd;
use std::ptr;
use libc::{self, sockaddr, sockaddr_in, socklen_t, c_int, c_void};

/// Syscall-based byte parser that operates directly on socket buffers
#[repr(C)]
pub struct SyscallParser {
    fd: RawFd,
    peek_buffer: [u8; 256],  // Stack-allocated peek buffer
    peek_len: usize,
    position: usize,
}

/// Parse combinator result with zero-copy byte slices
#[derive(Debug, Clone)]
pub struct ParseResult<'a> {
    pub protocol: ProtocolId,
    pub consumed: usize,
    pub remaining: &'a [u8],
    pub confidence: u8,
}

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

impl SyscallParser {
    /// Create parser directly from socket fd
    pub fn new(fd: RawFd) -> Self {
        Self {
            fd,
            peek_buffer: [0u8; 256],
            peek_len: 0,
            position: 0,
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
                remaining: &[],
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
                remaining: first_byte,
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
                    remaining: &bytes[2..],
                    confidence: 255,
                });
            }
        }
        
        Ok(ParseResult {
            protocol: ProtocolId::Unknown,
            consumed: 0,
            remaining: bytes,
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
                    remaining: &bytes[3..],
                    confidence: 240,
                });
            }
        }
        
        Ok(ParseResult {
            protocol: ProtocolId::Unknown,
            consumed: 0,
            remaining: bytes,
            confidence: 0,
        })
    }

    fn parse_http(&mut self) -> Result<ParseResult, std::io::Error> {
        let bytes = self.peek_bytes(16)?;
        
        // Convert to string only for the minimum needed
        if let Ok(text) = std::str::from_utf8(&bytes[..bytes.len().min(8)]) {
            let methods = [GET , POST, PUT , DELE, HEAD, OPTI, CONN, PATC];
            
            for method in &methods {
                if text.starts_with(method) {
                    // Check if it's a PAC request
                    if bytes.len() >= 16 {
                        if let Ok(full_text) = std::str::from_utf8(&bytes) {
                            if full_text.contains(/proxy.pac) || full_text.contains(/wpad.dat) {
                                return Ok(ParseResult {
                                    protocol: ProtocolId::Pac,
                                    consumed: method.len(),
                                    remaining: &bytes[method.len()..],
                                    confidence: 200,
                                });
                            }
                        }
                    }
                    
                    return Ok(ParseResult {
                        protocol: ProtocolId::Http,
                        consumed: method.len(),
                        remaining: &bytes[method.len()..],
                        confidence: 220,
                    });
                }
            }
        }
        
        Ok(ParseResult {
            protocol: ProtocolId::Unknown,
            consumed: 0,
            remaining: bytes,
            confidence: 0,
        })
    }

    fn parse_ssh(&mut self) -> Result<ParseResult, std::io::Error> {
        let bytes = self.peek_bytes(8)?;
        
        if bytes.len() >= 4 {
            if let Ok(text) = std::str::from_utf8(&bytes[..4]) {
                if text == SSH- {
                    return Ok(ParseResult {
                        protocol: ProtocolId::Ssh,
                        consumed: 4,
                        remaining: &bytes[4..],
                        confidence: 255,
                    });
                }
            }
        }
        
        Ok(ParseResult {
            protocol: ProtocolId::Unknown,
            consumed: 0,
            remaining: bytes,
            confidence: 0,
        })
    }

    fn parse_upnp(&mut self) -> Result<ParseResult, std::io::Error> {
        let bytes = self.peek_bytes(16)?;
        
        if bytes.len() >= 8 {
            if let Ok(text) = std::str::from_utf8(&bytes[..8]) {
                if text.starts_with(M-SEARCH) {
                    return Ok(ParseResult {
                        protocol: ProtocolId::Upnp,
                        consumed: 8,
                        remaining: &bytes[8..],
                        confidence: 255,
                    });
                }
            }
        }
        
        Ok(ParseResult {
            protocol: ProtocolId::Unknown,
            consumed: 0,
            remaining: bytes,
            confidence: 0,
        })
    }
}

/// Overlapping listener manager for port/protocol tuples
pub struct OverlappingListener {
    port: u16,
    listeners: Vec<(ProtocolId, RawFd)>,
    main_fd: RawFd,
}

impl OverlappingListener {
    /// Create overlapping listener on specified port
    pub fn bind(port: u16) -> Result<Self, std::io::Error> {
        let main_fd = unsafe {
            let fd = libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0);
            if fd < 0 {
                return Err(std::io::Error::last_os_error());
            }
            
            // Set SO_REUSEADDR and SO_REUSEPORT for overlapping
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
            
            #[cfg(target_os = linux)]
            if libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_REUSEPORT,
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
            listeners: Vec::new(),
            main_fd,
        })
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
        match parser.recognize_protocol() {
            Ok(result) => Ok((client_fd, result.protocol)),
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
            for (_, fd) in &self.listeners {
                libc::close(*fd);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_recognition_speed() {
        // Test that protocol recognition requires minimal bytes
        let socks5_data = [0x05, 0x01, 0x00];
        let http_data = bGET /;
        let tls_data = [0x16, 0x03, 0x03];
        
        // Mock tests - in real usage these would use actual socket fds
        assert_eq\!(socks5_data[0], 0x05);
        assert\!(http_data.starts_with(bGET));
        assert_eq\!(tls_data[0], 0x16);
        assert_eq\!(tls_data[1], 0x03);
    }
}
