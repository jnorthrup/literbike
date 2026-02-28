//! TLS termination for QUIC server — rcgen cert generation, rustls QUIC handshake, OpenSSL SslContext

use openssl::pkey::PKey;
use openssl::ssl::{SslContext, SslMethod};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::sync::Arc;

/// ALPN protocol identifiers for H2/H3 negotiation
pub mod alpn {
    pub const H3: &[u8] = b"h3";
    pub const H2: &[u8] = b"h2";
    pub const HQ_INTEROP: &[u8] = b"hq-interop";
    pub const CUSTOM_QUIC: &[u8] = b"customquic";

    pub fn supported() -> Vec<Vec<u8>> {
        vec![H3.to_vec(), HQ_INTEROP.to_vec(), CUSTOM_QUIC.to_vec(), H2.to_vec()]
    }
}

/// TLS terminator: rcgen generates the cert/key; rustls handles the QUIC handshake;
/// OpenSSL SslContext is also available for raw TLS use.
pub struct TlsTerminator {
    pub ssl_ctx: Arc<SslContext>,
    pub rustls_config: Arc<rustls::ServerConfig>,
}

fn generate_self_signed(sans: Vec<rcgen::SanType>)
    -> Result<(Vec<u8>, Vec<u8>), Box<dyn std::error::Error + Send + Sync>>
{
    let key_pair = rcgen::KeyPair::generate()?;
    let mut params = rcgen::CertificateParams::new(vec![])?;
    params.subject_alt_names = sans;
    let cert = params.self_signed(&key_pair)?;
    Ok((cert.der().as_ref().to_vec(), key_pair.serialize_der()))
}

fn build_ssl_ctx(cert_der: &[u8], key_der: &[u8])
    -> Result<Arc<SslContext>, Box<dyn std::error::Error + Send + Sync>>
{
    let cert = openssl::x509::X509::from_der(cert_der)?;
    let pkey = PKey::private_key_from_der(key_der)?;
    let mut b = SslContext::builder(SslMethod::tls_server())?;
    b.set_certificate(&cert)?;
    b.set_private_key(&pkey)?;
    b.check_private_key()?;
    let mut wire = Vec::new();
    for p in alpn::supported() { wire.push(p.len() as u8); wire.extend_from_slice(&p); }
    b.set_alpn_protos(&wire)?;
    Ok(Arc::new(b.build()))
}

fn build_rustls_config(cert_der: &[u8], key_der: &[u8])
    -> Result<Arc<rustls::ServerConfig>, Box<dyn std::error::Error + Send + Sync>>
{
    let mut cfg = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(cert_der.to_vec())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_der.to_vec())),
        )?;
    cfg.alpn_protocols = alpn::supported();
    Ok(Arc::new(cfg))
}

impl TlsTerminator {
    pub fn localhost() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let (cert_der, key_der) = generate_self_signed(vec![
            rcgen::SanType::DnsName("localhost".try_into()?),
            rcgen::SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
        ])?;
        Self::new(cert_der, key_der)
    }

    pub fn domain(domain: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let (cert_der, key_der) = generate_self_signed(vec![
            rcgen::SanType::DnsName(domain.try_into()?),
        ])?;
        Self::new(cert_der, key_der)
    }

    pub fn new(cert_der: Vec<u8>, key_der: Vec<u8>)
        -> Result<Self, Box<dyn std::error::Error + Send + Sync>>
    {
        Ok(Self {
            ssl_ctx: build_ssl_ctx(&cert_der, &key_der)?,
            rustls_config: build_rustls_config(&cert_der, &key_der)?,
        })
    }

    pub fn from_pem_files(cert_path: &str, key_path: &str)
        -> Result<Self, Box<dyn std::error::Error + Send + Sync>>
    {
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
        Self::new(certs[0].as_ref().to_vec(), keys[0].secret_pkcs8_der().to_vec())
    }

    pub fn alpn_protocols(&self) -> Vec<Vec<u8>> { alpn::supported() }
    pub fn supports_alpn(&self, protocol: &[u8]) -> bool {
        alpn::supported().iter().any(|p| p.as_slice() == protocol)
    }
    pub fn server_config(&self) -> Arc<rustls::ServerConfig> {
        self.rustls_config.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_localhost_terminator() {
        let t = TlsTerminator::localhost().expect("Failed to create localhost terminator");
        assert_eq!(t.alpn_protocols().len(), 4);
    }

    #[test]
    fn test_alpn_protocols() {
        assert_eq!(alpn::H3, b"h3");
        assert!(alpn::supported().contains(&b"h3".to_vec()));
    }
}
