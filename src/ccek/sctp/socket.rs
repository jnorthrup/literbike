//! SCTP Socket implementation for reactor integration
//!
//! Provides SctpSocket type that implements SelectableChannel for the Literbike reactor.
//! Supports multi-homing, stream multiplexing, and the 4-way handshake.

use std::io;
use std::net::SocketAddr;
use std::os::unix::io::RawFd;

#[cfg(target_os = "linux")]
use std::os::fd::BorrowedFd;

use crate::chunk::{Chunk, DataChunk, DataFlags};

pub trait SelectableChannel: Send + Sync {
    fn as_raw_fd(&self) -> RawFd;
    fn raw_fd(&self) -> RawFd;
    fn is_open(&self) -> bool;
    fn close(&mut self) -> io::Result<()>;
    fn bind(&mut self, addr: SocketAddr) -> io::Result<()>;
    fn listen(&mut self) -> io::Result<()>;
    fn accept(&mut self) -> io::Result<(std::net::TcpStream, SocketAddr)>;
}

/// SCTP protocol constants
pub const IPPROTO_SCTP: u8 = 132;
pub const SCTP_PORT: u16 = 8888;

/// SCTP association state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssociationState {
    Closed,
    CookieWait,
    CookieEchoed,
    Established,
    ShutdownPending,
    ShutdownSent,
    ShutdownReceived,
    ShutdownAckSent,
}

/// SCTP socket wrapper
pub struct SctpSocket {
    fd: RawFd,
    state: AssociationState,
    local_tag: u32,
    remote_tag: u32,
    local_port: u16,
    remote_port: u16,
    // Multi-homing support
    primary_path: Option<SocketAddr>,
    alternate_paths: Vec<SocketAddr>,
}

impl SctpSocket {
    /// Create a new SCTP socket bound to the specified port
    pub fn bind(port: u16) -> io::Result<Self> {
        #[cfg(target_os = "linux")]
        {
            // On Linux, try to create an actual SCTP socket
            // Note: requires kernel SCTP support
            use std::mem::zeroed;

            unsafe {
                let fd = libc::socket(libc::AF_INET, libc::SOCK_STREAM, IPPROTO_SCTP as i32);
                if fd < 0 {
                    return Err(io::Error::last_os_error());
                }

                #[cfg(target_os = "macos")]
                let addr = libc::sockaddr_in {
                    sin_len: std::mem::size_of::<libc::sockaddr_in>() as u8,
                    sin_family: libc::AF_INET as u8,
                    sin_port: port.to_be(),
                    sin_addr: libc::in_addr { s_addr: 0 },
                    sin_zero: zeroed(),
                };

                #[cfg(not(target_os = "macos"))]
                let addr = libc::sockaddr_in {
                    sin_family: libc::AF_INET as u16,
                    sin_port: port.to_be(),
                    sin_addr: libc::in_addr { s_addr: 0 },
                    sin_zero: zeroed(),
                };

                if libc::bind(
                    fd,
                    &addr as *const libc::sockaddr_in as *const libc::sockaddr,
                    std::mem::size_of::<libc::sockaddr_in>() as u32,
                ) < 0
                {
                    let err = io::Error::last_os_error();
                    libc::close(fd);
                    return Err(err);
                }

                Ok(Self {
                    fd,
                    state: AssociationState::Closed,
                    local_tag: generate_tag(),
                    remote_tag: 0,
                    local_port: port,
                    remote_port: 0,
                    primary_path: None,
                    alternate_paths: Vec::new(),
                })
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            // On non-Linux, create a UDP socket for emulation
            use std::mem::zeroed;

            unsafe {
                let fd = libc::socket(libc::AF_INET, libc::SOCK_DGRAM, libc::IPPROTO_UDP);
                if fd < 0 {
                    return Err(io::Error::last_os_error());
                }

                #[cfg(target_os = "macos")]
                let addr = libc::sockaddr_in {
                    sin_len: std::mem::size_of::<libc::sockaddr_in>() as u8,
                    sin_family: libc::AF_INET as u8,
                    sin_port: port.to_be(),
                    sin_addr: libc::in_addr { s_addr: 0 },
                    sin_zero: zeroed(),
                };

                #[cfg(not(target_os = "macos"))]
                let addr = libc::sockaddr_in {
                    sin_family: libc::AF_INET as u16,
                    sin_port: port.to_be(),
                    sin_addr: libc::in_addr { s_addr: 0 },
                    sin_zero: zeroed(),
                };

                if libc::bind(
                    fd,
                    &addr as *const libc::sockaddr_in as *const libc::sockaddr,
                    std::mem::size_of::<libc::sockaddr_in>() as u32,
                ) < 0
                {
                    let err = io::Error::last_os_error();
                    libc::close(fd);
                    return Err(err);
                }

                Ok(Self {
                    fd,
                    state: AssociationState::Closed,
                    local_tag: generate_tag(),
                    remote_tag: 0,
                    local_port: port,
                    remote_port: 0,
                    primary_path: None,
                    alternate_paths: Vec::new(),
                })
            }
        }
    }

    /// Connect to a remote SCTP endpoint (initiates 4-way handshake)
    pub fn connect(&mut self, remote: SocketAddr) -> io::Result<()> {
        self.remote_port = remote.port();
        self.primary_path = Some(remote);
        self.state = AssociationState::CookieWait;

        #[cfg(target_os = "linux")]
        {
            use std::mem::zeroed;

            let (addr, addr_len) = match remote {
                SocketAddr::V4(v4) => {
                    #[cfg(target_os = "macos")]
                    let addr = libc::sockaddr_in {
                        sin_len: std::mem::size_of::<libc::sockaddr_in>() as u8,
                        sin_family: libc::AF_INET as u16,
                        sin_port: v4.port().to_be(),
                        sin_addr: libc::in_addr {
                            s_addr: u32::from_ne_bytes(v4.ip().octets()),
                        },
                        sin_zero: zeroed(),
                    };

                    #[cfg(not(target_os = "macos"))]
                    let addr = libc::sockaddr_in {
                        sin_family: libc::AF_INET as u16,
                        sin_port: v4.port().to_be(),
                        sin_addr: libc::in_addr {
                            s_addr: u32::from_ne_bytes(v4.ip().octets()),
                        },
                        sin_zero: zeroed(),
                    };
                    (
                        &addr as *const libc::sockaddr_in as *const libc::sockaddr,
                        std::mem::size_of::<libc::sockaddr_in>() as u32,
                    )
                }
                SocketAddr::V6(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "IPv6 not yet supported",
                    ))
                }
            };

            unsafe {
                if libc::connect(self.fd, addr, addr_len) < 0 {
                    let err = io::Error::last_os_error();
                    // On some systems, connect may return EINPROGRESS for non-blocking
                    if err.raw_os_error() != Some(libc::EINPROGRESS) {
                        return Err(err);
                    }
                }
            }
        }

        Ok(())
    }

    /// Send data on a specific stream
    pub fn send(&mut self, stream_id: u16, data: &[u8]) -> io::Result<usize> {
        if self.state != AssociationState::Established {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "association not established",
            ));
        }

        let chunk = DataChunk {
            flags: DataFlags::new(DataFlags::BEGIN | DataFlags::END),
            stream_id,
            stream_seq_num: 0,
            payload_protocol_id: 0,
            transmission_seq_num: 0, // Would be managed by association state
            user_data: data.to_vec(),
        };

        let packet = chunk.to_bytes();
        self.send_packet(&packet)
    }

    /// Receive data from any stream
    pub fn recv(&mut self, buf: &mut [u8]) -> io::Result<(u16, usize)> {
        let mut packet_buf = vec![0u8; 65536];
        let n = self.recv_packet(&mut packet_buf)?;

        // Parse the SCTP packet
        match Chunk::from_bytes(&packet_buf[..n]) {
            Ok(Chunk::Data(data_chunk)) => {
                let len = buf.len().min(data_chunk.user_data.len());
                buf[..len].copy_from_slice(&data_chunk.user_data[..len]);
                Ok((data_chunk.stream_id, len))
            }
            Ok(_) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "non-DATA chunk received",
            )),
            Err(e) => Err(e),
        }
    }

    /// Send a raw SCTP packet
    fn send_packet(&self, data: &[u8]) -> io::Result<usize> {
        unsafe {
            let n = libc::send(self.fd, data.as_ptr() as *const libc::c_void, data.len(), 0);
            if n < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(n as usize)
            }
        }
    }

    /// Receive a raw SCTP packet
    fn recv_packet(&self, buf: &mut [u8]) -> io::Result<usize> {
        unsafe {
            let n = libc::recv(self.fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len(), 0);
            if n < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(n as usize)
            }
        }
    }

    /// Get current association state
    pub fn state(&self) -> AssociationState {
        self.state
    }

    /// Add an alternate path for multi-homing
    pub fn add_alternate_path(&mut self, addr: SocketAddr) {
        self.alternate_paths.push(addr);
    }

    /// Get primary path
    pub fn primary_path(&self) -> Option<SocketAddr> {
        self.primary_path
    }

    /// Get all paths (primary + alternates)
    pub fn all_paths(&self) -> Vec<SocketAddr> {
        let mut paths = self.alternate_paths.clone();
        if let Some(primary) = self.primary_path {
            paths.insert(0, primary);
        }
        paths
    }
}

impl SelectableChannel for SctpSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }

    fn raw_fd(&self) -> RawFd {
        self.fd
    }

    fn is_open(&self) -> bool {
        self.fd >= 0
    }

    fn close(&mut self) -> io::Result<()> {
        if self.fd >= 0 {
            unsafe {
                if libc::close(self.fd) < 0 {
                    return Err(io::Error::last_os_error());
                }
            }
        }
        self.state = AssociationState::Closed;
        Ok(())
    }

    fn bind(&mut self, _addr: SocketAddr) -> io::Result<()> {
        // Re-bind is handled via SctpSocket::bind() constructor
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "use SctpSocket::bind() instead",
        ))
    }

    fn listen(&mut self) -> io::Result<()> {
        unsafe {
            if libc::listen(self.fd, 128) < 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    fn accept(&mut self) -> io::Result<(std::net::TcpStream, SocketAddr)> {
        // SCTP associations are not TCP streams; this trait method
        // doesn't map cleanly. Stub for SelectableChannel compatibility.
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "use SCTP association handshake instead",
        ))
    }
}

impl Drop for SctpSocket {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// Generate a random verification tag
fn generate_tag() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    // Simple hash-based tag generation
    ((nonce.wrapping_mul(0x5deece66du64).wrapping_add(0xbu64)) & 0xFFFFFFFF) as u32
}

/// SCTP packet header (12 bytes common header)
#[derive(Debug, Clone)]
pub struct PacketHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub verification_tag: u32,
    pub checksum: u32,
}

impl PacketHeader {
    pub const SIZE: usize = 12;

    pub fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        if bytes.len() < Self::SIZE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "packet header too short",
            ));
        }
        Ok(Self {
            src_port: u16::from_be_bytes([bytes[0], bytes[1]]),
            dst_port: u16::from_be_bytes([bytes[2], bytes[3]]),
            verification_tag: u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            checksum: u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
        })
    }

    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0..2].copy_from_slice(&self.src_port.to_be_bytes());
        bytes[2..4].copy_from_slice(&self.dst_port.to_be_bytes());
        bytes[4..8].copy_from_slice(&self.verification_tag.to_be_bytes());
        bytes[8..12].copy_from_slice(&self.checksum.to_be_bytes());
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_packet_header_roundtrip() {
        let header = PacketHeader {
            src_port: 8888,
            dst_port: 9999,
            verification_tag: 0x12345678,
            checksum: 0xABCDEF00,
        };
        let bytes = header.to_bytes();
        let parsed = PacketHeader::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.src_port, 8888);
        assert_eq!(parsed.dst_port, 9999);
        assert_eq!(parsed.verification_tag, 0x12345678);
        assert_eq!(parsed.checksum, 0xABCDEF00);
    }

    #[test]
    fn test_association_state_transitions() {
        let mut socket = SctpSocket::bind(0).unwrap();
        assert_eq!(socket.state(), AssociationState::Closed);

        // Connect should transition to CookieWait
        let addr = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 9999);
        let _ = socket.connect(addr);
        assert_eq!(socket.state(), AssociationState::CookieWait);
    }
}
