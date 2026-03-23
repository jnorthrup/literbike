// special8888 - Port 8888 dynamic protocol sharing
// Self-contained protocol detection and multiplexing for litebike's proxy foundation

use std::io;
use tokio::io::{AsyncRead, AsyncWrite};

/// Default port for dynamic protocol sharing
pub const DEFAULT_PORT: u16 = 8888;

/// HTTP methods detected by byte-sniff
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Head,
    Options,
    Connect,
    Patch,
}

/// Protocol detection result from first-read byte sniff
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolDetection {
    Http(HttpMethod),
    Socks5,
    WebSocket,
    Pac,
    Wpad,
    Upnp,
    Unknown,
}

/// Detect protocol from the first bytes read off the wire.
/// Does not consume the buffer — the caller must replay it.
pub fn detect_protocol(buf: &[u8]) -> ProtocolDetection {
    if buf.is_empty() {
        return ProtocolDetection::Unknown;
    }

    // SOCKS5: first byte 0x05
    if buf[0] == 0x05 {
        return ProtocolDetection::Socks5;
    }

    // Text-based protocols
    if let Ok(text) = std::str::from_utf8(&buf[..buf.len().min(512)]) {
        let text_upper = text.to_uppercase();

        let method = if text.starts_with("GET ") {
            Some(HttpMethod::Get)
        } else if text.starts_with("POST ") {
            Some(HttpMethod::Post)
        } else if text.starts_with("PUT ") {
            Some(HttpMethod::Put)
        } else if text.starts_with("DELETE ") {
            Some(HttpMethod::Delete)
        } else if text.starts_with("HEAD ") {
            Some(HttpMethod::Head)
        } else if text.starts_with("OPTIONS ") {
            Some(HttpMethod::Options)
        } else if text.starts_with("CONNECT ") {
            Some(HttpMethod::Connect)
        } else if text.starts_with("PATCH ") {
            Some(HttpMethod::Patch)
        } else {
            None
        };

        if let Some(method) = method {
            // Refine: WebSocket upgrade overrides plain HTTP
            if text_upper.contains("UPGRADE: WEBSOCKET") {
                return ProtocolDetection::WebSocket;
            }
            // PAC/WPAD file requests
            if text.contains("/wpad.dat") {
                return ProtocolDetection::Wpad;
            }
            if text.contains("/proxy.pac") {
                return ProtocolDetection::Pac;
            }
            return ProtocolDetection::Http(method);
        }

        // UPnP SSDP M-SEARCH / NOTIFY
        if text.starts_with("M-SEARCH ") || text.starts_with("NOTIFY ") {
            return ProtocolDetection::Upnp;
        }
    }

    ProtocolDetection::Unknown
}

/// Wrapper stream that replays already-read bytes before delegating to the inner stream.
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
        if self.prefix_offset < self.prefix.len() {
            let remaining = &self.prefix[self.prefix_offset..];
            let to_copy = remaining.len().min(buf.remaining());
            buf.put_slice(&remaining[..to_copy]);
            self.prefix_offset += to_copy;
            return std::task::Poll::Ready(Ok(()));
        }
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

/// Dynamic protocol listener on a single TCP port.
/// Detects the protocol from the first bytes and dispatches to registered handlers.
pub struct Special8888Listener {
    pub port: u16,
}

impl Special8888Listener {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub fn default_port() -> Self {
        Self::new(DEFAULT_PORT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[test]
    fn detect_socks5() {
        assert_eq!(detect_protocol(&[0x05, 0x01, 0x00]), ProtocolDetection::Socks5);
    }

    #[test]
    fn detect_http_get() {
        let buf = b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n";
        assert_eq!(detect_protocol(buf), ProtocolDetection::Http(HttpMethod::Get));
    }

    #[test]
    fn detect_http_connect() {
        let buf = b"CONNECT example.com:443 HTTP/1.1\r\n\r\n";
        assert_eq!(detect_protocol(buf), ProtocolDetection::Http(HttpMethod::Connect));
    }

    #[test]
    fn detect_websocket() {
        let buf = b"GET /ws HTTP/1.1\r\nUpgrade: websocket\r\n\r\n";
        assert_eq!(detect_protocol(buf), ProtocolDetection::WebSocket);
    }

    #[test]
    fn detect_pac() {
        let buf = b"GET /proxy.pac HTTP/1.1\r\n\r\n";
        assert_eq!(detect_protocol(buf), ProtocolDetection::Pac);
    }

    #[test]
    fn detect_wpad() {
        let buf = b"GET /wpad.dat HTTP/1.1\r\n\r\n";
        assert_eq!(detect_protocol(buf), ProtocolDetection::Wpad);
    }

    #[test]
    fn detect_upnp() {
        let buf = b"M-SEARCH * HTTP/1.1\r\n\r\n";
        assert_eq!(detect_protocol(buf), ProtocolDetection::Upnp);
    }

    #[test]
    fn detect_unknown() {
        assert_eq!(detect_protocol(&[0x00, 0xFF, 0xAA]), ProtocolDetection::Unknown);
        assert_eq!(detect_protocol(&[]), ProtocolDetection::Unknown);
    }

    #[tokio::test]
    async fn prefixed_stream_replays_prefix() {
        let prefix = b"Hello, ".to_vec();
        let inner_data = b"World!";
        let inner = std::io::Cursor::new(inner_data.to_vec());
        let mut s = PrefixedStream::new(inner, prefix);

        let mut result = Vec::new();
        s.read_to_end(&mut result).await.unwrap();
        assert_eq!(result, b"Hello, World!");
    }
}
