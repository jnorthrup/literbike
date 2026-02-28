# Literbike HTTP/1.1 Server

A lean, zero-copy HTTP/1.1 server implementation for literbike, borrowing the relaxfactory NIO reactor pattern.

## Overview

This HTTP server implementation is inspired by the [relaxfactory](https://github.com/jnorthrup/relaxfactory) Java HTTP server, which achieves high throughput (200k+ req/sec) with minimal object allocation using Java NIO.

Key design principles from relaxfactory:
- **Zero-copy header parsing**: Parse HTTP headers directly from byte buffers without string allocation
- **Lazy parsing**: Only parse what's necessary, defer expensive operations
- **Reactor pattern**: Use POSIX select/epoll for scalable I/O multiplexing
- **Keep-alive support**: Reuse connections for multiple requests
- **Minimal allocation**: Reuse buffers where possible

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     HttpServer                               │
│  - Binds to TCP port                                         │
│  - Manages routes                                            │
│  - Provides handler to reactor                               │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   HttpEventHandler                           │
│  - Implements EventHandler trait                             │
│  - on_accept(): Accept new connections                       │
│  - on_read(): Parse headers, route requests                  │
│  - on_write(): Send responses                                │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   HttpSession                                │
│  - Per-connection state                                      │
│  - HeaderParser (buffer + parsed state)                      │
│  - Response buffer                                           │
│  - State machine (ReadingHeaders → Processing → Writing)     │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   HeaderParser                               │
│  - Zero-copy header parsing                                  │
│  - Request/response line parsing                             │
│  - Header extraction                                         │
│  - Response building                                         │
└─────────────────────────────────────────────────────────────┘
```

## Usage

### Basic Server

```rust
use literbike::http::{HttpServer, HttpSession, send_response};
use literbike::http::header_parser::{HttpStatus, mime};

fn main() -> std::io::Result<()> {
    // Create server
    let mut server = HttpServer::new("myserver", "127.0.0.1", 8080);
    
    // Register routes
    server.route_fn("/", |session| {
        send_response(session, HttpStatus::Status200, mime::TEXT_HTML, 
                     b"<h1>Hello!</h1>");
    });
    
    server.route_fn("/api/data", |session| {
        send_response(session, HttpStatus::Status200, mime::APPLICATION_JSON, 
                     b"{}");
    });
    
    // Bind and integrate with reactor
    server.bind()?;
    
    // Use with literbike reactor (see examples/http_server.rs)
    Ok(())
}
```

### Request Handling

```rust
server.route_fn("/api/echo", |session| {
    // Access request info
    let method = session.method();  // Option<HttpMethod>
    let path = session.path();      // Option<&str>
    let headers = session.parser.headers();  // &HashMap<String, String>
    
    // Access body (if Content-Length was set)
    let body = session.body();  // &[u8]
    
    // Send response
    send_response(session, HttpStatus::Status200, mime::TEXT_PLAIN, 
                 b"OK");
});
```

### Response Helpers

```rust
use literbike::http::{send_response, send_json, send_html, send_redirect};

// Simple response
send_response(session, HttpStatus::Status200, mime::TEXT_PLAIN, b"OK");

// JSON response
send_json(session, HttpStatus::Status200, r#"{"key":"value"}"#);

// HTML response
send_html(session, HttpStatus::Status200, "<html>...</html>");

// Redirect
send_redirect(session, "/new-location");
```

## Features

### Header Parsing
- Parses HTTP/1.1 request and response headers
- Extracts method, path, protocol, status code
- Extracts all headers into HashMap
- Tracks Content-Length for body reading
- Supports keep-alive connections

### Session State Machine
```
ReadingHeaders → ReadingBody → Processing → Writing → Done/Reset
```

### Buffer Management
- Pre-allocated header buffer (512 bytes default)
- Dynamic body buffer for Content-Length payloads
- Response buffer for writing
- Buffer reuse on keep-alive reset

## Comparison with relaxfactory

| Feature | relaxfactory (Java) | literbike-http (Rust) |
|---------|-------------------|----------------------|
| I/O Model | Java NIO Selector | POSIX select |
| Header Parsing | ByteBuffer + lazy | Vec<u8> + lazy |
| Allocation | Direct ByteBuffers | Vec with capacity |
| Concurrency | Multi-shard selectors | Single reactor thread |
| Performance | 200k+ req/sec | TBD (early stage) |

## Files

- `header_parser.rs`: HTTP header parsing (like `Rfc822HeaderState`)
- `session.rs`: Per-connection session state (like `Tx`)
- `server.rs`: HTTP server and event handler (like `RxfBenchMarkHttpServer`)
- `mod.rs`: Module exports

## Example

Run the example:
```bash
cargo check --example http_server
```

See `examples/http_server.rs` for complete usage.

## Future Work

- [ ] Full reactor integration (accept → enqueue new connections)
- [ ] Chunked transfer encoding support
- [ ] HTTPS/TLS support
- [ ] WebSocket upgrade
- [ ] Performance benchmarks vs relaxfactory
- [ ] Zero-copy body streaming for large payloads

## License

AGPL-3.0 (same as literbike)
