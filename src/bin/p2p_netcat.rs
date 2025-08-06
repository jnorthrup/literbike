
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};
use log::{info, debug, error};
use std::io;
use tokio::time::timeout;
use std::time::Duration;
use litebike::bonjour::BonjourDiscovery;

const LOCAL_PORT: u16 = 8888;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Connects to a target address with timeout
async fn connect_to_target(target_addr: SocketAddr) -> io::Result<TcpStream> {
    debug!("Connecting to {}", target_addr);
    match timeout(CONNECT_TIMEOUT, TcpStream::connect(target_addr)).await {
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


#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("🚀 P2P Netcat with Auto-Discovery");

    // 1. Discover the peer's address
    let bonjour = BonjourDiscovery::new().expect("Failed to initialize Bonjour");
    let peer_addr = loop {
        info!("Discovering LiteBike peers...");
        let mut found_peer = None;
        for service in bonjour.discover_peers() {
            if let Some(ipv4) = service.get_addresses().iter().find_map(|addr| {
                match addr {
                    IpAddr::V4(ipv4) => Some(ipv4),
                    _ => None,
                }
            }) {
                found_peer = Some(SocketAddr::new(IpAddr::V4(*ipv4), service.get_port()));
                info!("Discovered LiteBike peer at {}", found_peer.unwrap());
                break;
            }
        }
        if let Some(addr) = found_peer {
            break addr;
        } else {
            info!("No LiteBike peers found, retrying in 5 seconds...");
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    };

    // 2. Listen for local connections on localhost:8888
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), LOCAL_PORT))
        .await
        .expect("Failed to bind to local port");

    info!("Listening on http://localhost:{}", LOCAL_PORT);


    // 3. Accept connections and forward them to the peer
    loop {
        if let Ok((stream, addr)) = listener.accept().await {
            debug!("Accepted connection from {}", addr);
            
            tokio::spawn(async move {
                info!("Connecting to peer at {}", peer_addr);
                match connect_to_target(peer_addr).await {
                    Ok(remote_stream) => {
                        info!("Connection to peer successful. Relaying traffic.");
                        if let Err(e) = relay_streams(stream, remote_stream).await {
                            error!("Relay error: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to connect to peer: {}", e);
                    }
                }
            });
        }
    }
}
