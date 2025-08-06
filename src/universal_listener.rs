use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::net::TcpStream;
use log::{debug, info};

/// Protocol detection result
#[derive(PartialEq, Debug)]
pub enum Protocol {
    Http,
    Socks5,
    WebSocket,
    WebRTC,
    Pac,        // Proxy Auto-Config
    Wpad,       // Web Proxy Auto-Discovery
    Bonjour,    // mDNS/DNS-SD
    Upnp,       // UPnP discovery
    Unknown,
}

/// Detects the protocol based on the first few bytes
pub async fn detect_protocol<S>(stream: &mut S) -> io::Result<(Protocol, Vec<u8>)>
where
    S: AsyncRead + Unpin,
{
    let mut buffer = vec![0u8; 1024];
    let n = stream.read(&mut buffer).await?;
    
    if n == 0 {
        return Ok((Protocol::Unknown, vec![]));
    }
    
    buffer.truncate(n);
    
    // SOCKS5 starts with version byte 0x05
    if n >= 2 && buffer[0] == 0x05 {
        debug!("Detected SOCKS5 protocol");
        return Ok((Protocol::Socks5, buffer));
    }
    
    // Check for text-based protocols
    if let Ok(text) = std::str::from_utf8(&buffer[..std::cmp::min(n, 512)]) {
        let text_upper = text.to_uppercase();
        
        // HTTP methods: GET, POST, PUT, DELETE, HEAD, OPTIONS, CONNECT, PATCH
        if text.starts_with("GET ") || 
           text.starts_with("POST ") || 
           text.starts_with("PUT ") || 
           text.starts_with("DELETE ") || 
           text.starts_with("HEAD ") || 
           text.starts_with("OPTIONS ") || 
           text.starts_with("CONNECT ") || 
           text.starts_with("PATCH ") {
            
            // Check for WebSocket upgrade
            if text_upper.contains("UPGRADE: WEBSOCKET") {
                debug!("Detected WebSocket protocol");
                return Ok((Protocol::WebSocket, buffer));
            }
            
            // Check for PAC file request
            if text.contains("/proxy.pac") || text.contains("/wpad.dat") {
                if text.contains("/wpad.dat") {
                    debug!("Detected WPAD request");
                    return Ok((Protocol::Wpad, buffer));
                } else {
                    debug!("Detected PAC request");
                    return Ok((Protocol::Pac, buffer));
                }
            }
            
            debug!("Detected HTTP protocol");
            return Ok((Protocol::Http, buffer));
        }
        
        // UPnP M-SEARCH (SSDP)
        if text.starts_with("M-SEARCH ") {
            debug!("Detected UPnP M-SEARCH");
            return Ok((Protocol::Upnp, buffer));
        }
        
        // UPnP NOTIFY
        if text.starts_with("NOTIFY ") {
            debug!("Detected UPnP NOTIFY");
            return Ok((Protocol::Upnp, buffer));
        }
    }
    
    // Binary protocol detection
    
    // WebRTC STUN binding request (starts with 0x00 0x01)
    if n >= 20 && buffer[0] == 0x00 && buffer[1] == 0x01 {
        // STUN magic cookie at bytes 4-7: 0x2112A442
        if n >= 8 && 
           buffer[4] == 0x21 && 
           buffer[5] == 0x12 && 
           buffer[6] == 0xA4 && 
           buffer[7] == 0x42 {
            debug!("Detected WebRTC STUN");
            return Ok((Protocol::WebRTC, buffer));
        }
    }
    
    // mDNS/Bonjour (DNS packets on port 5353)
    // DNS header starts with transaction ID (2 bytes) followed by flags
    // mDNS typically has flags with QR=0 (query) or QR=1 (response)
    if n >= 12 {
        // Check for DNS-like structure
        let flags = (buffer[2] as u16) << 8 | buffer[3] as u16;
        let opcode = (flags >> 11) & 0x0F;
        
        // Standard query
        if opcode == 0 {
            // Check for mDNS multicast bit (bit 15)
            if (flags & 0x8000) != 0 {
                debug!("Detected Bonjour/mDNS protocol");
                return Ok((Protocol::Bonjour, buffer));
            }
        }
    }
    
    debug!("Unknown protocol detected");
    Ok((Protocol::Unknown, buffer))
}

/// Wrapper stream that prefixes read operations with buffered data
pub struct PrefixedStream<S> {
    pub inner: S,
    prefix: Vec<u8>,
    prefix_offset: usize,
}

impl<S> PrefixedStream<S> {
    pub fn new(inner: S, prefix: Vec<u8>) -> Self {
        Self {
            inner,
            prefix,
            prefix_offset: 0,
        }
    }
}

impl<S> AsyncRead for PrefixedStream<S>
where
    S: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        // First, drain the prefix buffer
        if self.prefix_offset < self.prefix.len() {
            let remaining = &self.prefix[self.prefix_offset..];
            let to_copy = std::cmp::min(remaining.len(), buf.remaining());
            buf.put_slice(&remaining[..to_copy]);
            self.prefix_offset += to_copy;
            return std::task::Poll::Ready(Ok(()));
        }
        
        // Then read from the inner stream
        std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<S> AsyncWrite for PrefixedStream<S>
where
    S: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, io::Error>> {
        std::pin::Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// Handler function type
pub type ProtocolHandler = Box<dyn Fn(PrefixedStream<TcpStream>) -> std::pin::Pin<Box<dyn std::future::Future<Output = io::Result<()>> + Send>> + Send + Sync>;

/// Protocol handlers collection
pub struct ProtocolHandlers {
    pub http: ProtocolHandler,
    pub socks5: ProtocolHandler,
    pub websocket: Option<ProtocolHandler>,
    pub webrtc: Option<ProtocolHandler>,
    pub pac: Option<ProtocolHandler>,
    pub wpad: Option<ProtocolHandler>,
    pub bonjour: Option<ProtocolHandler>,
    pub upnp: Option<ProtocolHandler>,
}

/// Handle a connection with protocol detection
pub async fn handle_connection(
    mut stream: TcpStream,
    handlers: &ProtocolHandlers,
) -> io::Result<()> {
    let peer_addr = stream.peer_addr()?;
    info!("New connection from {}", peer_addr);
    
    let (protocol, buffer) = detect_protocol(&mut stream).await?;
    
    // Create a prefixed stream that includes the already-read bytes
    let prefixed_stream = PrefixedStream::new(stream, buffer);
    
    match protocol {
        Protocol::Http => {
            info!("Routing {} to HTTP handler", peer_addr);
            (handlers.http)(prefixed_stream).await
        }
        Protocol::Socks5 => {
            info!("Routing {} to SOCKS5 handler", peer_addr);
            (handlers.socks5)(prefixed_stream).await
        }
        Protocol::WebSocket => {
            if let Some(ref handler) = handlers.websocket {
                info!("Routing {} to WebSocket handler", peer_addr);
                handler(prefixed_stream).await
            } else {
                info!("WebSocket detected from {} but no handler configured, falling back to HTTP", peer_addr);
                (handlers.http)(prefixed_stream).await
            }
        }
        Protocol::WebRTC => {
            if let Some(ref handler) = handlers.webrtc {
                info!("Routing {} to WebRTC handler", peer_addr);
                handler(prefixed_stream).await
            } else {
                info!("WebRTC protocol from {} but no handler configured", peer_addr);
                Err(io::Error::new(io::ErrorKind::InvalidData, "WebRTC not supported"))
            }
        }
        Protocol::Pac => {
            if let Some(ref handler) = handlers.pac {
                info!("Routing {} to PAC handler", peer_addr);
                handler(prefixed_stream).await
            } else {
                info!("PAC request from {} but no handler configured, using HTTP", peer_addr);
                (handlers.http)(prefixed_stream).await
            }
        }
        Protocol::Wpad => {
            if let Some(ref handler) = handlers.wpad {
                info!("Routing {} to WPAD handler", peer_addr);
                handler(prefixed_stream).await
            } else {
                info!("WPAD request from {} but no handler configured, using HTTP", peer_addr);
                (handlers.http)(prefixed_stream).await
            }
        }
        Protocol::Bonjour => {
            if let Some(ref handler) = handlers.bonjour {
                info!("Routing {} to Bonjour handler", peer_addr);
                handler(prefixed_stream).await
            } else {
                info!("Bonjour protocol from {} but no handler configured", peer_addr);
                Err(io::Error::new(io::ErrorKind::InvalidData, "Bonjour not supported"))
            }
        }
        Protocol::Upnp => {
            if let Some(ref handler) = handlers.upnp {
                info!("Routing {} to UPnP handler", peer_addr);
                handler(prefixed_stream).await
            } else {
                info!("UPnP protocol from {} but no handler configured", peer_addr);
                Err(io::Error::new(io::ErrorKind::InvalidData, "UPnP not supported"))
            }
        }
        Protocol::Unknown => {
            info!("Unknown protocol from {}, closing connection", peer_addr);
            Err(io::Error::new(io::ErrorKind::InvalidData, "Unknown protocol"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_detect_http_get() {
        let data = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        let mut cursor = std::io::Cursor::new(data.to_vec());
        
        let (protocol, buffer) = detect_protocol(&mut cursor).await.unwrap();
        matches!(protocol, Protocol::Http);
        assert_eq!(buffer, data.to_vec());
    }

    #[tokio::test]
    async fn test_detect_socks5() {
        let data = b"\x05\x01\x00"; // SOCKS5, 1 method, no auth
        let mut cursor = std::io::Cursor::new(data.to_vec());
        
        let (protocol, buffer) = detect_protocol(&mut cursor).await.unwrap();
        matches!(protocol, Protocol::Socks5);
        assert_eq!(buffer, data.to_vec());
    }

    #[tokio::test]
    async fn test_prefixed_stream() {
        let prefix = b"Hello, ".to_vec();
        let inner_data = b"World!";
        let inner = std::io::Cursor::new(inner_data.to_vec());
        
        let mut prefixed = PrefixedStream::new(inner, prefix.clone());
        
        let mut result = Vec::new();
        prefixed.read_to_end(&mut result).await.unwrap();
        
        assert_eq!(result, b"Hello, World!");
    }
}