//! Network channel abstractions for unified I/O operations

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicU64, Ordering};

/// Metadata about a channel
#[derive(Debug)]
pub struct ChannelMetadata {
    pub remote_addr: Option<std::net::SocketAddr>,
    pub local_addr: Option<std::net::SocketAddr>,
    pub protocol: Option<super::protocols::Protocol>,
    pub bytes_read: AtomicU64,
    pub bytes_written: AtomicU64,
}

impl Clone for ChannelMetadata {
    fn clone(&self) -> Self {
        Self {
            remote_addr: self.remote_addr,
            local_addr: self.local_addr,
            protocol: self.protocol.clone(),
            bytes_read: AtomicU64::new(self.bytes_read.load(Ordering::Relaxed)),
            bytes_written: AtomicU64::new(self.bytes_written.load(Ordering::Relaxed)),
        }
    }
}

impl Default for ChannelMetadata {
    fn default() -> Self {
        Self {
            remote_addr: None,
            local_addr: None,
            protocol: None,
            bytes_read: AtomicU64::new(0),
            bytes_written: AtomicU64::new(0),
        }
    }
}

impl ChannelMetadata {
    pub fn bytes_read(&self) -> u64 {
        self.bytes_read.load(Ordering::Relaxed)
    }

    pub fn bytes_written(&self) -> u64 {
        self.bytes_written.load(Ordering::Relaxed)
    }
}

/// Trait for network channels that support blocking I/O
pub trait Channel: Send + Sync {
    fn channel_type(&self) -> &str;
    fn is_connected(&self) -> bool;
    fn metadata(&self) -> Option<ChannelMetadata> {
        None
    }
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn write(&mut self, buf: &[u8]) -> io::Result<usize>;
}

/// Basic TCP channel implementation using blocking I/O
pub struct TcpChannel {
    stream: TcpStream,
    metadata: ChannelMetadata,
}

impl TcpChannel {
    pub fn new(stream: TcpStream) -> io::Result<Self> {
        stream.set_nonblocking(false)?;
        let remote_addr = stream.peer_addr().ok();
        let local_addr = stream.local_addr().ok();

        Ok(Self {
            stream,
            metadata: ChannelMetadata {
                remote_addr,
                local_addr,
                ..Default::default()
            },
        })
    }

    pub fn connect(addr: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        Self::new(stream)
    }
}

impl Channel for TcpChannel {
    fn channel_type(&self) -> &str {
        "TCP"
    }

    fn is_connected(&self) -> bool {
        self.stream.peer_addr().is_ok()
    }

    fn metadata(&self) -> Option<ChannelMetadata> {
        Some(self.metadata.clone())
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.stream.read(buf)?;
        self.metadata
            .bytes_read
            .fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.stream.write(buf)?;
        self.metadata
            .bytes_written
            .fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

/// Provider for creating channels
pub trait ChannelProvider: Send + Sync {
    fn create_channel(&self, addr: &str) -> io::Result<Box<dyn Channel>>;
    fn provider_name(&self) -> &str;
}

/// Default TCP channel provider
pub struct TcpChannelProvider;

impl ChannelProvider for TcpChannelProvider {
    fn create_channel(&self, addr: &str) -> io::Result<Box<dyn Channel>> {
        let stream = TcpStream::connect(addr)?;
        let ch = TcpChannel::new(stream)?;
        Ok(Box::new(ch))
    }

    fn provider_name(&self) -> &str {
        "TCP"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_metadata() {
        let metadata = ChannelMetadata::default();
        assert_eq!(metadata.bytes_read(), 0);
        assert_eq!(metadata.bytes_written(), 0);
    }

    #[test]
    fn test_tcp_provider() {
        let provider = TcpChannelProvider;
        assert_eq!(provider.provider_name(), "TCP");
    }
}
