# LiteBike Proxy

A lightweight, high-performance proxy server written in Rust, designed for mobile and embedded environments. Supports both HTTP/HTTPS and SOCKS5 protocols with intelligent network interface routing and comprehensive protocol detection.

## Core Components

### PAC Server (Port 8888)
- Serves proxy auto-configuration file
- URL: `http://$TERMUX_HOST:8888/proxy.pac`
 

### Universal HTTP Proxy (Port 8080)
- Handles HTTP, HTTPS, and CONNECT tunneling
- Protocol detection on single port
- Bridges WiFi (swlan0) to mobile data (rmnet)

### Compliance Ports
Individual protocol ports for strict compliance requirements:
- **1080**: SOCKS5 (RFC 1928 compliant)
- **8443**: Direct TLS proxy
- **3128**: Squid-compatible HTTP
- **1900**: UPnP/SSDP discovery

## Client Configuration

### Automatic (via PAC)
```
Proxy Auto-Config URL: http://$TERMUX_HOST:8888/proxy.pac
```

### Manual
```
HTTP Proxy:  $TERMUX_HOST:8080
HTTPS Proxy: $TERMUX_HOST:8080
SOCKS Proxy: $TERMUX_HOST:1080
```

## Sample PAC File
```javascript
function FindProxyForURL(url, host) {
  if (isInNet(host, "10.0.0.0", "255.0.0.0"))
    return "DIRECT";
  return "PROXY $TERMUX_HOST:8080; SOCKS $TERMUX_HOST:1080; DIRECT";
}
```

<<<<<<< HEAD
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
=======
## Termux-Specific Notes
- $TERMUX_HOST: Auto-detected swlan0 IP address
- Ingress: WiFi interface (swlan0)
- Egress: Mobile data (rmnet_data*)
- Purpose: Share mobile data via WiFi proxy bridge
>>>>>>> 102b8e2 (feat: Add Termux ARM64 build and complete proxy implementation)
