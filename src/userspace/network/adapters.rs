//! Network protocol adapters for various transport protocols

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};

/// Type of network adapter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterType {
    Http,
    Https,
    Quic,
    Ssh,
    WebSocket,
    Raw,
}

/// Trait for network protocol adapters
pub trait NetworkAdapter: Send + Sync {
    fn adapter_type(&self) -> AdapterType;
    fn remote_addr(&self) -> io::Result<SocketAddr>;
    fn is_connected(&self) -> bool;
    fn close(&mut self) -> io::Result<()>;
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn write(&mut self, buf: &[u8]) -> io::Result<usize>;
}

/// HTTP adapter for HTTP/1.1 and HTTP/2 protocols
pub struct HttpAdapter {
    stream: TcpStream,
    remote: SocketAddr,
    connected: bool,
}

impl HttpAdapter {
    pub fn new(stream: TcpStream, remote: SocketAddr) -> Self {
        Self {
            connected: true,
            stream,
            remote,
        }
    }
}

impl NetworkAdapter for HttpAdapter {
    fn adapter_type(&self) -> AdapterType {
        AdapterType::Http
    }

    fn remote_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.remote)
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn close(&mut self) -> io::Result<()> {
        self.connected = false;
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf)
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }
}

/// QUIC adapter for QUIC protocol
pub struct QuicAdapter {
    stream: TcpStream,
    remote: SocketAddr,
    stream_id: u64,
    connected: bool,
}

impl QuicAdapter {
    pub fn new(stream: TcpStream, remote: SocketAddr, stream_id: u64) -> Self {
        Self {
            stream,
            remote,
            stream_id,
            connected: true,
        }
    }

    pub fn stream_id(&self) -> u64 {
        self.stream_id
    }
}

impl NetworkAdapter for QuicAdapter {
    fn adapter_type(&self) -> AdapterType {
        AdapterType::Quic
    }

    fn remote_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.remote)
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn close(&mut self) -> io::Result<()> {
        self.connected = false;
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf)
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }
}

/// SSH adapter for SSH protocol
pub struct SshAdapter {
    stream: TcpStream,
    remote: SocketAddr,
    session_id: Vec<u8>,
    connected: bool,
}

impl SshAdapter {
    pub fn new(stream: TcpStream, remote: SocketAddr) -> Self {
        Self {
            stream,
            remote,
            session_id: Vec::new(),
            connected: true,
        }
    }

    pub fn set_session_id(&mut self, id: Vec<u8>) {
        self.session_id = id;
    }

    pub fn session_id(&self) -> &[u8] {
        &self.session_id
    }
}

impl NetworkAdapter for SshAdapter {
    fn adapter_type(&self) -> AdapterType {
        AdapterType::Ssh
    }

    fn remote_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.remote)
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn close(&mut self) -> io::Result<()> {
        self.connected = false;
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf)
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }
}

/// Factory for creating network adapters
pub struct AdapterFactory;

impl AdapterFactory {
    pub fn create_adapter(
        adapter_type: AdapterType,
        stream: TcpStream,
        remote: SocketAddr,
    ) -> Box<dyn NetworkAdapter> {
        match adapter_type {
            AdapterType::Http | AdapterType::Https => Box::new(HttpAdapter::new(stream, remote)),
            AdapterType::Quic => Box::new(QuicAdapter::new(stream, remote, 0)),
            AdapterType::Ssh => Box::new(SshAdapter::new(stream, remote)),
            _ => Box::new(HttpAdapter::new(stream, remote)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_types() {
        assert_eq!(
            HttpAdapter::new(
                TcpStream::connect("127.0.0.1:0").unwrap(),
                "127.0.0.1:8080".parse().unwrap()
            )
            .adapter_type(),
            AdapterType::Http
        );
    }
}
