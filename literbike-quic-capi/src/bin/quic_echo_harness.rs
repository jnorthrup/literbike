use literbike::quic::QuicServer;
use std::net::SocketAddr;
use std::time::Duration;

fn main() {
    let bind_addr: SocketAddr = std::env::var("LB_QUIC_ECHO_BIND")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| "127.0.0.1:0".parse().expect("default bind addr"));
    let lifetime_ms: u64 = std::env::var("LB_QUIC_ECHO_LIFETIME_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5000);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    rt.block_on(async move {
        let server = QuicServer::bind(bind_addr).await.expect("bind quic server");
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async move {
                server.start().await.expect("start quic server");
                let addr = server.local_addr().expect("server local addr");
                println!("LB_QUIC_ECHO_ADDR={addr}");
                tokio::time::sleep(Duration::from_millis(lifetime_ms)).await;
            })
            .await;
    });
}
