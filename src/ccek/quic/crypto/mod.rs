//! QUIC Crypto - TLS handshake and key derivation
//!
//! This module CANNOT see stream or connection.
//! It only knows about itself and ccek-core.

use ccek_core::{Context, Element, Key};
use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicU32, Ordering};

/// CryptoKey - TLS handshake state
pub struct CryptoKey;

impl CryptoKey {
    pub const FACTORY: fn() -> CryptoElement = || CryptoElement::new();
}

impl Key for CryptoKey {
    type Element = CryptoElement;
    const FACTORY: fn() -> Self::Element = CryptoKey::FACTORY;
}

/// CryptoElement - crypto handshake state
pub struct CryptoElement {
    pub handshake_complete: AtomicU32,
    pub bytes_sent: u64,
    pub bytes_recv: u64,
}

impl CryptoElement {
    pub fn new() -> Self {
        Self {
            handshake_complete: AtomicU32::new(0),
            bytes_sent: 0,
            bytes_recv: 0,
        }
    }

    pub fn is_handshake_complete(&self) -> bool {
        self.handshake_complete.load(Ordering::Relaxed) == 1
    }

    pub fn complete_handshake(&self) {
        self.handshake_complete.store(1, Ordering::Relaxed);
    }
}

impl Element for CryptoElement {
    fn key_type(&self) -> TypeId {
        TypeId::of::<CryptoKey>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// TLS cipher suites
#[derive(Debug, Clone, Copy)]
pub enum CipherSuite {
    Aes128Gcm,
    Aes256Gcm,
    Chacha20Poly1305,
}

/// TLS 1.3 handshake state
#[derive(Debug, Clone, Copy)]
pub enum HandshakeState {
    None,
    ClientHello,
    ServerHello,
    EncryptedExtensions,
    Certificate,
    CertificateVerify,
    Finished,
    Complete,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crypto_factory() {
        let elem = CryptoKey::FACTORY();
        assert!(!elem.is_handshake_complete());
    }

    #[test]
    fn test_crypto_handshake() {
        let elem = CryptoElement::new();
        elem.complete_handshake();
        assert!(elem.is_handshake_complete());
    }
}
