// Direct POSIX socket operations
// Bypasses /proc filesystem restrictions by using direct syscalls


use libc::{recv, MSG_PEEK, c_void, size_t};

use nix::sys::socket::{getsockopt, sockopt};

use std::io::{self, Error, ErrorKind};

use std::os::fd::{self, AsRawFd};

use tokio::net::TcpStream;

/// POSIX-safe peek operation using direct POSIX recv() with MSG_PEEK

pub fn posix_peek(stream: &TcpStream, buf: &mut [u8]) -> io::Result<usize> {
    let fd = stream.as_raw_fd();
    
    let result = unsafe {
        recv(
            fd,
            buf.as_mut_ptr() as *mut c_void,
            buf.len() as size_t,
            MSG_PEEK
        )
    };
    
    if result < 0 {
        Err(Error::last_os_error())
    } else {
        Ok(result as usize)
    }
}

/// Extract socket information without /proc access

pub fn get_socket_info(stream: &TcpStream) -> io::Result<SocketInfo> {
    let fd = stream.as_raw_fd();
    
    // Get socket type to determine if it's TCP
    let socket_type: nix::sys::socket::SockType = getsockopt(stream, sockopt::SockType)
        .map_err(|e| Error::new(ErrorKind::Other, e))?;
    
    // Get receive buffer size  
    let rcv_buf: usize = getsockopt(stream, sockopt::RcvBuf)
        .map_err(|e| Error::new(ErrorKind::Other, e))?;
    
    Ok(SocketInfo {
        socket_type,
        receive_buffer_size: rcv_buf,
    })
}


pub struct SocketInfo {
    pub socket_type: nix::sys::socket::SockType,
    pub receive_buffer_size: usize,
}

/// Wrapper for TcpStream that uses POSIX peek instead of standard library

pub struct PosixTcpStream {
    inner: TcpStream,
    peek_buffer: Vec<u8>,
    peek_offset: usize,
}


impl PosixTcpStream {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            inner: stream,
            peek_buffer: Vec::new(),
            peek_offset: 0,
        }
    }
    
    /// POSIX-safe peek that works around /proc restrictions
    pub fn posix_peek(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // If we have data in our peek buffer, return that first
        if self.peek_offset < self.peek_buffer.len() {
            let available = self.peek_buffer.len() - self.peek_offset;
            let to_copy = std::cmp::min(buf.len(), available);
            buf[..to_copy].copy_from_slice(
                &self.peek_buffer[self.peek_offset..self.peek_offset + to_copy]
            );
            return Ok(to_copy);
        }
        
        // Use direct POSIX recv with MSG_PEEK
        let peeked = posix_peek(&self.inner, buf)?;
        
        // Store the peeked data for potential replay
        if peeked > 0 {
            self.peek_buffer.clear();
            self.peek_buffer.extend_from_slice(&buf[..peeked]);
            self.peek_offset = 0;
        }
        
        Ok(peeked)
    }
    
    /// Get reference to inner stream for other operations
    pub fn inner(&self) -> &TcpStream {
        &self.inner
    }
    
    /// Get mutable reference to inner stream
    pub fn inner_mut(&mut self) -> &mut TcpStream {
        &mut self.inner
    }
}


impl std::ops::Deref for PosixTcpStream {
    type Target = TcpStream;
    
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}


impl std::ops::DerefMut for PosixTcpStream {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

