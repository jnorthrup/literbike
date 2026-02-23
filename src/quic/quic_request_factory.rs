use std::net::SocketAddr;
use crate::{quic_client::QuicClient, quic_config::QuicConfig};
use std::sync::Arc;

pub struct QuicRequestFactory;

impl QuicRequestFactory {
    pub async fn demo(addr: SocketAddr) -> anyhow::Result<()> {
        let client = QuicClient::new(Arc::new(QuicConfig::default()))?;
        let _conn = client.connect(addr, "localhost").await?;
        Ok(())
    }
}
