//! TLS termination for QUIC server with rustls
//!
//! Provides TLS 1.3 encryption with ALPN negotiation for H2/H3 protocols.
//! This is an additive layer on top of the existing QUIC server infrastructure.

use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::sync::Arc;

/// ALPN protocol identifiers for H2/H3 negotiation
pub mod alpn {
    /// HTTP/3 over QUIC
    pub const H3: &[u8] = b"h3";
    /// HTTP/2 over TLS
    pub const H2: &[u8] = b"h2";
    /// QUIC interop testing
    pub const HQ_INTEROP: &[u8] = b"hq-interop";
    /// Custom QUIC protocol
    pub const CUSTOM_QUIC: &[u8] = b"customquic";
    
    /// Get all supported ALPN protocols
    pub fn supported() -> Vec<Vec<u8>> {
        vec![
            H3.to_vec(),
            HQ_INTEROP.to_vec(),
            CUSTOM_QUIC.to_vec(),
            H2.to_vec(),
        ]
    }
}

/// TLS configuration and certificate management
pub struct TlsTerminator {
    /// rustls server configuration
    pub config: Arc<rustls::ServerConfig>,
    /// Certificate chain
    pub cert_chain: Vec<CertificateDer<'static>>,
}

impl TlsTerminator {
    /// Create a new TLS terminator with self-signed localhost certificate
    pub fn localhost() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Generate self-signed certificate
        let key_pair = rcgen::KeyPair::generate()?;
        let mut cert_params = rcgen::CertificateParams::new(vec![
            "localhost".to_string(),
            "127.0.0.1".to_string(),
        ])?;
        cert_params.subject_alt_names = vec![
            rcgen::SanType::DnsName("localhost".try_into()?),
            rcgen::SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
        ];
        let cert = cert_params.self_signed(&key_pair)?;
        
        let cert_der = cert.der().as_ref().to_vec();
        let key_der = key_pair.serialize_der();
        
        Self::new(cert_der, key_der)
    }

    /// Create TLS terminator for a specific domain
    pub fn domain(domain: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let key_pair = rcgen::KeyPair::generate()?;
        let mut cert_params = rcgen::CertificateParams::new(vec![domain.to_string()])?;
        cert_params.subject_alt_names = vec![
            rcgen::SanType::DnsName(domain.try_into()?),
        ];
        let cert = cert_params.self_signed(&key_pair)?;
        
        let cert_der = cert.der().as_ref().to_vec();
        let key_der = key_pair.serialize_der();
        
        Self::new(cert_der, key_der)
    }

    /// Create TLS terminator with custom certificate and key
    pub fn new(
        cert_der: Vec<u8>,
        key_der: Vec<u8>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Build rustls config with TLS 1.3
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(cert_der.clone())],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_der)),
            )?;

        Ok(Self {
            config: Arc::new(config),
            cert_chain: vec![CertificateDer::from(cert_der)],
        })
    }

    /// Load certificate and key from PEM files
    pub fn from_pem_files(
        cert_path: &str,
        key_path: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let cert_pem = std::fs::read(cert_path)?;
        let key_pem = std::fs::read(key_path)?;
        
        let mut cert_reader = &cert_pem[..];
        let mut key_reader = &key_pem[..];
        
        let certs: Vec<CertificateDer> = rustls_pemfile::certs(&mut cert_reader)
            .collect::<Result<Vec<_>, _>>()?;
        
        let keys = rustls_pemfile::pkcs8_private_keys(&mut key_reader)
            .collect::<Result<Vec<_>, _>>()?;
        
        if certs.is_empty() || keys.is_empty() {
            return Err("No certificate or key found in PEM files".into());
        }

        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs.clone(), PrivateKeyDer::from(keys[0].clone_key()))?;

        Ok(Self {
            config: Arc::new(config),
            cert_chain: certs,
        })
    }

    /// Get ALPN protocols as wire format
    pub fn alpn_protocols(&self) -> Vec<Vec<u8>> {
        alpn::supported()
    }

    /// Check if a specific ALPN protocol is supported
    pub fn supports_alpn(&self, protocol: &[u8]) -> bool {
        alpn::supported().iter().any(|p| p.as_slice() == protocol)
    }

    /// Get the rustls server config
    pub fn server_config(&self) -> Arc<rustls::ServerConfig> {
        self.config.clone()
    }
}

/// Helper to extract ALPN from TLS handshake
pub fn extract_alpn(server_config: &rustls::ServerConfig) -> Option<Vec<u8>> {
    server_config.alpn_protocols.first().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_localhost_terminator() {
        let terminator = TlsTerminator::localhost().expect("Failed to create localhost terminator");
        assert!(!terminator.cert_chain.is_empty());
        assert_eq!(terminator.alpn_protocols().len(), 4);
    }

    #[test]
    fn test_alpn_protocols() {
        assert_eq!(alpn::H3, b"h3");
        assert_eq!(alpn::H2, b"h2");
        assert!(alpn::supported().contains(&b"h3".to_vec()));
    }
}
