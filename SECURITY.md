# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 1.0.x   | :white_check_mark: |
| < 1.0   | :x:                |

## Reporting a Vulnerability

### For AGPL Users
Report security vulnerabilities by opening a GitHub issue marked "Security" - these will be handled privately.

### For Commercial License Holders
- **Critical**: Direct email with 4-hour response SLA
- **High/Medium**: Priority support queue
- **Low**: Regular issue tracking

## Security Features

### Memory Safety
- Written in Rust - prevents buffer overflows, use-after-free
- No unsafe blocks in critical paths
- Bounds checking on all protocol parsing

### Protocol Security
- TLS 1.2+ enforced by default
- Certificate validation
- SNI extraction without decryption
- Constant-time comparisons for auth

### Network Security
- Rate limiting per IP
- Connection limits
- Configurable bind addresses
- Optional authentication layer

## Known Security Considerations

### Default Configuration
⚠️ **Default binds to 0.0.0.0** - This is intentional for mobile data sharing use case but may expose the proxy to external networks. In secure environments:

```bash
# Restrict to localhost only
BIND_IP=127.0.0.1 litebike-proxy

# Disable discovery protocols
DISABLE_UPNP=true litebike-proxy
```

### Protocol Detection Timing
The protocol detection provides O(k) protocol detection but may leak protocol type via timing. This is acceptable for a proxy but should be considered in high-security environments.

### Resource Limits
Configure appropriate limits:
```bash
# Limit connections
MAX_CONNECTIONS=10000 litebike-proxy

# Rate limiting
RATE_LIMIT_PER_IP=100 litebike-proxy
```

## Security Checklist

- [ ] Review bind address for deployment environment
- [ ] Configure firewall rules appropriately  
- [ ] Enable authentication if needed
- [ ] Set resource limits for production
- [ ] Monitor for unusual traffic patterns
- [ ] Keep binary updated for security patches

## Disclosure Timeline

1. **Report received**: Acknowledge within 24h
2. **Triage**: Severity assessment within 72h
3. **Fix development**: Based on severity
4. **Testing**: Internal validation
5. **Release**: Coordinated disclosure
6. **Public disclosure**: After patch availability

## Security Hardening

### Compile-time Options
```bash
# Build with additional hardening
RUSTFLAGS="-C target-cpu=native -C opt-level=3 -C lto=fat -C codegen-units=1" \
  cargo build --release
```

### Runtime Hardening
```bash
# Linux: Use seccomp filters
litebike-proxy --seccomp-filter

# Capability dropping
setcap cap_net_bind_service=+ep litebike-proxy

# Run as non-root
sudo -u proxy litebike-proxy
```

### Audit Logging
```bash
# Enable audit logging
AUDIT_LOG=/var/log/litebike/audit.log litebike-proxy

# Log format includes:
# - Timestamp
# - Source IP
# - Detected protocol
# - Target destination
# - Bytes transferred
```

## Bug Bounty Program

(Available for commercial deployments upon request)