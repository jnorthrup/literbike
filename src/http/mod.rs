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
pub mod session;
pub mod server;

pub use header_parser::{HttpMethod, HttpStatus, HeaderParser, headers, mime};
pub use session::HttpSession;
pub use server::{HttpServer, HttpHandler, FnHandler, HttpEventHandler, HttpSessionContainer};
pub use server::{send_response, send_json, send_html, send_redirect};
