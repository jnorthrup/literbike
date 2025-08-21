# Termux/Knox Integration Guide

This branch contains specialized networking tools optimized for Termux environments on Samsung devices with Knox security.

## Features

### Samsung Note 20 5G Platform Support
- **Knox-aware networking**: Works within Samsung's security constraints
- **Carrier evasion techniques**: TTL spoofing, packet fragmentation, TCP fingerprinting
- **Mobile-optimized proxy operations**: HTTP CONNECT, SOCKS5 with mobile fingerprints
- **UPnP aggressive discovery**: Enhanced port mapping for Samsung network stack
- **Tethering bypass**: Circumvent carrier tethering restrictions

### Edge Network Operations
- **Direct syscall networking**: Bypass /proc restrictions in Knox environments  
- **POSIX socket operations**: Work around Android security policies
- **Network namespace awareness**: Detect and work with Knox containers
- **Interface enumeration**: rmnet_data*, wlan0, knox_bridge0 detection

### Pattern Matching Infrastructure
- **SIMD-accelerated scanning**: Fast pattern detection for protocol analysis
- **Glob/regex support**: File and network pattern matching
- **Protocol detection**: HTTP, SOCKS5, TLS identification
- **Carrier middleware detection**: Identify transparent proxy injection

## Usage

### Basic Network Analysis
```bash
# On Termux device
./litebike ifconfig              # Interface status
./litebike route                 # Routing table
./litebike netstat               # Network connections
./litebike carrier               # Carrier detection
```

### Knox Proxy Operations
```bash
# Start Knox-aware proxy
./litebike knox-proxy

# Test proxy functionality  
./litebike proxy-test localhost 8080

# Configure for tethering bypass
./litebike carrier-bypass
```

### Pattern Matching
```bash
# Scan for network patterns
./litebike pattern-scan "*.json" /data/data/com.termux/
./litebike pattern-regex "HTTP/1\\.1 [0-9]{3}" logfile.txt
```

### Development Sync
```bash
# Sync with Termux device over SSH
./scripts/sync_termux.sh

# Configure remote (update IP as needed)
export REMOTE_URL=ssh://u0_a471@192.168.x.x:8022/~/litebike.git
```

## Knox Environment Considerations

### What Knox Restricts
- Standard raw socket creation
- /proc/net filesystem access  
- System iptables modifications
- Some network ioctl operations

### How We Work Around It
- **Direct syscalls**: Use libc calls instead of /proc reads
- **POSIX compliance**: Stay within allowed system calls
- **Mobile fingerprinting**: Mimic legitimate mobile app traffic
- **Namespace awareness**: Work within Knox container constraints

### Carrier Network Evasion
- **TTL normalization**: Set to 64 to avoid tethering detection
- **TCP MSS clamping**: Match Samsung device patterns
- **User-Agent rotation**: Cycle through mobile browser headers
- **DNS override**: Use 8.8.8.8, 1.1.1.1 to bypass carrier DNS

## File Structure

### Core Knox Integration
- `src/knox_proxy.rs` - Main proxy server with Knox awareness
- `src/tethering_bypass.rs` - Carrier restriction evasion
- `src/posix_sockets.rs` - Direct syscall networking
- `src/tcp_fingerprint.rs` - Mobile device TCP characteristics

### Mobile Network Optimization
- `src/packet_fragment.rs` - DPI evasion through fragmentation
- `src/upnp_aggressive.rs` - Enhanced UPnP for Samsung devices
- `src/host_trust.rs` - Private network trust management

### Pattern Matching
- `src/rbcursive/patterns.rs` - SIMD-accelerated pattern scanning
- `src/rbcursive/protocols.rs` - Protocol detection and parsing

## Building for Termux

### Prerequisites
```bash
# On Termux device
pkg install rust git openssh make
```

### Build Process
```bash
# Cross-compile from macOS (if needed)
cargo build --release --target aarch64-linux-android

# Or build directly on Termux
cargo build --release
```

### Installation
```bash
# Copy to Termux PATH
cp target/release/litebike $PREFIX/bin/

# Install completion
mkdir -p $PREFIX/share/bash-completion/completions/
cp completion/litebike-completion.bash $PREFIX/share/bash-completion/completions/litebike
```

## Security Notes

This tool is designed for **legitimate network troubleshooting and testing** in environments where you have proper authorization. The "bypass" techniques are focused on:

- **Carrier policy restrictions** (tethering limits, DNS hijacking)
- **Network troubleshooting** in restrictive environments  
- **Protocol analysis** for debugging connectivity issues

**Not intended for:**
- Circumventing Knox security policies
- Unauthorized network access
- Malicious traffic injection

## Troubleshooting

### Common Issues

**SSH Connection Failed**
- Check IP address in `scripts/sync_termux.sh`
- Ensure SSH server is running: `sshd` 
- Verify port 8022 is accessible

**Knox Restrictions**
- Some operations require root access
- Check SELinux context: `id -Z`
- Verify Termux permissions in device settings

**Carrier Detection**
- Test without VPN first
- Check for carrier-injected headers
- Verify DNS server responses

### Debug Mode
```bash
# Enable verbose logging
RUST_LOG=debug ./litebike knox-proxy

# Test specific features
./litebike pattern-scan --debug "*.log" /data/data/com.termux/
```