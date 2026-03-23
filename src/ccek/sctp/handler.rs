//! SCTP event handler for reactor integration
//!
//! Implements EventHandler trait for dispatching SCTP events through the reactor.

use super::socket::SctpSocket;
use crate::chunks::Chunk;
use parking_lot::Mutex;
use std::io;
use std::os::unix::io::RawFd;
use std::sync::Arc;

/// Callback for received SCTP data
pub type SctpDataCallback = Arc<Mutex<dyn FnMut(u16, Vec<u8>) + Send>>;

/// Callback for association state changes
pub type SctpStateCallback = Arc<Mutex<dyn FnMut(super::socket::AssociationState) + Send>>;

/// EventHandler stub for SCTP
pub trait EventHandler: Send + Sync {
    fn on_readable(&mut self, fd: RawFd);
    fn on_writable(&mut self, fd: RawFd);
    fn on_error(&mut self, fd: RawFd, error: io::Error);
}

/// SCTP handler for reactor integration
pub struct SctpHandler {
    socket: SctpSocket,
    on_data: Option<SctpDataCallback>,
    on_state_change: Option<SctpStateCallback>,
}

impl SctpHandler {
    /// Create a new SCTP handler
    pub fn new(socket: SctpSocket) -> Self {
        Self {
            socket,
            on_data: None,
            on_state_change: None,
        }
    }

    /// Set the data receive callback
    pub fn on_data(mut self, callback: SctpDataCallback) -> Self {
        self.on_data = Some(callback);
        self
    }

    /// Set the state change callback
    pub fn on_state_change(mut self, callback: SctpStateCallback) -> Self {
        self.on_state_change = Some(callback);
        self
    }

    /// Get the underlying socket
    pub fn socket(&self) -> &SctpSocket {
        &self.socket
    }

    /// Get mutable reference to the socket
    pub fn socket_mut(&mut self) -> &mut SctpSocket {
        &mut self.socket
    }
}

impl EventHandler for SctpHandler {
    fn on_readable(&mut self, _fd: RawFd) {
        let mut buf = vec![0u8; 65536];
        match self.socket.recv(&mut buf) {
            Ok((stream_id, n)) => {
                let data = buf[..n].to_vec();
                if let Some(ref cb) = self.on_data {
                    if let Some(mut callback) = cb.try_lock() {
                        callback(stream_id, data);
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // No data available, ignore
            }
            Err(e) => {
                eprintln!("SCTP recv error: {}", e);
            }
        }
    }

    fn on_writable(&mut self, _fd: RawFd) {
        // Socket is writable for sending
    }

    fn on_error(&mut self, _fd: RawFd, error: io::Error) {
        eprintln!("SCTP socket error: {}", error);
        if let Some(ref cb) = self.on_state_change {
            if let Some(mut callback) = cb.try_lock() {
                callback(super::socket::AssociationState::Closed);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_creation() {
        let socket = SctpSocket::bind(0).unwrap();
        let handler = SctpHandler::new(socket);
        assert!(handler.socket().is_open());
    }
}
