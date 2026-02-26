use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct QuicConfig {
    pub alpn: Vec<Vec<u8>>, // e.g., [b"h3", b"hq-interop"]
    pub max_idle_timeout_ms: u64,
    pub max_udp_payload_size: u32,
    pub enable_gso: bool,
    pub enable_ecn: bool,
}

impl Default for QuicConfig {
    fn default() -> Self {
        Self {
            alpn: vec![b"h3".to_vec()],
            max_idle_timeout_ms: 30_000,
            max_udp_payload_size: 1350,
            enable_gso: true,
            enable_ecn: true,
        }
    }
}

impl QuicConfig {
    pub fn arc() -> Arc<Self> {
        Arc::new(Self::default())
    }
}
