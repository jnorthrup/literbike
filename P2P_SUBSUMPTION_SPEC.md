# P2P Litebike Node - Complete Subsumption Specification

## Single Executable Deliverable

**Binary:** `litebike` (self-contained, self-replicating agent)
**Size:** ~10MB (all dependencies statically linked)
**Features:** All Knox bypass, network tools, git sync, SSH deploy

## Subsumption Hierarchy (Complete Symmetry)

### Level 0: Argv[0] Dispatch (SysV Tool Emulation)
```
ifconfig -> litebike (argv[0] = "ifconfig")
route    -> litebike (argv[0] = "route")  
netstat  -> litebike (argv[0] = "netstat")
ip       -> litebike (argv[0] = "ip")
watch    -> litebike (argv[0] = "watch")
```

### Level 1: Core Network Operations
```
litebike ifconfig     # Interface management
litebike route        # Routing table
litebike netstat      # Network connections
litebike ip           # IP address management
litebike watch        # Network monitoring
```

### Level 2: Advanced Network Features  
```
litebike probe        # Network discovery
litebike domains      # DNS operations
litebike carrier      # Carrier detection
litebike radios       # Radio interface scan
litebike scan-ports   # Port scanning
```

### Level 3: Proxy & Knox Operations
```
litebike proxy-quick 127.0.0.1 8888    # Instant proxy on 8888
litebike knox-proxy --bind 127.0.0.1:8888 --enable-tethering-bypass
litebike proxy-setup enable localhost 8888
litebike proxy-config --host 127.0.0.1 --http-port 8888
litebike carrier-bypass                 # Enable carrier bypass
```

### Level 4: Git & SSH Subsumption
```
litebike git-sync     # Git repository sync
litebike git-push     # Git operations
litebike ssh-deploy   # SSH deployment to TERMUX
litebike remote-sync  # Remote synchronization
```

### Level 5: Self-Replication Agent
```
litebike bootstrap             # Self-rebuild from cache
litebike bootstrap peer_host   # P2P replication
```

## P2P Node Symmetry Requirements

### Cargo Cache Subsumption
- **Local:** `~/.cargo/registry` (1.2GB)
- **Remote:** Identical cache via P2P transfer
- **Lock:** `Cargo.lock` ensures version symmetry
- **Build:** `cargo build --offline` (no network dependency)

### Git Repository Symmetry
- **Local:** Working git repository
- **Remote:** Temporary git remote for sync
- **Sync:** Bidirectional via SSH + git
- **State:** Matching commit hashes on both nodes

### Environment Variable Symmetry
```bash
# Both nodes need matching:
TERMUX_HOST=192.168.1.100
TERMUX_PORT=8022
TERMUX_USER=u0_a471
```

### Network Configuration Symmetry
- **HTTP Proxy:** 8888 (standardized)
- **SOCKS Proxy:** 1080 (standardized)  
- **Knox Bypass:** Enabled on both
- **Tethering:** Bypass carrier restrictions

## Single Executable Features (All Included)

### Network Utilities (Built-in)
- Interface management (ifconfig equivalent)
- Routing (route equivalent)
- Connection monitoring (netstat equivalent)
- IP configuration (ip equivalent)
- Port scanning
- Network discovery

### Knox Proxy Features (Built-in)
- HTTP/HTTPS proxy server
- SOCKS5 proxy server
- TCP fingerprinting bypass
- TLS fingerprinting bypass
- Packet fragmentation
- Tethering bypass
- Carrier detection bypass
- UPnP aggressive mode

### Git & SSH Features (Built-in)
- Git repository sync
- SSH deployment automation
- Remote binary sync via rsync
- Temporary git remote management
- Bidirectional file sync

### Self-Replication Features (Built-in)
- Self-bootstrap from cargo cache
- P2P cache transfer
- Version comparison
- Self-replacement (optional)
- Offline build capability

## P2P Deployment Process

### Initial Setup (Node A)
```bash
# Build the deliverable
cargo build --release --features knox-bypass,termux-compat

# Create hardlinks for SysV compatibility  
ln target/release/litebike ifconfig
ln target/release/litebike route
ln target/release/litebike netstat
ln target/release/litebike ip

# Test self-bootstrap
./target/release/litebike bootstrap
```

### P2P Transfer to Node B
```bash
# Pack cargo cache + binary + source
tar czf litebike-p2p.tar.gz \
    target/release/litebike \
    ~/.cargo/registry \
    Cargo.lock \
    src/

# Transfer to peer node
scp -P 8022 litebike-p2p.tar.gz peer@192.168.1.100:~/

# On Node B: unpack and bootstrap
tar xzf litebike-p2p.tar.gz
chmod +x litebike
./litebike bootstrap
```

### Symmetric Operation
```bash
# Both nodes can now:
./litebike knox-proxy --bind 127.0.0.1:8888 --enable-tethering-bypass
./litebike ssh-deploy --auto-sync
./litebike proxy-quick 127.0.0.1 8888
```

## Technical Debt Halving

### Before P2P Subsumption
- Node A: Full internet dependency for cargo downloads
- Node B: Full internet dependency for cargo downloads  
- Total: 2x network bandwidth cost
- Risk: Proxy outages block both nodes

### After P2P Subsumption  
- Node A: One-time cargo cache build
- Node B: P2P transfer from Node A (local network)
- Total: 1x network bandwidth cost
- Risk: Zero dependency on external proxies

### Cache Efficiency
- **Shared:** 1.2GB cargo cache used by both nodes
- **Transfer:** Local network speed (gigabit)
- **Rebuild:** Offline builds on both nodes
- **Symmetry:** Identical binaries guaranteed by Cargo.lock

## Executable Self-Awareness

The litebike binary knows:
1. **What it is:** Network utility + proxy + git tool
2. **Where it is:** Current executable path
3. **What it can become:** Self-update via bootstrap
4. **Who it can talk to:** SSH peers, git remotes
5. **How to replicate:** P2P cache transfer + rebuild

## Result: True P2P Node

Single `litebike` executable that:
- Subsumes all network tools (ifconfig, route, netstat, ip)
- Runs Knox proxy with full bypass features
- Syncs code via git over SSH
- Replicates itself to peer nodes
- Operates offline after initial setup
- Maintains perfect symmetry between nodes

**Zero external dependencies after P2P bootstrap.**