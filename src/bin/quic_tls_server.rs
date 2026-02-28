//! QUIC TLS Server Binary with H2/H3 support
//!
//! Uses the existing QUIC server infrastructure with additive rustls TLS termination.

use clap::Parser;
use literbike::quic::tls::TlsTerminator;
use std::net::SocketAddr;

#[derive(Parser, Debug)]
#[command(author, version, about = "QUIC TLS Server with H2/H3 support", long_about = None)]
struct Args {
    /// Bind address
    #[arg(short, long, default_value = "0.0.0.0")]
    bind: String,

    /// Port number
    #[arg(short, long, default_value = "4433")]
    port: u16,

    /// Domain name for certificate
    #[arg(short, long, default_value = "localhost")]
    domain: String,

    /// Use PEM certificate files instead of generating
    #[arg(short, long)]
    use_certs: bool,

    /// Certificate file path
    #[arg(long, default_value = "certs/server.crt")]
    cert_path: String,

    /// Key file path
    #[arg(long, default_value = "certs/server.key")]
    key_path: String,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Install rustls ring crypto provider (required for rustls 0.23+)
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install ring crypto provider");

    let args = Args::parse();

    // Initialize logging
    if args.verbose {
        std::env::set_var("RUST_LOG", "debug");
    } else {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    let addr: SocketAddr = format!("{}:{}", args.bind, args.port).parse()?;

    println!("🔒 QUIC TLS Server");
    println!("=================");
    println!("Bind: {}", addr);
    println!();

    // Create TLS terminator
    let tls = if args.use_certs {
        println!("📜 Loading certificate from {}...", args.cert_path);
        println!("📜 Loading key from {}...", args.key_path);
        TlsTerminator::from_pem_files(&args.cert_path, &args.key_path)?
    } else if args.domain != "localhost" {
        println!("📜 Generating certificate for {}...", args.domain);
        TlsTerminator::domain(&args.domain)?
    } else {
        println!("📜 Generating self-signed certificate for localhost...");
        TlsTerminator::localhost()?
    };

    println!("✅ TLS Terminator ready");
    println!();
    println!("📡 ALPN Protocols:");
    for proto in tls.alpn_protocols() {
        println!("   - {}", String::from_utf8_lossy(&proto));
    }
    println!();
    println!("🧪 Test with curl:");
    println!("   curl -k --http3 https://{}:{}/", args.bind, args.port);
    println!("   curl -k --http2 https://{}:{}/", args.bind, args.port);
    println!();
    println!("🛑 Press Ctrl+C to stop");
    println!();

    println!("✨ QUIC server with TLS is ready");
    println!("   TLS 1.3 encryption active");
    println!("   ALPN negotiation enabled for H2/H3");

    let tls_ccek = std::sync::Arc::new(literbike::quic::tls_ccek::TlsCcekService::new(tls, 100));
    let ctx = literbike::concurrency::ccek::EmptyContext + tls_ccek.clone() as std::sync::Arc<dyn literbike::concurrency::ccek::ContextElement>;

    let local = tokio::task::LocalSet::new();
    local.run_until(async move {
        // Spawn the background channel loop for the CCEK TLS config manager
        let tls_ccek_loop = tls_ccek.clone();
        tokio::task::spawn_local(async move {
            let svc = (*tls_ccek_loop).clone();
            svc.run_command_loop().await;
        });

        match literbike::quic::QuicServer::bind(addr, ctx).await {
            Ok(server) => {
                println!("✓ QUIC (UDP) Server ACTIVE on {}", addr);
                
                // Spawn TCP Alt-Svc beacon on the same port
                let tcp_bind_addr = addr.clone();
                tokio::task::spawn_local(async move {
                    use tokio::io::AsyncWriteExt;
                    if let Ok(listener) = tokio::net::TcpListener::bind(&tcp_bind_addr).await {
                        println!("⚓ TCP Alt-Svc Beacon listening on {}", tcp_bind_addr);
                        loop {
                            if let Ok((mut stream, _)) = listener.accept().await {
                                tokio::task::spawn_local(async move {
                                    let response = "HTTP/1.1 200 OK\r\n\
                                                    Alt-Svc: h3=\":4433\"; ma=86400\r\n\
                                                    Content-Type: text/html\r\n\
                                                    Connection: close\r\n\r\n\
                                                    <html><head><meta http-equiv='refresh' content='2'></head>\
                                                    <body style='background:#111;color:#0f0;font-family:monospace;padding:40px;'>\
                                                    <h1>⚓ LITEBIKE QUIC BOOTSTRAP</h1>\
                                                    <p>Browser connected via TCP. Sending Alt-Svc header...</p>\
                                                    <p>Refreshing in 2 seconds to transition to QUIC/H3.</p>\
                                                    </body></html>";
                                    let _ = stream.write_all(response.as_bytes()).await;
                                });
                            }
                        }
                    }
                });

                if let Err(e) = server.start().await {
                    eprintln!("❌ Server error: {}", e);
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to bind server to {}: {}", addr, e);
            }
        }
    }).await;

    Ok(())
}
