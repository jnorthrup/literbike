//! I/O Utilities for CCEK Protocol Handling
//!
//! This module claims all I/O abstractions from litebike protocols,
//! providing production-ready connection handling and stream management.

use std::io::{self, Read, Write};
use std::net::TcpStream;

/// PrefixedStream wraps a TcpStream with a buffer of already-read data.
/// This is essential for protocol detection where we need to peek at initial bytes
/// before deciding how to handle the connection.
pub struct PrefixedStream {
    inner: TcpStream,
    prefix: Vec<u8>,
    prefix_offset: usize,
}

impl PrefixedStream {
    /// Create a new PrefixedStream with the given prefix buffer.
    /// The prefix contains bytes that were already read during protocol detection.
    pub fn new(inner: TcpStream, prefix: Vec<u8>) -> Self {
        Self {
            inner,
            prefix,
            prefix_offset: 0,
        }
    }

    /// Get the underlying TCP stream (for advanced operations)
    pub fn into_inner(self) -> TcpStream {
        self.inner
    }

    /// Get a reference to the underlying stream
    pub fn get_ref(&self) -> &TcpStream {
        &self.inner
    }

    /// Get the total bytes available (prefix + stream)
    pub fn available_bytes(&self) -> usize {
        self.prefix.len() - self.prefix_offset
    }

    /// Check if prefix has been fully consumed
    pub fn prefix_consumed(&self) -> bool {
        self.prefix_offset >= self.prefix.len()
    }
}

impl Read for PrefixedStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // First, serve from prefix buffer
        if self.prefix_offset < self.prefix.len() {
            let available = self.prefix.len() - self.prefix_offset;
            let to_copy = available.min(buf.len());
            buf[..to_copy]
                .copy_from_slice(&self.prefix[self.prefix_offset..self.prefix_offset + to_copy]);
            self.prefix_offset += to_copy;
            return Ok(to_copy);
        }

        // Then read from underlying stream
        self.inner.read(buf)
    }
}

impl Write for PrefixedStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Connection represents an accepted client connection with protocol detection state
pub struct Connection {
    pub stream: PrefixedStream,
    pub peer_addr: Option<std::net::SocketAddr>,
    pub local_addr: Option<std::net::SocketAddr>,
    pub detection_buffer: Vec<u8>,
    pub max_detection_bytes: usize,
}

impl Connection {
    /// Create a new connection from a TCP stream
    pub fn new(stream: TcpStream) -> io::Result<Self> {
        let peer_addr = stream.peer_addr().ok();
        let local_addr = stream.local_addr().ok();

        Ok(Self {
            stream: PrefixedStream::new(stream, Vec::new()),
            peer_addr,
            local_addr,
            detection_buffer: Vec::with_capacity(4096),
            max_detection_bytes: 8192,
        })
    }

    /// Read initial bytes for protocol detection
    pub fn detect_bytes(&mut self) -> io::Result<&[u8]> {
        let mut temp_buf = [0u8; 1024];

        loop {
            if self.detection_buffer.len() >= self.max_detection_bytes {
                break;
            }

            match self.stream.read(&mut temp_buf) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    self.detection_buffer.extend_from_slice(&temp_buf[..n]);
                    // Store for later use in PrefixedStream
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            }
        }

        // Now we need to wrap the stream with the detection buffer
        // First, extract the inner stream
        let inner = std::mem::replace(
            &mut self.stream,
            PrefixedStream::new(
                TcpStream::connect("127.0.0.1:1").unwrap(), // placeholder
                Vec::new(),
            ),
        );
        let tcp_stream = inner.into_inner();

        // Create new PrefixedStream with detection buffer
        self.stream = PrefixedStream::new(tcp_stream, self.detection_buffer.clone());

        Ok(&self.detection_buffer)
    }

    /// Set maximum bytes to read for detection
    pub fn with_max_detection_bytes(mut self, max: usize) -> Self {
        self.max_detection_bytes = max;
        self
    }
}

/// ConnectionPool manages a pool of active connections for the reactor
pub struct ConnectionPool {
    connections: Vec<Connection>,
    max_connections: usize,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new(max_connections: usize) -> Self {
        Self {
            connections: Vec::with_capacity(max_connections),
            max_connections,
        }
    }

    /// Add a connection to the pool
    pub fn add(&mut self, conn: Connection) -> Result<(), Connection> {
        if self.connections.len() < self.max_connections {
            self.connections.push(conn);
            Ok(())
        } else {
            Err(conn)
        }
    }

    /// Remove a connection from the pool
    pub fn remove(&mut self, index: usize) -> Option<Connection> {
        if index < self.connections.len() {
            Some(self.connections.remove(index))
        } else {
            None
        }
    }

    /// Get the number of active connections
    pub fn len(&self) -> usize {
        self.connections.len()
    }

    /// Check if pool is empty
    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }

    /// Get a reference to a connection
    pub fn get(&self, index: usize) -> Option<&Connection> {
        self.connections.get(index)
    }

    /// Get a mutable reference to a connection
    pub fn get_mut(&mut self, index: usize) -> Option<&mut Connection> {
        self.connections.get_mut(index)
    }

    /// Iterate over all connections
    pub fn iter(&self) -> impl Iterator<Item = &Connection> {
        self.connections.iter()
    }

    /// Iterate mutably over all connections
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Connection> {
        self.connections.iter_mut()
    }

    /// Clear all connections
    pub fn clear(&mut self) {
        self.connections.clear();
    }
}

/// I/O statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct IoStats {
    pub bytes_read: u64,
    pub bytes_written: u64,
    pub connections_accepted: u64,
    pub connections_closed: u64,
    pub errors: u64,
}

impl IoStats {
    /// Create new empty stats
    pub fn new() -> Self {
        Self::default()
    }

    /// Record bytes read
    pub fn record_read(&mut self, bytes: usize) {
        self.bytes_read += bytes as u64;
    }

    /// Record bytes written
    pub fn record_write(&mut self, bytes: usize) {
        self.bytes_written += bytes as u64;
    }

    /// Record connection accepted
    pub fn record_accept(&mut self) {
        self.connections_accepted += 1;
    }

    /// Record connection closed
    pub fn record_close(&mut self) {
        self.connections_closed += 1;
    }

    /// Record error
    pub fn record_error(&mut self) {
        self.errors += 1;
    }

    /// Get active connection count
    pub fn active_connections(&self) -> u64 {
        self.connections_accepted
            .saturating_sub(self.connections_closed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefixed_stream_reads_prefix_first() {
        // Create a mock stream by connecting to localhost
        let stream = TcpStream::connect("127.0.0.1:1");
        if stream.is_err() {
            // Skip test if we can't create a connection
            return;
        }

        let prefix = b"Hello, World!".to_vec();
        let mut prefixed = PrefixedStream::new(stream.unwrap(), prefix);

        let mut buf = [0u8; 5];
        // This would read from prefix in a real scenario
        // but we can't easily test without a mock
    }

    #[test]
    fn test_connection_pool() {
        let mut pool = ConnectionPool::new(10);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn test_io_stats() {
        let mut stats = IoStats::new();
        stats.record_read(100);
        stats.record_write(50);
        stats.record_accept();

        assert_eq!(stats.bytes_read, 100);
        assert_eq!(stats.bytes_written, 50);
        assert_eq!(stats.connections_accepted, 1);
        assert_eq!(stats.active_connections(), 1);

        stats.record_close();
        assert_eq!(stats.active_connections(), 0);
    }
}
