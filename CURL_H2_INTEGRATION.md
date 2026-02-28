# QUIC Server with curl-h2 Integration

## Overview

This additive integration adds HTTP/2 client testing capabilities to the literbike QUIC server using curl with HTTP/2 support. The implementation allows testing the QUIC server's HTTP/3 (H3) over QUIC capabilities using HTTP/2 curl clients.

## Added Components

### 1. New Feature Flag: `curl-h2`

Added to `Cargo.toml`:
```toml
curl-h2 = ["dep:curl", "dep:h2", "dep:http", "dep:tokio-io", "dep:clap"]
```

### 2. New Module: `src/curl_h2/`

A complete HTTP/2 client implementation with:

- **mod.rs** - Module exports
- **error.rs** - Error types (`H2Error`)
- **request.rs** - Request builder (`H2Request`)
- **response.rs** - Response structure (`H2Response`)
- **client.rs** - HTTP/2 client implementation (`H2Client`)

### 3. New Binary: `src/bin/quic_curl_h2.rs`

Command-line tool for testing QUIC server HTTP/2 endpoints with the following capabilities:
- Fetch UI test pattern assets (index.html, index.css, bw_test_pattern.png)
- Verify HTTP/2 protocol support
- Download and save responses
- Verbose output mode
- Custom timeout and SSL verification options

## Usage

### Build with curl-h2 feature

```bash
cargo build --bin quic_curl_h2 --features curl-h2
```

### Run the test client

```bash
# Test default server (https://localhost:4433)
./target/debug/quic_curl_h2

# Test with custom URL
./target/debug/quic_curl_h2 -u https://localhost:4433

# Verbose output
./target/debug/quic_curl_h2 -v

# Save downloaded files
./target/debug/quic_curl_h2 -o ./downloaded

# Test specific path
./target/debug/quic_curl_h2 -p /index.css

# With custom timeout
./target/debug/quic_curl_h2 -t 60
```

### Programmatic Usage

```rust
use literbike::curl_h2::{H2Client, H2Request, H2Response};

// Create client
let mut client = H2Client::new()?;

// Simple GET request
let response = client.get("https://localhost:4433/")?;
println!("Status: {}", response.status);
println!("Body: {} bytes", response.body.len());

// Custom request
let request = H2Request::get("https://localhost:4433/index.css")
    .header("User-Agent", "literbike-test/0.1")
    .timeout(30)
    .build();

let response = client.request(request)?;
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    QUIC Server (port 4433)                   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  quic_server.rs - UDP socket, RbCursive preflight    │   │
│  │  quic_engine.rs - Connection state, stream handling  │   │
│  │  Serves: index.html, index.css, bw_test_pattern.png  │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              ↕ QUIC/UDP
┌─────────────────────────────────────────────────────────────┐
│                  curl-h2 Test Client                         │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  H2Client - curl easy handle with HTTP/2 enabled     │   │
│  │  H2Request - Request builder with fluent API         │   │
│  │  H2Response - Response with headers, body, status    │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Test Pattern Assets

The QUIC server hosts the following UI test pattern files:

1. **index.html** - Main HTML page with test pattern display
2. **index.css** - Styling for the test pattern UI
3. **bw_test_pattern.png** - Black & white TV test pattern image

These assets are served over QUIC with HTTP/3 framing, and the curl-h2 client tests the server's ability to handle HTTP/2 connections.

## No Code Pruning

This integration is **purely additive**:
- No existing code was removed or modified
- No bypasses of existing functionality
- All new code is feature-gated behind `curl-h2`
- Existing QUIC server functionality remains unchanged

## Dependencies Added

- `curl` v0.4 with HTTP/2 support
- `h2` v0.4 - HTTP/2 protocol library
- `http` v1.0 - HTTP types
- `clap` v4.0 with derive - CLI argument parsing
- `tokio-io` v0.1 - Async IO (for h2 compatibility)

## Future Enhancements

1. Add HTTP/3 (h3) client support using quinn library
2. Integrate with existing quic_smoke_test binary
3. Add benchmarking capabilities
4. Support for concurrent stream testing
5. ALPN negotiation verification
