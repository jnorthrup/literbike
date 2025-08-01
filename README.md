# Litebike Proxy

A lightweight, high-performance proxy server written in Rust, designed for mobile and embedded environments. Supports both HTTP/HTTPS and SOCKS5 protocols with intelligent network interface routing.

## Features

- **Dual Protocol Support**: HTTP/HTTPS proxy on port 8080, SOCKS5 on port 1080
- **Smart Routing**: Configurable ingress/egress interfaces for mobile data optimization
- **Minimal Dependencies**: Pure Rust implementation with tokio async runtime
- **Auto-configuration**: Includes proxy-bridge script for easy setup

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/jnorthrup/litebike.git
cd litebike

# Build with cargo
cargo build --release

# Copy binary to home directory
cp target/release/litebike-proxy ~/
```

### Quick Start

```bash
# Start with default configuration (ingress=local_ip, egress=0.0.0.0)
./litebike-proxy

# Or use the proxy-bridge script for full system configuration
./scripts/proxy-bridge server
```

## Usage

### Environment Variables

- `BIND_IP` - IP address to bind to (default: 0.0.0.0)
- `EGRESS_IP` - Egress IP for outbound connections (default: 0.0.0.0)
- `EGRESS_INTERFACE` - Specific network interface for egress (e.g., rmnet_data0)
- `HTTP_PORT` - HTTP/HTTPS proxy port (default: 8080)
- `SOCKS_PORT` - SOCKS5 proxy port (default: 1080)

### Examples

```bash
# Bind to specific IP with rmnet egress
BIND_IP=192.168.1.100 EGRESS_INTERFACE=rmnet_data0 ./litebike-proxy

# Use specific IPs for ingress and egress
BIND_IP=192.168.1.100 EGRESS_IP=10.0.0.1 ./litebike-proxy
```

## Proxy Bridge Script

The included `scripts/proxy-bridge` script provides comprehensive proxy management:

- Auto-discovery of gateway IPs
- SSH remote server startup
- System-wide proxy configuration for macOS/Linux
- Developer tool integration (git, npm, curl, VSCode, etc.)

See [scripts/proxy-bridge](scripts/proxy-bridge) for detailed usage.

## Architecture

Litebike uses Tokio for async I/O and implements both HTTP CONNECT tunneling and SOCKS5 protocol handling in a single binary. The design prioritizes:

1. **Performance**: Minimal overhead, efficient buffer management
2. **Compatibility**: Works with mobile platform restrictions
3. **Flexibility**: Configurable routing for complex network setups
4. **Simplicity**: Single binary, no external dependencies

## Building

Requirements:
- Rust 1.70+ with cargo
- Tokio runtime dependencies

```bash
cargo build --release
```

## License

MIT License - See LICENSE file for details

## Contributing

Contributions welcome! Please submit pull requests or issues on GitHub.
