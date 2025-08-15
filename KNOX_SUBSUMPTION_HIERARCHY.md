# Knox Subsumption Hierarchy - Complete System Architecture

```mermaid
graph TD
    %% Main Entry Point
    LITEBIKE[litebike binary]
    
    %% Core Commands
    LITEBIKE --> KNOX[knox-proxy]
    LITEBIKE --> SSH_DEPLOY[ssh-deploy]
    LITEBIKE --> PROXY_CONFIG[proxy-config]
    LITEBIKE --> PROXY_QUICK[proxy-quick]
    LITEBIKE --> CARRIER_BYPASS[carrier-bypass]
    LITEBIKE --> GIT_REMOTE[git-remote-sync]
    
    %% Knox Proxy Subsystems
    KNOX --> KNOX_CONFIG{KnoxProxyConfig}
    KNOX_CONFIG --> TCP_FP[TCP Fingerprint Manager]
    KNOX_CONFIG --> TLS_FP[TLS Fingerprint Manager]
    KNOX_CONFIG --> PACKET_FRAG[Packet Fragmenter]
    KNOX_CONFIG --> TETHER_BYPASS[Tethering Bypass]
    KNOX_CONFIG --> POSIX_SOCK[POSIX Sockets]
    
    %% TCP Fingerprinting Details
    TCP_FP --> MOBILE_PROFILES[Mobile Profiles]
    MOBILE_PROFILES --> IPHONE14[iPhone 14<br/>TTL: 64<br/>Window: 65535<br/>MSS: 1460<br/>Keepalive: 7200s]
    MOBILE_PROFILES --> IPHONE15[iPhone 15<br/>TTL: 64<br/>Window: 131072<br/>MSS: 1460<br/>Scale: 7]
    MOBILE_PROFILES --> SAMSUNG[Samsung S24<br/>TTL: 64<br/>Window: 87380<br/>MSS: 1440<br/>NodeDelay: true]
    MOBILE_PROFILES --> PIXEL[Pixel Pro 7<br/>TTL: 64<br/>Window: 65536<br/>MSS: 1460<br/>Scale: 6]
    MOBILE_PROFILES --> ONEPLUS[OnePlus 11<br/>TTL: 64<br/>Window: 87380<br/>MSS: 1460<br/>NodeDelay: true]
    
    %% TLS Fingerprinting Details
    TLS_FP --> BROWSER_PROFILES[Browser Profiles]
    BROWSER_PROFILES --> SAFARI[Safari 17<br/>TLS 1.3<br/>h2/http1.1<br/>No Early Data<br/>Compress Cert]
    BROWSER_PROFILES --> CHROME[Chrome 120<br/>TLS 1.3<br/>Early Data<br/>Compress Cert<br/>x25519,secp256r1]
    BROWSER_PROFILES --> FIREFOX[Firefox 121<br/>TLS 1.3<br/>No Compress<br/>No Early Data<br/>Conservative]
    BROWSER_PROFILES --> SAMSUNG_BROWSER[Samsung 21<br/>TLS 1.3<br/>Basic Extensions<br/>h2/http1.1]
    BROWSER_PROFILES --> EDGE[Edge 120<br/>TLS 1.3<br/>Early Data<br/>Compress Cert<br/>Full Features]
    
    %% Packet Fragmentation Details
    PACKET_FRAG --> FRAG_PATTERNS[Fragment Patterns]
    FRAG_PATTERNS --> CONSERVATIVE[Conservative<br/>512-1460 bytes<br/>1-5ms delay<br/>No randomization]
    FRAG_PATTERNS --> AGGRESSIVE[Aggressive<br/>8-256 bytes<br/>5-100ms delay<br/>Randomized order<br/>Duplicates + Overlaps]
    FRAG_PATTERNS --> ADAPTIVE[Adaptive<br/>Dynamic sizing<br/>Time-based variation<br/>Detection response]
    FRAG_PATTERNS --> CARRIER_SPECIFIC[Carrier Specific]
    
    CARRIER_SPECIFIC --> VERIZON[Verizon<br/>MTU: 1428<br/>64-1200 bytes<br/>5-25ms delay<br/>No randomization]
    CARRIER_SPECIFIC --> ATT[AT&T<br/>MTU: 1500<br/>32-1460 bytes<br/>10-40ms delay<br/>Randomized order]
    CARRIER_SPECIFIC --> TMOBILE[T-Mobile<br/>MTU: 1500<br/>128-1400 bytes<br/>2-15ms delay<br/>Duplicates enabled]
    CARRIER_SPECIFIC --> SPRINT[Sprint<br/>MTU: 1472<br/>96-1300 bytes<br/>8-30ms delay<br/>Overlaps enabled]
    
    %% Tethering Bypass Details
    TETHER_BYPASS --> TTL_SPOOF[TTL Spoofing]
    TETHER_BYPASS --> DNS_OVERRIDE[DNS Override]
    TETHER_BYPASS --> UA_ROTATION[User-Agent Rotation]
    TETHER_BYPASS --> TRAFFIC_SHAPE[Traffic Shaping]
    
    TTL_SPOOF --> TTL_IMPL[Implementation]
    TTL_IMPL --> LINUX_TTL[Linux: iptables<br/>-j TTL --ttl-set 64<br/>-j HL --hl-set 64]
    TTL_IMPL --> MACOS_TTL[macOS: pfctl<br/>scrub set-tos<br/>random-id]
    TTL_IMPL --> ANDROID_TTL[Android: iptables<br/>requires root<br/>su -c iptables]
    
    DNS_OVERRIDE --> DNS_SERVERS[DNS Servers<br/>8.8.8.8 (Google)<br/>1.1.1.1 (Cloudflare)<br/>9.9.9.9 (Quad9)]
    DNS_OVERRIDE --> DNS_IMPL[Implementation]
    DNS_IMPL --> LINUX_DNS[Linux: /etc/resolv.conf<br/>backup + override]
    DNS_IMPL --> MACOS_DNS[macOS: networksetup<br/>-setdnsservers Wi-Fi]
    
    UA_ROTATION --> UA_LIST[Mobile User Agents]
    UA_LIST --> UA_IPHONE[iPhone iOS 17<br/>Safari 17.0<br/>WebKit 605.1.15]
    UA_LIST --> UA_ANDROID[Android 14<br/>Chrome 120<br/>Samsung Galaxy S24]
    UA_LIST --> UA_PIXEL[Android 13<br/>Chrome 119<br/>Google Pixel 7]
    
    TRAFFIC_SHAPE --> TIMING[Packet Timing]
    TIMING --> DELAY_RANGE[Delay Range<br/>Min: 10ms<br/>Max: 50ms<br/>Burst: 3 packets]
    TIMING --> LINUX_TC[Linux: tc qdisc<br/>netem delay 10ms 5ms]
    
    %% SSH Deployment Flow
    SSH_DEPLOY --> SSH_FLOW{SSH Workflow}
    SSH_FLOW --> TEST_CONN[1. Test SSH Connection<br/>ssh -o ConnectTimeout=5<br/>-o StrictHostKeyChecking=no]
    SSH_FLOW --> SYNC_BINARY[2. Sync Binary<br/>rsync -avz --progress<br/>./target/release/litebike]
    SSH_FLOW --> SETUP_ENV[3. Setup TERMUX Environment<br/>ANDROID_NDK_HOME=$PREFIX<br/>chmod +x litebike-knox]
    SSH_FLOW --> START_PROXY[4. Start Knox Proxy<br/>nohup ./litebike-knox<br/>knox-proxy &]
    SSH_FLOW --> AUTO_SYNC[5. Setup Auto-Sync<br/>git sync mechanism]
    
    %% Git Remote Management
    GIT_REMOTE --> GIT_FLOW{Git Remote Flow}
    GIT_FLOW --> CHECK_EXISTING[1. Check Existing<br/>git remote -v<br/>List current remotes]
    GIT_FLOW --> PRUNE_OLD[2. Prune Old<br/>git remote prune origin<br/>Clean stale refs]
    GIT_FLOW --> CREATE_TMP[3. Create Temp Remote<br/>git remote add tmp<br/>ssh://user@host:port/path]
    GIT_FLOW --> PUSH_CHANGES[4. Push Changes<br/>git push tmp master<br/>Sync to TERMUX]
    GIT_FLOW --> BUILD_RELEASE[5. Build Release<br/>cargo build --release<br/>--features knox-bypass]
    GIT_FLOW --> RUN_DEFAULTS[6. Run Defaults<br/>Start automation]
    
    %% Proxy Configuration Hierarchy
    PROXY_CONFIG --> PROXY_HIERARCHY{Proxy Configuration}
    PROXY_HIERARCHY --> GIT_PROXY[Git Proxy<br/>git config --global<br/>http.proxy<br/>https.proxy]
    PROXY_HIERARCHY --> NPM_PROXY[NPM Proxy<br/>npm config set<br/>proxy + https-proxy]
    PROXY_HIERARCHY --> SYSTEM_PROXY[System Proxy<br/>networksetup (macOS)<br/>HTTP/HTTPS/SOCKS]
    PROXY_HIERARCHY --> SSH_PROXY[SSH Proxy<br/>~/.ssh/config<br/>ProxyCommand nc -x]
    PROXY_HIERARCHY --> ENV_PROXY[Environment<br/>http_proxy<br/>https_proxy<br/>all_proxy]
    
    %% Quick Proxy (Port 8888)
    PROXY_QUICK --> QUICK_8888{Port 8888 Setup}
    QUICK_8888 --> AUTO_DETECT[Auto-detect Target<br/>localhost vs remote]
    QUICK_8888 --> MACOS_SYSTEM[macOS System Proxy<br/>networksetup Wi-Fi<br/>127.0.0.1:8888]
    QUICK_8888 --> ENV_VARS[Environment Variables<br/>export http_proxy=<br/>export https_proxy=<br/>export all_proxy=]
    QUICK_8888 --> TEST_CONN_8888[Test Connection<br/>curl -x http://host:8888<br/>http://httpbin.org/ip]
    
    %% POSIX Socket Operations
    POSIX_SOCK --> POSIX_OPS{POSIX Operations}
    POSIX_OPS --> RECV_PEEK[recv() with MSG_PEEK<br/>Direct syscall<br/>Bypass /proc filesystem]
    POSIX_OPS --> SOCKET_INFO[Socket Information<br/>getsockopt() calls<br/>SO_RCVBUF, SO_SNDBUF<br/>Socket type detection]
    POSIX_OPS --> BYPASS_PROC[/proc Bypass<br/>Knox restriction<br/>Direct file descriptor<br/>No /proc/net access]
    POSIX_OPS --> WRAPPED_STREAM[PosixTcpStream<br/>Wrapper around TcpStream<br/>Peek buffer management<br/>Replay capability]
    
    %% Protocol Detection Engine
    KNOX --> PROTOCOL_DETECT{Protocol Detection}
    PROTOCOL_DETECT --> POSIX_DETECT[POSIX Detection<br/>detect_protocol_posix()<br/>Uses MSG_PEEK]
    PROTOCOL_DETECT --> ASYNC_DETECT[Async Detection<br/>detect_protocol()<br/>Standard async read]
    
    POSIX_DETECT --> HTTP_DETECT[HTTP Detection<br/>GET/POST/PUT/DELETE<br/>HEAD/OPTIONS/CONNECT<br/>PATCH methods]
    POSIX_DETECT --> SOCKS5_DETECT[SOCKS5 Detection<br/>First byte: 0x05<br/>Version check]
    POSIX_DETECT --> WEBSOCKET_DETECT[WebSocket Detection<br/>Upgrade: websocket<br/>Connection: Upgrade]
    POSIX_DETECT --> TLS_DETECT[TLS Detection<br/>0x16 (Handshake)<br/>Version bytes]
    
    %% Connection Handling Pipeline
    KNOX --> CONN_PIPELINE{Connection Pipeline}
    CONN_PIPELINE --> ACCEPT_CONN[1. Accept Connection<br/>TcpListener::accept()]
    CONN_PIPELINE --> FINGERPRINT[2. Apply Fingerprints<br/>TCP + TLS parameters]
    CONN_PIPELINE --> DETECT_PROTO[3. Detect Protocol<br/>POSIX peek or async]
    CONN_PIPELINE --> FRAGMENT[4. Fragment Packets<br/>DPI evasion]
    CONN_PIPELINE --> ROUTE[5. Route Traffic<br/>HTTP vs SOCKS5]
    CONN_PIPELINE --> FORWARD[6. Forward/Proxy<br/>Bidirectional copy]
    
    %% TERMUX Integration Details
    SSH_DEPLOY --> TERMUX{TERMUX Integration}
    TERMUX --> TERMUX_ENV[Environment Setup<br/>ANDROID_NDK_HOME=$PREFIX<br/>TERMUX_PKG_CACHEDIR<br/>RUSTFLAGS optimization]
    TERMUX --> TERMUX_BUILD[Build Configuration<br/>target: aarch64-linux-android<br/>linker: clang<br/>strip binary]
    TERMUX --> TERMUX_FEATURES[TERMUX Features<br/>--features knox-bypass<br/>--features termux-compat<br/>--features posix-sockets]
    TERMUX --> TERMUX_RUNTIME[Runtime Setup<br/>./litebike-knox<br/>nohup background<br/>PID file management]
    
    %% Default Automation Parameters
    RUN_DEFAULTS --> DEFAULT_CONFIG{Default Configuration}
    DEFAULT_CONFIG --> DEFAULT_BIND[Bind Configuration<br/>HTTP: 0.0.0.0:8080<br/>SOCKS: 0.0.0.0:1080<br/>Max connections: 100]
    DEFAULT_CONFIG --> DEFAULT_BYPASS[Bypass Features<br/>--enable-knox-bypass<br/>--enable-tethering-bypass<br/>--tcp-fingerprint<br/>--tls-fingerprint<br/>--packet-fragmentation]
    DEFAULT_CONFIG --> DEFAULT_TTL[TTL Configuration<br/>--ttl-spoofing 64<br/>Mobile device mimicry]
    DEFAULT_CONFIG --> DEFAULT_BUFFER[Buffer Settings<br/>--buffer-size 4096<br/>TERMUX optimized]
    
    %% Carrier Detection & Countermeasures
    CARRIER_BYPASS --> DETECT_METHODS{Detection Methods}
    DETECT_METHODS --> TTL_DETECT[TTL Detection<br/>Different TTL responses<br/>Mobile vs Desktop]
    DETECT_METHODS --> DPI_DETECT[Deep Packet Inspection<br/>Protocol fingerprinting<br/>Content analysis]
    DETECT_METHODS --> UA_DETECT[User-Agent Filtering<br/>Desktop vs Mobile<br/>Browser fingerprinting]
    DETECT_METHODS --> DNS_DETECT[DNS Filtering<br/>Query blocking<br/>Response manipulation]
    DETECT_METHODS --> PORT_DETECT[Port Blocking<br/>Common port restrictions<br/>Connection timeouts]
    DETECT_METHODS --> BANDWIDTH_DETECT[Bandwidth Throttling<br/>Speed limitations<br/>QoS restrictions]
    
    %% Knox Security Bypass Mechanisms
    KNOX --> KNOX_BYPASS{Knox Security Bypass}
    KNOX_BYPASS --> AVOID_PROC[Avoid /proc Filesystem<br/>Knox blocks /proc access<br/>Use direct syscalls]
    KNOX_BYPASS --> DIRECT_SYSCALL[Direct System Calls<br/>recv(), getsockopt()<br/>Bypass Android policies]
    KNOX_BYPASS --> POSIX_ONLY[POSIX Operations Only<br/>No Android-specific APIs<br/>Standard POSIX calls]
    KNOX_BYPASS --> NO_STANDARD_LIB[Bypass Standard Library<br/>Direct libc calls<br/>Avoid Android interceptors]
    KNOX_BYPASS --> FD_DIRECT[Direct File Descriptors<br/>AsRawFd trait<br/>Raw socket operations]
    
    %% Testing & Verification Suite
    SSH_DEPLOY --> TEST_SUITE{Testing & Verification}
    TEST_SUITE --> TEST_PROXY[Proxy Testing<br/>curl -x http://host:8080<br/>HTTP connectivity]
    TEST_SUITE --> TEST_SOCKS[SOCKS Testing<br/>curl --socks5 host:1080<br/>SOCKS5 connectivity]
    TEST_SUITE --> TEST_BYPASS[Bypass Testing<br/>httpbin.org/ip<br/>IP address comparison]
    TEST_SUITE --> TEST_FINGERPRINT[Fingerprint Testing<br/>JA3 fingerprint<br/>TCP characteristics]
    TEST_SUITE --> MONITOR_LOG[Log Monitoring<br/>tail -f knox-proxy.log<br/>Connection tracking]
    TEST_SUITE --> BANDWIDTH_TEST[Bandwidth Testing<br/>Speed comparison<br/>Throttling detection]
    
    %% Connection Protocol Handlers
    ROUTE --> HTTP_HANDLER[HTTP Handler<br/>CONNECT method<br/>Tunnel establishment<br/>Bidirectional copy]
    ROUTE --> SOCKS5_HANDLER[SOCKS5 Handler<br/>Authentication<br/>Connection request<br/>Target connection]
    
    HTTP_HANDLER --> HTTP_CONNECT[HTTP CONNECT<br/>Target parsing<br/>Success response<br/>200 Connection established]
    HTTP_HANDLER --> HTTP_REGULAR[HTTP Regular<br/>URL parsing<br/>Host header<br/>Request forwarding]
    
    SOCKS5_HANDLER --> SOCKS5_AUTH[SOCKS5 Auth<br/>No auth: 0x00<br/>Username/pass: 0x02]
    SOCKS5_HANDLER --> SOCKS5_CONN[SOCKS5 Connect<br/>IPv4: 0x01<br/>Domain: 0x03<br/>IPv6: 0x04]
    
    %% Advanced Features
    KNOX --> ADVANCED{Advanced Features}
    ADVANCED --> JA3_EVASION[JA3 Evasion<br/>TLS fingerprint<br/>Cipher randomization<br/>Extension ordering]
    ADVANCED --> TIMING_ATTACKS[Timing Attack Defense<br/>Connection delays<br/>Response timing<br/>Jitter injection]
    ADVANCED --> CONN_MULTIPLEX[Connection Multiplexing<br/>Multiple targets<br/>Load balancing<br/>Failover]
    ADVANCED --> ADAPTIVE_BYPASS[Adaptive Bypass<br/>Detection response<br/>Strategy switching<br/>Learning algorithms]
    
    %% Documentation References
    LITEBIKE --> DOCS{Documentation}
    DOCS --> README[README.md<br/>Usage instructions<br/>Default configurations]
    DOCS --> DEPLOY[DEPLOY.md<br/>SSH deployment<br/>TERMUX setup]
    DOCS --> SECURITY[SECURITY.md<br/>Knox bypass details<br/>Carrier evasion]
    DOCS --> API_DOCS[API Documentation<br/>Rust docs<br/>Module descriptions]
    
    %% Style definitions for visual clarity
    classDef command fill:#e1f5fe,stroke:#01579b,stroke-width:3px
    classDef config fill:#f3e5f5,stroke:#4a148c,stroke-width:2px
    classDef feature fill:#e8f5e9,stroke:#1b5e20,stroke-width:2px
    classDef detail fill:#fff3e0,stroke:#e65100,stroke-width:1px
    classDef flow fill:#fce4ec,stroke:#880e4f,stroke-width:2px
    classDef security fill:#ffebee,stroke:#b71c1c,stroke-width:3px
    classDef network fill:#e0f2f1,stroke:#004d40,stroke-width:2px
    
    class LITEBIKE,KNOX,SSH_DEPLOY,PROXY_CONFIG,PROXY_QUICK,CARRIER_BYPASS,GIT_REMOTE command
    class KNOX_CONFIG,SSH_FLOW,GIT_FLOW,PROXY_HIERARCHY,QUICK_8888,DEFAULT_CONFIG config
    class TCP_FP,TLS_FP,PACKET_FRAG,TETHER_BYPASS,POSIX_SOCK,PROTOCOL_DETECT feature
    class MOBILE_PROFILES,BROWSER_PROFILES,FRAG_PATTERNS,TTL_VALUES,DNS_SERVERS,UA_LIST detail
    class TEST_CONN,SYNC_BINARY,SETUP_ENV,START_PROXY,AUTO_SYNC,CONN_PIPELINE flow
    class KNOX_BYPASS,AVOID_PROC,DIRECT_SYSCALL,POSIX_ONLY,NO_STANDARD_LIB security
    class HTTP_HANDLER,SOCKS5_HANDLER,DETECT_METHODS,ADVANCED network
```

## Subsumption Hierarchy Explanation

### Primary Subsumption Levels

1. **litebike Binary** - Main entry point that subsumes all functionality
2. **Core Commands** - knox-proxy, ssh-deploy, proxy-config, etc.
3. **Subsystem Managers** - TCP/TLS fingerprinting, packet fragmentation, etc.
4. **Implementation Details** - Mobile profiles, carrier specifics, protocol handlers

### Key Subsumption Relationships

- **knox-proxy** subsumes all bypass technologies (TCP fingerprinting, TLS obfuscation, packet fragmentation, tethering bypass)
- **ssh-deploy** subsumes git remote management, TERMUX setup, and automation
- **proxy-config** subsumes all system proxy configurations (Git, NPM, SSH, system-wide)
- **Protocol detection** subsumes multiple detection methods (POSIX vs async, various protocols)

### Knox Security Bypass Hierarchy

The Knox bypass operates through multiple layers of subsumption:
1. Avoid Android/Knox-specific APIs entirely
2. Use only POSIX-compliant operations
3. Direct system calls bypass Knox interceptors
4. /proc filesystem avoidance prevents detection

### Carrier Bypass Hierarchy

Carrier detection countermeasures are hierarchically organized:
1. Network-level (TTL spoofing, packet fragmentation)
2. Protocol-level (TLS fingerprinting, User-Agent rotation)
3. Traffic-level (timing, shaping, DNS override)
4. Application-level (mobile device mimicry)

### Default Automation Flow

The system follows this subsumption hierarchy for automation:
1. SSH connection establishment
2. Git remote synchronization
3. Binary building and deployment
4. Knox proxy startup with all bypass features
5. System proxy configuration
6. Connectivity testing and verification

This hierarchy ensures that higher-level commands automatically handle all lower-level details, providing seamless automation while maintaining full control over individual components when needed.