#!/data/data/com.termux/files/usr/bin/bash
# TERMUX Knox Client Automation
# Expert automation for client-side Knox bypass and proxy setup

set -euo pipefail

# TERMUX-specific paths
TERMUX_HOME="$HOME"
PROJECT_DIR="$TERMUX_HOME/litebike"
CARGO_TARGET_DIR="$PROJECT_DIR/target"

# Configuration
PROXY_PORT="${PROXY_PORT:-8080}"
SOCKS_PORT="${SOCKS_PORT:-1080}"
HOST_IP="${HOST_IP:-192.168.1.10}"  # Mac host IP

log() { echo -e "\033[0;32m[$(date +'%H:%M:%S')]\033[0m $*"; }
warn() { echo -e "\033[1;33m[WARN]\033[0m $*"; }
error() { echo -e "\033[0;31m[ERROR]\033[0m $*"; }

# Setup TERMUX environment for Knox bypass
setup_knox_environment() {
    log "Setting up TERMUX Knox bypass environment"
    
    # Install required packages
    pkg update -y
    pkg install -y rust clang make cmake git openssh curl wget
    
    # Setup Android-specific Rust configuration
    rustup target add aarch64-linux-android || warn "Target already installed"
    
    # Create optimized cargo config for TERMUX
    mkdir -p "$HOME/.cargo"
    cat > "$HOME/.cargo/config.toml" << 'EOF'
[build]
target = "aarch64-linux-android"
jobs = 2  # Limit jobs for mobile device

[target.aarch64-linux-android]
linker = "aarch64-linux-android-clang"

[env]
CC_aarch64_linux_android = "aarch64-linux-android-clang"
CXX_aarch64_linux_android = "aarch64-linux-android-clang++"
AR_aarch64_linux_android = "aarch64-linux-android-ar"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"

[profile.dev]
opt-level = 1
debug = false  # Save space on mobile
EOF
    
    # Set TERMUX-specific environment variables
    export ANDROID_NDK_HOME="$PREFIX"
    export TERMUX_PKG_CACHEDIR="$HOME/.cache/termux"
    export RUSTFLAGS="-C target-cpu=native -C opt-level=3 -C link-arg=-s"
    
    log "TERMUX Knox environment setup completed"
}

# Build Knox bypass binary optimized for TERMUX
build_knox_binary() {
    log "Building Knox bypass binary for TERMUX"
    
    cd "$PROJECT_DIR"
    
    # Clean previous builds to save space
    cargo clean
    
    # Build with Knox-specific features
    cargo build --release \
        --features knox-bypass,termux-compat,posix-sockets \
        --bin litebike \
        --target aarch64-linux-android
    
    # Strip binary to reduce size
    aarch64-linux-android-strip "$CARGO_TARGET_DIR/aarch64-linux-android/release/litebike" || warn "Strip failed"
    
    # Copy to convenient location
    cp "$CARGO_TARGET_DIR/aarch64-linux-android/release/litebike" "$PROJECT_DIR/litebike-knox"
    chmod +x "$PROJECT_DIR/litebike-knox"
    
    log "Knox binary built successfully: $(du -h $PROJECT_DIR/litebike-knox | cut -f1)"
}

# Start Knox bypass proxy
start_knox_proxy() {
    log "Starting Knox bypass proxy"
    
    # Kill existing processes
    pkill -f litebike-knox || true
    sleep 2
    
    # Clear any existing proxy settings
    unset http_proxy https_proxy all_proxy HTTP_PROXY HTTPS_PROXY ALL_PROXY 2>/dev/null || true
    
    # Start Knox bypass proxy with TERMUX optimizations and fingerprinting
    cd "$PROJECT_DIR"
    ./litebike-knox knox-proxy \
        --bind "0.0.0.0:$PROXY_PORT" \
        --socks-port "$SOCKS_PORT" \
        --enable-knox-bypass \
        --enable-tethering-bypass \
        --tcp-fingerprint \
        --tls-fingerprint \
        --packet-fragmentation \
        --ttl-spoofing 64 \
        --max-connections 50 \
        > "$HOME/knox-proxy.log" 2>&1 &
    
    local proxy_pid=$!
    echo "$proxy_pid" > "$HOME/knox-proxy.pid"
    
    # Wait for proxy to start
    sleep 5
    
    # Test Knox proxy
    if curl -x "http://localhost:$PROXY_PORT" -s --connect-timeout 10 "http://httpbin.org/ip" > /dev/null; then
        log "✅ Knox proxy started successfully (PID: $proxy_pid)"
        return 0
    else
        error "❌ Knox proxy failed to start"
        tail -20 "$HOME/knox-proxy.log"
        return 1
    fi
}

# Setup automated git sync with host
setup_git_sync() {
    log "Setting up automated git sync"
    
    # Generate SSH key if not exists
    if [[ ! -f "$HOME/.ssh/id_rsa" ]]; then
        ssh-keygen -t rsa -b 2048 -f "$HOME/.ssh/id_rsa" -N ""
        log "SSH key generated. Add this to your host's authorized_keys:"
        cat "$HOME/.ssh/id_rsa.pub"
    fi
    
    # Create git sync script
    cat > "$PROJECT_DIR/sync_with_host.sh" << EOF
#!/data/data/com.termux/files/usr/bin/bash
# Auto-sync with host git repository

set -euo pipefail

cd "$PROJECT_DIR"

# Pull latest changes from host
if git remote get-url origin &>/dev/null; then
    git pull origin master || {
        echo "Pull failed, attempting to resolve conflicts"
        git stash
        git pull origin master
        git stash pop || echo "No stash to pop"
    }
else
    echo "No git remote configured"
fi

# Rebuild if source changed
if [[ -n "\$(git diff HEAD~1 --name-only | grep -E '\.(rs|toml)$')" ]]; then
    echo "Source files changed, rebuilding..."
    ./$(basename "$0") build
fi
EOF
    
    chmod +x "$PROJECT_DIR/sync_with_host.sh"
    log "Git sync script created"
}

# Test Knox bypass effectiveness
test_knox_bypass() {
    log "Testing Knox bypass effectiveness"
    
    # Test direct connection (should fail on Knox devices)
    log "Testing direct connection..."
    if timeout 10 curl -s "http://httpbin.org/ip" > /tmp/direct_test.json 2>/dev/null; then
        warn "Direct connection works (not on Knox device or bypass already active)"
    else
        log "Direct connection blocked (expected on Knox device)"
    fi
    
    # Test Knox proxy
    log "Testing Knox proxy..."
    if curl -x "http://localhost:$PROXY_PORT" -s --connect-timeout 10 "http://httpbin.org/ip" > /tmp/proxy_test.json; then
        log "✅ Knox proxy working"
        
        # Compare IPs
        if [[ -f /tmp/direct_test.json ]] && [[ -f /tmp/proxy_test.json ]]; then
            local direct_ip=$(jq -r '.origin' /tmp/direct_test.json 2>/dev/null || echo "unknown")
            local proxy_ip=$(jq -r '.origin' /tmp/proxy_test.json 2>/dev/null || echo "unknown")
            
            if [[ "$direct_ip" != "$proxy_ip" ]]; then
                log "✅ IP address changed through proxy: $direct_ip -> $proxy_ip"
            else
                warn "IP address unchanged - proxy may not be working"
            fi
        fi
    else
        error "❌ Knox proxy not working"
        return 1
    fi
    
    # Test SOCKS proxy
    log "Testing SOCKS proxy..."
    if curl --socks5 "localhost:$SOCKS_PORT" -s --connect-timeout 10 "http://httpbin.org/ip" > /dev/null; then
        log "✅ SOCKS proxy working"
    else
        warn "SOCKS proxy not working"
    fi
    
    # Test tethering detection bypass
    log "Testing tethering detection bypass..."
    local user_agent=$(curl -x "http://localhost:$PROXY_PORT" -s "http://httpbin.org/user-agent" | jq -r '.user-agent' 2>/dev/null || echo "unknown")
    if [[ "$user_agent" =~ (iPhone|Android|Mobile) ]]; then
        log "✅ Mobile User-Agent detected: $user_agent"
    else
        warn "Desktop User-Agent detected: $user_agent"
    fi
    
    log "Knox bypass testing completed"
}

# Monitor Knox proxy status
monitor_proxy() {
    log "Monitoring Knox proxy status (Ctrl+C to stop)"
    
    while true; do
        if [[ -f "$HOME/knox-proxy.pid" ]]; then
            local pid=$(cat "$HOME/knox-proxy.pid")
            if kill -0 "$pid" 2>/dev/null; then
                local connections=$(netstat -an | grep ":$PROXY_PORT " | wc -l)
                echo "$(date +'%H:%M:%S') - Knox proxy running (PID: $pid, Connections: $connections)"
            else
                error "Knox proxy process died (PID: $pid)"
                break
            fi
        else
            error "Knox proxy PID file not found"
            break
        fi
        sleep 10
    done
}

# Stop Knox proxy
stop_proxy() {
    log "Stopping Knox proxy"
    
    if [[ -f "$HOME/knox-proxy.pid" ]]; then
        local pid=$(cat "$HOME/knox-proxy.pid")
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid"
            rm -f "$HOME/knox-proxy.pid"
            log "Knox proxy stopped"
        else
            warn "Knox proxy process not running"
        fi
    fi
    
    # Cleanup any remaining processes
    pkill -f litebike-knox || true
}

# Main function
main() {
    local command="${1:-help}"
    
    case "$command" in
        "setup")
            setup_knox_environment
            ;;
        "build")
            build_knox_binary
            ;;
        "start")
            start_knox_proxy
            ;;
        "test")
            test_knox_bypass
            ;;
        "sync")
            setup_git_sync
            ;;
        "monitor")
            monitor_proxy
            ;;
        "stop")
            stop_proxy
            ;;
        "full")
            setup_knox_environment
            build_knox_binary
            start_knox_proxy
            test_knox_bypass
            setup_git_sync
            log "🎉 Knox bypass setup completed successfully"
            ;;
        "help"|*)
            echo "TERMUX Knox Client Automation"
            echo "Usage: $0 {setup|build|start|test|sync|monitor|stop|full}"
            echo ""
            echo "Commands:"
            echo "  setup   - Setup TERMUX environment for Knox bypass"
            echo "  build   - Build Knox bypass binary"
            echo "  start   - Start Knox proxy"
            echo "  test    - Test Knox bypass effectiveness"
            echo "  sync    - Setup git sync with host"
            echo "  monitor - Monitor proxy status"
            echo "  stop    - Stop Knox proxy"
            echo "  full    - Run complete setup (recommended)"
            ;;
    esac
}

# Trap to cleanup on exit
trap 'stop_proxy 2>/dev/null || true' EXIT

# Run main function
main "$@"