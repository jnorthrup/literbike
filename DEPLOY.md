# LiteBike Production Deployment Guide

## Quick Start

### Termux/Android

```bash
# Install and run
pkg install rust git
git clone https://github.com/jnorthrup/litebike
cd litebike
cargo build --release --bin litebike-proxy
./target/release/litebike-proxy
```

### Linux Server

```bash
# Build optimized binary
RUSTFLAGS="-C target-cpu=native" cargo build --release
sudo setcap cap_net_bind_service=+ep target/release/litebike-proxy
./target/release/litebike-proxy
```

## Configuration

### Environment Variables

```bash
# Network binding
BIND_IP=0.0.0.0          # External access (default)
BIND_IP=127.0.0.1        # Local only
BIND_IP=192.168.1.100    # Specific interface

# Performance tuning
MAX_CONNECTIONS=10000    # Connection limit
WORKER_THREADS=4         # Tokio worker threads
BUFFER_SIZE=65536        # Transfer buffer size

# Currently no feature flags implemented

# Logging
RUST_LOG=info            # Log level
AUDIT_LOG=/var/log/litebike/audit.log
```

## System Tuning

### Linux Kernel Parameters

```bash
# /etc/sysctl.conf
net.ipv4.ip_forward = 1
net.ipv4.tcp_keepalive_time = 600
net.ipv4.tcp_keepalive_intvl = 60
net.ipv4.tcp_keepalive_probes = 3
net.core.somaxconn = 65535
net.ipv4.tcp_max_syn_backlog = 65535
fs.file-max = 1000000

# Apply
sudo sysctl -p
```

### File Descriptor Limits

```bash
# /etc/security/limits.conf
* soft nofile 65535
* hard nofile 65535
```

## Monitoring

### Health Check Endpoints

```bash
# TCP health check
nc -zv localhost 8080

# HTTP health check (future)
curl http://localhost:8080/health

# Metrics endpoint (future)
curl http://localhost:9090/metrics
```

### Logging

```bash
# Standard output
RUST_LOG=debug litebike-proxy 2>&1 | tee proxy.log

# Systemd
journalctl -u litebike-proxy -f

# JSON structured logs (future)
LOG_FORMAT=json litebike-proxy
```

## Service Management

### Systemd Service

```ini
# /etc/systemd/system/litebike-proxy.service
[Unit]
Description=LiteBike Universal Proxy
After=network.target

[Service]
Type=simple
User=proxy
Group=proxy
ExecStart=/usr/local/bin/litebike-proxy
Restart=on-failure
RestartSec=5s

# Security
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log/litebike

# Resource limits
LimitNOFILE=65535
LimitNPROC=4096

[Install]
WantedBy=multi-user.target
```

### Docker Compose

```yaml
version: '3.8'
services:
  litebike:
    build: .
    restart: unless-stopped
    ports:
      - "8080:8080"  # Universal port
      - "1080:1080"  # SOCKS5
    environment:
      - RUST_LOG=info
      - BIND_IP=0.0.0.0
    ulimits:
      nofile:
        soft: 65535
        hard: 65535
    deploy:
      resources:
        limits:
          cpus: '2'
          memory: 512M
        reservations:
          cpus: '0.5'
          memory: 128M
```

## High Availability

### HAProxy Load Balancer

```
global
    maxconn 100000
    
defaults
    mode tcp
    timeout connect 5s
    timeout client 30s
    timeout server 30s

listen litebike_cluster
    bind *:8080
    balance leastconn
    server proxy1 10.0.0.1:8080 check
    server proxy2 10.0.0.2:8080 check
    server proxy3 10.0.0.3:8080 check backup
```

### Nginx Stream Proxy

```nginx
stream {
    upstream litebike {
        least_conn;
        server 127.0.0.1:8081;
        server 127.0.0.1:8082;
        server 127.0.0.1:8083;
    }
    
    server {
        listen 8080;
        proxy_pass litebike;
        proxy_connect_timeout 1s;
    }
}
```

## Security Hardening

### Firewall Rules

```bash
# Allow proxy ports
sudo ufw allow 8080/tcp comment 'LiteBike Universal'
sudo ufw allow 1080/tcp comment 'LiteBike SOCKS5'

# Restrict management
sudo ufw allow from 10.0.0.0/8 to any port 9090 comment 'Metrics'
```

### AppArmor Profile

```
# /etc/apparmor.d/usr.local.bin.litebike-proxy
profile litebike-proxy /usr/local/bin/litebike-proxy {
  # Network
  network inet stream,
  network inet6 stream,
  
  # Capabilities
  capability net_bind_service,
  
  # Files
  /usr/local/bin/litebike-proxy r,
  /var/log/litebike/** rw,
  
  # Deny everything else
  deny /** w,
}
```

## Performance Testing

### Connection Testing

```bash
# Test SOCKS5
curl --socks5 localhost:1080 https://example.com

# Test HTTP
curl -x http://localhost:8080 https://example.com

# Benchmark
ab -n 10000 -c 100 -X localhost:8080 http://example.com/
```

### Load Testing

```bash
# Simple load test
for i in {1..1000}; do
    curl -x localhost:8080 https://example.com &
done

# Professional load test
vegeta attack -targets=targets.txt -rate=1000 -duration=30s | vegeta report
```

## Troubleshooting

### Common Issues

**High CPU Usage**

- Check for connection leaks
- Reduce WORKER_THREADS
- Enable rate limiting

**Memory Growth**

- Set MAX_CONNECTIONS
- Check for slow clients
- Enable connection timeouts

**Port Already in Use**

```bash
sudo lsof -i :8080
sudo kill -9 <PID>
```

**Permission Denied**

```bash
# Option 1: Capability
sudo setcap cap_net_bind_service=+ep litebike-proxy

# Option 2: Non-privileged ports
BIND_IP=0.0.0.0:8888 litebike-proxy
```

## Production Checklist

- [ ] Configure appropriate BIND_IP
- [ ] Set resource limits
- [ ] Enable logging
- [ ] Configure monitoring
- [ ] Set up service management
- [ ] Apply security hardening
- [ ] Test failover scenarios
- [ ] Document configuration
- [ ] Plan for updates
- [ ] Monitor performance metrics

## SSH co-build + run + :8888 smoke test

```bash
# Vars
LB_USER="${LB_USER:-jim}"
LB_HOST="${LB_HOST:-host.example.com}"
LB_DIR="${LB_DIR:-/opt/litebike}"
BRANCH="$(git rev-parse --abbrev-ref HEAD)"

# 1) Push current branch
git push -u origin "$BRANCH"

# 2) Build locally in parallel (optional)
cargo build --release --bin litebike-proxy &

# 3) Remote co-build and run on :8888
ssh -A -o StrictHostKeyChecking=accept-new "${LB_USER}@${LB_HOST}" '
  set -euo pipefail
  LB_DIR="${LB_DIR:-/opt/litebike}"
  BRANCH="${BRANCH:-main}"
  mkdir -p "$LB_DIR"
  if [ -d "$LB_DIR/.git" ]; then
    cd "$LB_DIR"
    git fetch --all --prune
    git checkout "$BRANCH"
    git reset --hard "origin/$BRANCH"
  else
    git clone --depth=1 https://github.com/jnorthrup/litebike "$LB_DIR"
    cd "$LB_DIR"
    git checkout "$BRANCH" || true
    git pull --ff-only || true
  fi
  RUSTFLAGS="-C target-cpu=native" cargo build --release --bin litebike-proxy
  pkill -f "litebike-proxy.*8888" 2>/dev/null || true
  nohup env BIND_IP=0.0.0.0:8888 ./target/release/litebike-proxy > litebike.log 2>&1 &
  sleep 1
  (ss -ltn 2>/dev/null || netstat -ltn) | grep ":8888" || (echo "listener not found on :8888" && exit 1)
'

# 4) Smoke test HTTP proxy via :8888
curl -I -x "http://${LB_HOST}:8888" https://example.com || echo "HTTP proxy test failed"

# 5) Quick CONNECT test (TLS through proxy)
printf 'QUIT\r\n' | openssl s_client -proxy "${LB_HOST}:8888" -connect example.com:443 -servername example.com -quiet || true

# 6) Tail remote logs (Ctrl-C to exit)
ssh "${LB_USER}@${LB_HOST}" 'tail -n 100 -F '"$LB_DIR"'/litebike.log'
```

## Termux quick SSH example (hard-coded device)

```bash
# Connect, ensure toolchain, pull/build, and run on :8888
ssh -p 8022 u0_a471@192.168.225.152 '
  set -euo pipefail
  command -v cargo >/dev/null 2>&1 || pkg install -y rust git
  if [ -d "$HOME/litebike/.git" ]; then
    cd "$HOME/litebike"
    git pull --ff-only
  else
    git clone --depth=1 https://github.com/jnorthrup/litebike "$HOME/litebike"
    cd "$HOME/litebike"
  fi
  cargo build --release --bin litebike-proxy
  pkill -f "litebike-proxy.*8888" 2>/dev/null || true
  nohup env BIND_IP=0.0.0.0:8888 ./target/release/litebike-proxy > "$HOME/litebike.log" 2>&1 &
  sleep 1
  (ss -ltn 2>/dev/null || netstat -ltn) | grep ":8888" || (echo "listener not found on :8888" && exit 1)
'

# Local smoke test against the device proxy
curl -I -x "http://192.168.225.152:8888" https://example.com || echo "HTTP proxy test failed"

# Optional local port-forward for apps expecting localhost:8888
ssh -N -L 8888:127.0.0.1:8888 -p 8022 u0_a471@192.168.225.152
```

## Termux host autodetect (macOS/Linux)

```bash
# macOS: derive TERMUX_HOST from default route
TERMUX_HOST=$(route get default 2>/dev/null | grep gateway | awk '{print $2}' || echo "192.168.1.1")

# Linux/Android/Termux: derive TERMUX_HOST from route to 8.8.8.8
TERMUX_HOST=$(ip route get 8.8.8.8 2>/dev/null | grep via | awk '{print $3}' || echo "192.168.1.1")
```

### Example device SSH

```bash
ssh -p 8022 u0_a471@192.168.225.152
```
