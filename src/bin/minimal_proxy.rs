// Minimal proxy binary for testing basic functionality
use std::net::{IpAddr, SocketAddr};
use std::io;
use std::time::Duration;

use env_logger::Env;
use log::{debug, error, info};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

const HTTP_PORT: u16 = 8080;
const SOCKS_PORT: u16 = 1080;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Simple DNS resolution using system resolver
async fn resolve_host(host: &str) -> io::Result<IpAddr> {
    match host.parse::<IpAddr>() {
        Ok(ip) => Ok(ip),
        Err(_) => {
            // Use tokio's built-in DNS resolution
            let addresses: Vec<SocketAddr> = tokio::net::lookup_host(format!("{}:80", host))
                .await?
                .collect();
            
            addresses.first()
                .map(|addr| addr.ip())
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No IP address found"))
        }
    }
}

/// Connects to a target address with timeout
async fn connect_to_target(target: &str) -> io::Result<TcpStream> {
    let (host, port_str) = match target.rsplit_once(':') {
        Some((host, port)) => (host, port),
        None => return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid target format, missing port")),
    };
    
    let port: u16 = port_str.parse().map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid port number"))?;
    let ip_addr = resolve_host(host).await?;
    let socket_addr = SocketAddr::new(ip_addr, port);

    debug!("Connecting to {} -> {}", target, socket_addr);
    match timeout(CONNECT_TIMEOUT, TcpStream::connect(socket_addr)).await {
        Ok(Ok(stream)) => Ok(stream),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(io::Error::new(io::ErrorKind::TimedOut, "Connection timed out")),
    }
}

/// Relays data between two streams efficiently
async fn relay_streams<S1, S2>(mut client: S1, mut remote: S2) -> io::Result<()>
where
    S1: AsyncRead + AsyncWrite + Unpin,
    S2: AsyncRead + AsyncWrite + Unpin,
{
    let (mut client_reader, mut client_writer) = tokio::io::split(&mut client);
    let (mut remote_reader, mut remote_writer) = tokio::io::split(&mut remote);

    let client_to_remote = tokio::io::copy(&mut client_reader, &mut remote_writer);
    let remote_to_client = tokio::io::copy(&mut remote_reader, &mut client_writer);

    tokio::select! {
        res = client_to_remote => {
            if let Err(e) = res { debug!("Error copying client to remote: {}", e); }
        },
        res = remote_to_client => {
            if let Err(e) = res { debug!("Error copying remote to client: {}", e); }
        },
    }
    debug!("Relay streams finished.");
    Ok(())
}

/// Simple HTTP proxy handler
async fn handle_http<S>(mut stream: S) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut buffer = [0u8; 4096];
    let n = stream.read(&mut buffer).await?;
    if n == 0 { return Ok(()); }

    let request_str = std::str::from_utf8(&buffer[..n]).unwrap_or("");
    debug!("HTTP Request: {}", request_str);

    if request_str.starts_with("CONNECT ") {
        if let Some(host) = request_str.split_whitespace().nth(1) {
            let target = if host.contains(':') { host.to_string() } else { format!("{}:443", host) };
            match connect_to_target(&target).await {
                Ok(remote) => {
                    info!("HTTP CONNECT to {}", target);
                    stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await?;
                    relay_streams(stream, remote).await?;
                }
                Err(e) => {
                    error!("Failed to connect to {}: {}", target, e);
                    stream.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n").await?;
                }
            }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    info!("🚀 Minimal LiteBike Proxy - Basic HTTP proxy only");
    
    let bind_ip: IpAddr = std::env::var("BIND_IP")
        .unwrap_or_else(|_| "0.0.0.0".to_string())
        .parse()
        .expect("Invalid BIND_IP address");

    // Start HTTP proxy
    let http_listener = TcpListener::bind(SocketAddr::new(bind_ip, HTTP_PORT))
        .await
        .expect("Failed to bind HTTP port");
    
    info!("Basic HTTP proxy listening on {}", 
          http_listener.local_addr().unwrap_or_else(|_| SocketAddr::new(bind_ip, HTTP_PORT)));
    
    loop {
        if let Ok((stream, addr)) = http_listener.accept().await {
            debug!("Connection from {}", addr);
            tokio::spawn(async move {
                if let Err(e) = handle_http(stream).await {
                    debug!("HTTP handler error: {}", e);
                }
            });
        }
    }
}