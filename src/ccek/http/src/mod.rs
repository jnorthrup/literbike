//! Lean HTTP/1.1 Server - relaxfactory pattern
//!
//! Zero-copy, minimal-allocation HTTP server using POSIX select reactor
//!
//! # Example
//!
//! ```rust,no_run
//! use literbike::http::{HttpServer, HttpSession, send_response};
//! use literbike::http::header_parser::{HttpStatus, mime};
//! use literbike::reactor::{Reactor, ReactorService};
//!
//! fn main() -> std::io::Result<()> {
//!     // Create server
//!     let mut server = HttpServer::new("myserver", "127.0.0.1", 8080);
//!     
//!     // Register routes
//!     server.route_fn("/", |session| {
//!         send_response(session, HttpStatus::Status200, mime::TEXT_HTML, b"<h1>Hello!</h1>");
//!     });
//!     
//!     server.route_fn("/api/data", |session| {
//!         send_response(session, HttpStatus::Status200, mime::APPLICATION_JSON, b"{}");
//!     });
//!     
//!     // Create reactor
//!     let mut reactor: Reactor<_, _, _> = Reactor::new()?;
//!     
//!     // Start server with reactor
//!     server.start_with_reactor(&mut reactor)?;
//!     
//!     // Run reactor (in real code, you'd run the event loop)
//!     Ok(())
//! }
//! ```

pub mod header_parser;
pub mod server;
pub mod session;

pub use header_parser::{headers, mime, HeaderParser, HttpMethod, HttpStatus};
pub use server::{send_html, send_json, send_redirect, send_response};
pub use server::{FnHandler, HttpEventHandler, HttpHandler, HttpServer, HttpSessionContainer};
pub use session::HttpSession;

// Re-export userspace network adapters for HTTP protocol integration
#[cfg(feature = "userspace-network")]
pub use crate::userspace_network::adapters::NetworkAdapter;

/// HTTP protocol handler that implements ProtocolHandler for unified detection
use crate::protocol::ProtocolHandler;
use std::io;

pub struct HttpProtocolHandler {
    server: std::sync::Arc<std::sync::Mutex<HttpEventHandler>>,
}

impl HttpProtocolHandler {
    pub fn new(server: HttpEventHandler) -> Self {
        Self {
            server: std::sync::Arc::new(std::sync::Mutex::new(server)),
        }
    }
}

impl ProtocolHandler for HttpProtocolHandler {
    fn handle(&mut self, data: &[u8]) -> io::Result<()> {
        // Parse HTTP request and dispatch to server
        // This is a simplified integration point
        Ok(())
    }

    fn protocol(&self) -> crate::protocol::Protocol {
        crate::protocol::Protocol::Http
    }
}
