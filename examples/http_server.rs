//! Lean HTTP/1.1 Server Example - relaxfactory pattern
//!
//! This example demonstrates a minimal HTTP server using the literbike HTTP module,
//! borrowing the relaxfactory NIO reactor pattern.
//!
//! Run with: cargo run --example http_server

use literbike::http::header_parser::{mime, HttpMethod, HttpStatus};
use literbike::http::{send_json, send_response, HttpServer, HttpSession, HttpSessionContainer};

fn main() -> std::io::Result<()> {
    env_logger::init();

    println!("=== Literbike Lean HTTP Server ===");
    println!("Pattern borrowed from relaxfactory (Java NIO HTTP server)");
    println!();

    // Create server
    let mut server = HttpServer::new("literbike-example", "127.0.0.1", 8888);

    // Register routes
    server.route_fn("/", |session| {
        send_response(
            session,
            HttpStatus::Status200,
            mime::TEXT_HTML,
            b"<html><body><h1>Hello from Literbike!</h1><p>Lean HTTP/1.1 server inspired by relaxfactory</p></body></html>",
        );
    });

    server.route_fn("/api/health", |session| {
        send_json(
            session,
            HttpStatus::Status200,
            r#"{"status":"healthy","server":"literbike"}"#,
        );
    });

    server.route_fn("/api/echo", |session| {
        // Echo back the request method and path
        let method = session.method().map(|m| m.as_str()).unwrap_or("UNKNOWN");
        let path = session.path().unwrap_or("/unknown");
        let response = format!(r#"{{"method":"{}","path":"{}"}}"#, method, path);
        send_json(session, HttpStatus::Status200, &response);
    });

    server.route_fn("/api/data", |session| {
        // Only handle GET requests
        if session.method() == Some(HttpMethod::GET) {
            send_json(
                session,
                HttpStatus::Status200,
                r#"{"items":[{"id":1,"name":"Item 1"},{"id":2,"name":"Item 2"}]}"#,
            );
        } else {
            send_response(
                session,
                HttpStatus::Status405,
                mime::TEXT_PLAIN,
                b"Method Not Allowed",
            );
        }
    });

    // Default handler (404 for unmatched routes)
    // Note: For this example, unmatched routes will return 404 from the built-in default
    // To set a custom default handler, you would need to access the handler before wrapping in Arc
    // server.handler().set_default_handler_fn(|session| { ... });

    println!(
        "Server '{}' starting on http://{}:{}",
        server.name(),
        server.addr(),
        server.port()
    );
    println!();
    println!("Available endpoints:");
    println!("  GET  /              - HTML welcome page");
    println!("  GET  /api/health    - Health check (JSON)");
    println!("  GET  /api/echo      - Echo request info (JSON)");
    println!("  GET  /api/data      - Sample data (JSON)");
    println!();
    println!("Try: curl http://127.0.0.1:8888/api/health");
    println!();

    // Note: This example shows the API usage
    // For a full running server, you would integrate with the reactor:
    //
    // let mut reactor: Reactor<HttpSessionContainer, HttpEventHandler> = Reactor::new()?;
    // server.bind()?;
    // if let Some(fd) = server.listener_fd() {
    //     reactor.enqueue_register(
    //         fd,
    //         HttpSessionContainer { session: HttpSession::new() },
    //         server.handler(),
    //         Interest::ACCEPT,
    //     );
    //     reactor.run(server.handler())?;
    // }

    println!("Note: This example demonstrates the API.");
    println!("To run a full server, uncomment the reactor integration code in the example.");

    Ok(())
}
