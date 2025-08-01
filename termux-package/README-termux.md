# LiteBike Proxy for Termux

A lightweight HTTP/HTTPS and SOCKS5 proxy server optimized for Termux on Android ARM64.

## Installation

1. Copy the `termux-package` directory to your Termux device
2. In Termux, navigate to the directory: `cd termux-package`
3. Run: `bash install.sh`

## Usage

Start the proxy server:
```bash
litebike-proxy
```

Bind to all interfaces (allows connections from other devices):
```bash
BIND_IP=0.0.0.0 litebike-proxy
```

## Proxy Configuration

Configure your browser or apps to use:
- **HTTP/HTTPS Proxy**: `<termux-ip>:8080`  
- **SOCKS5 Proxy**: `<termux-ip>:1080`

To find your Termux device IP:
```bash
ip route get 1 | awk '{print $7}'
```
or
```bash
ifconfig | grep 'inet ' | grep -v 127.0.0.1
```

## Features

- ✅ HTTP/HTTPS proxy with CONNECT tunneling
- ✅ SOCKS5 proxy support
- ✅ IPv4 and IPv6 support
- ✅ Lightweight and fast (1.1MB binary)
- ✅ No external dependencies
- ✅ Perfect for mobile data sharing
- ✅ Termux-optimized build

## Advanced Usage

### Share Mobile Data via WiFi Hotspot

1. Enable mobile hotspot on Android
2. Run in Termux: `BIND_IP=0.0.0.0 litebike-proxy`
3. Connect other devices to your hotspot
4. Configure them to use `<hotspot-ip>:8080` as HTTP proxy
5. Enjoy shared mobile data!

### Background Operation

Run in background with nohup:
```bash
nohup litebike-proxy > /dev/null 2>&1 &
```

### Logs

Enable debug logging:
```bash
RUST_LOG=debug litebike-proxy
```

## Architecture

- **Target**: aarch64-linux-android (ARM64)
- **Runtime**: Tokio async
- **Binary Size**: ~1.1MB (stripped)
- **Memory**: Low footprint
- **Performance**: High throughput

## Troubleshooting

**Permission denied**: Make sure binary is executable
```bash
chmod +x $PREFIX/bin/litebike-proxy
```

**Port already in use**: Check if another service is using ports 8080/1080
```bash
netstat -tlnp | grep -E ':(8080|1080)'
```

**Connection refused**: Ensure you're binding to the correct interface
```bash
BIND_IP=0.0.0.0 litebike-proxy  # For external access
```