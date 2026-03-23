//! Lean HTTP/1.1 Server - relaxfactory pattern (Simplified)
//!
//! Zero-copy, minimal-allocation HTTP server using POSIX select reactor
//! Pattern borrowed from relaxfactory RxfBenchMarkHttpServer and ShardNode2

use super::header_parser::{headers, mime, HttpStatus};
use super::session::{HttpSession, SessionState};

use parking_lot::RwLock;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use std::sync::Arc;

pub trait EventHandler: Send + Sync {
    fn on_readable(&mut self, fd: RawFd);
    fn on_writable(&mut self, fd: RawFd);
    fn on_error(&mut self, fd: RawFd, error: std::io::Error);
}

/// HTTP request handler trait
pub trait HttpHandler: Send + Sync {
    /// Handle HTTP request, write response to session
    fn handle(&self, session: &mut HttpSession);
}

/// Simple closure-based handler
pub struct FnHandler<F>(pub F)
where
    F: Fn(&mut HttpSession) + Send + Sync;

impl<F> HttpHandler for FnHandler<F>
where
    F: Fn(&mut HttpSession) + Send + Sync,
{
    fn handle(&self, session: &mut HttpSession) {
        (self.0)(session)
    }
}

/// HTTP Server session
pub struct HttpSessionContainer {
    pub session: HttpSession,
}

/// HTTP Server event handler (like relaxfactory Impl)
pub struct HttpEventHandler {
    /// Request router: path -> handler
    routes: Arc<RwLock<HashMap<String, Arc<dyn HttpHandler>>>>,

    /// Default handler (if no route matches)
    default_handler: Option<Arc<dyn HttpHandler>>,

    /// Server info
    #[allow(dead_code)]
    server_name: String,

    /// Active sessions keyed by file descriptor
    sessions: HashMap<RawFd, HttpSession>,
}

impl HttpEventHandler {
    /// Create new HTTP event handler
    pub fn new(server_name: &str) -> Self {
        Self {
            routes: Arc::new(RwLock::new(HashMap::new())),
            default_handler: None,
            server_name: server_name.to_string(),
            sessions: HashMap::new(),
        }
    }

    /// Register route handler
    pub fn register_route<H: HttpHandler + 'static>(&self, path: &str, handler: H) {
        self.routes
            .write()
            .insert(path.to_string(), Arc::new(handler));
    }

    /// Register closure handler
    pub fn register_route_fn<F>(&self, path: &str, handler: F)
    where
        F: Fn(&mut HttpSession) + Send + Sync + 'static,
    {
        self.register_route(path, FnHandler(handler));
    }

    /// Set default handler
    pub fn set_default_handler<H: HttpHandler + 'static>(&mut self, handler: H) {
        self.default_handler = Some(Arc::new(handler));
    }

    /// Set default handler from closure
    pub fn set_default_handler_fn<F>(&mut self, handler: F)
    where
        F: Fn(&mut HttpSession) + Send + Sync + 'static,
    {
        self.set_default_handler(FnHandler(handler));
    }

    /// Route request to handler
    #[allow(dead_code)]
    fn route_request(&self, session: &mut HttpSession) {
        if let Some(path) = session.path() {
            // Strip query string for routing
            let path = path.split('?').next().unwrap_or(path);

            let routes = self.routes.read();
            if let Some(handler) = routes.get(path) {
                handler.handle(session);
                return;
            }
        }

        // Default handler
        if let Some(ref handler) = self.default_handler {
            handler.handle(session);
        } else {
            // Default 404
            session.prepare_response(HttpStatus::Status404, mime::TEXT_PLAIN, b"404 Not Found");
        }
    }

    /// Get or create session for fd
    pub fn get_session_mut(&mut self, fd: RawFd) -> Option<&mut HttpSession> {
        self.sessions.get_mut(&fd)
    }

    /// Add a new session
    pub fn add_session(&mut self, fd: RawFd, session: HttpSession) {
        self.sessions.insert(fd, session);
    }

    /// Remove a session
    pub fn remove_session(&mut self, fd: RawFd) {
        self.sessions.remove(&fd);
    }
}

impl EventHandler for HttpEventHandler {
    fn on_readable(&mut self, fd: RawFd) {
        // Get path first (before getting mutable session)
        let path = {
            let session = match self.sessions.get(&fd) {
                Some(s) => s,
                None => return,
            };
            session
                .path()
                .map(|p| p.split('?').next().unwrap_or(p).to_string())
        };

        // Get handler if route exists
        let handler = path.and_then(|p| {
            let routes = self.routes.read();
            routes.get(&p).cloned()
        });

        // Now get mutable session and process
        let session = match self.sessions.get_mut(&fd) {
            Some(s) => s,
            None => return,
        };

        // Read from socket - create temporary stream for reading
        let mut buf = [0u8; 1024];
        let n = unsafe {
            let mut stream = TcpStream::from_raw_fd(fd);
            let result = stream.read(&mut buf);
            let _ = stream.into_raw_fd();
            result
        };

        match n {
            Ok(0) => {
                // EOF - close connection
                self.sessions.remove(&fd);
            }
            Ok(bytes_read) => {
                session.parser.append(&buf[..bytes_read]);

                // Try to parse headers
                match session.try_parse_headers() {
                    Ok(true) => {
                        // Headers complete
                        if session.body_complete() {
                            session.finish_reading_body();
                            // Route the request if we have a handler
                            if let Some(ref h) = handler {
                                h.handle(session);
                            }
                        }
                    }
                    Ok(false) => {
                        // Need more data
                    }
                    Err(_) => {
                        // Parse error - send 400
                        session.prepare_response(
                            HttpStatus::Status400,
                            mime::TEXT_PLAIN,
                            b"400 Bad Request",
                        );
                    }
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // Would block, try again later
            }
            Err(_) => {
                // Read error - remove session
                self.sessions.remove(&fd);
            }
        }
    }

    fn on_writable(&mut self, fd: RawFd) {
        let session = match self.sessions.get_mut(&fd) {
            Some(s) => s,
            None => return,
        };

        if session.response_buffer.is_empty() {
            return;
        }

        // Write to socket
        let result = unsafe {
            let mut stream = TcpStream::from_raw_fd(fd);
            stream.set_nonblocking(true).ok();
            let result = stream.write(&session.response_buffer);
            let _ = stream.into_raw_fd();
            result
        };

        match result {
            Ok(bytes_written) => {
                if bytes_written >= session.response_buffer.len() {
                    session.response_buffer.clear();

                    if session.keep_alive {
                        session.reset();
                    } else {
                        session.state = SessionState::Done;
                        self.sessions.remove(&fd);
                    }
                } else {
                    session.response_buffer.drain(..bytes_written);
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // Try again later
            }
            Err(_) => {
                // Write error - remove session
                self.sessions.remove(&fd);
            }
        }
    }

    fn on_error(&mut self, fd: RawFd, error: io::Error) {
        log::error!("HTTP error on FD {}: {}", fd, error);
        // Remove session on error
        self.sessions.remove(&fd);
    }
}

/// Lean HTTP Server (like RxfBenchMarkHttpServer)
pub struct HttpServer {
    /// Server name
    name: String,

    /// Bind address
    addr: String,

    /// Port
    port: u16,

    /// Event handler
    handler: Arc<HttpEventHandler>,

    /// TCP Listener
    listener: Option<TcpListener>,

    /// Running state
    running: bool,
}

impl HttpServer {
    /// Create new HTTP server
    pub fn new(name: &str, addr: &str, port: u16) -> Self {
        Self {
            name: name.to_string(),
            addr: addr.to_string(),
            port,
            handler: Arc::new(HttpEventHandler::new(name)),
            listener: None,
            running: false,
        }
    }

    /// Get server name (like RxfBenchMarkHttpServer.getName)
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get bind address
    pub fn addr(&self) -> &str {
        &self.addr
    }

    /// Get port
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Register route handler
    pub fn route<H: HttpHandler + 'static>(&self, path: &str, handler: H) {
        self.handler.register_route(path, handler);
    }

    /// Register closure handler
    pub fn route_fn<F>(&self, path: &str, handler: F)
    where
        F: Fn(&mut HttpSession) + Send + Sync + 'static,
    {
        self.handler.register_route_fn(path, handler);
    }

    /// Bind to address (like RxfBenchMarkHttpServer.start)
    pub fn bind(&mut self) -> io::Result<()> {
        let bind_addr = format!("{}:{}", self.addr, self.port);
        let listener = TcpListener::bind(&bind_addr)?;
        listener.set_nonblocking(true)?;

        log::info!("HTTP Server '{}' listening on {}", self.name, bind_addr);

        self.listener = Some(listener);
        Ok(())
    }

    /// Get handler for reactor
    pub fn handler(&self) -> Arc<HttpEventHandler> {
        self.handler.clone()
    }

    /// Get listener FD for manual registration with reactor
    pub fn listener_fd(&self) -> Option<RawFd> {
        self.listener.as_ref().map(|l| l.as_raw_fd())
    }

    /// Stop server
    pub fn stop(&mut self) {
        self.running = false;
        if let Some(ref _listener) = self.listener {
            log::info!("HTTP Server '{}' stopping", self.name);
            // Drop listener
            self.listener = None;
        }
    }

    /// Check if running
    pub fn is_running(&self) -> bool {
        self.running
    }
}

/// Helper: send simple HTTP response
pub fn send_response(
    session: &mut HttpSession,
    status: HttpStatus,
    content_type: &str,
    body: &[u8],
) {
    session.prepare_response(status, content_type, body);
}

/// Helper: send JSON response
pub fn send_json(session: &mut HttpSession, status: HttpStatus, json: &str) {
    session.prepare_response(status, mime::APPLICATION_JSON, json.as_bytes());
}

/// Helper: send HTML response
pub fn send_html(session: &mut HttpSession, status: HttpStatus, html: &str) {
    session.prepare_response(status, mime::TEXT_HTML, html.as_bytes());
}

/// Helper: send redirect
pub fn send_redirect(session: &mut HttpSession, location: &str) {
    session.parser.set_status(HttpStatus::Status302);
    session.parser.set_header(headers::LOCATION, location);
    session.prepare_response(HttpStatus::Status302, mime::TEXT_PLAIN, b"Redirect");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_creation() {
        let server = HttpServer::new("test", "127.0.0.1", 8080);
        assert_eq!(server.name(), "test");
        assert_eq!(server.port(), 8080);
        assert!(!server.is_running());
    }

    #[test]
    fn test_route_registration() {
        let server = HttpServer::new("test", "127.0.0.1", 8080);

        server.route_fn("/test", |session| {
            send_response(session, HttpStatus::Status200, mime::TEXT_PLAIN, b"OK");
        });

        // Route should be registered
        let routes = server.handler.routes.read();
        assert!(routes.contains_key("/test"));
    }

    #[test]
    fn test_send_helpers() {
        let mut session = HttpSession::new();

        send_response(
            &mut session,
            HttpStatus::Status200,
            mime::TEXT_PLAIN,
            b"Hello",
        );
        assert!(session.wants_write());

        session.reset();
        send_json(&mut session, HttpStatus::Status200, r#"{"key":"value"}"#);
        assert!(session.wants_write());

        session.reset();
        send_redirect(&mut session, "/new-location");
        assert_eq!(session.parser.status(), Some(HttpStatus::Status302));
    }
}
