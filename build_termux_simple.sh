#!/bin/bash
# Termux ARM64 Simple Build Script
# Build minimal LiteBike proxy for Termux on Android ARM64

set -e

echo "🔥 Building LiteBike for Termux ARM64 (Simplified) 🔥"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Environment setup
export ANDROID_NDK_ROOT="/opt/homebrew/Caskroom/android-ndk/28b/AndroidNDK13356709.app/Contents/NDK"
export TARGET="aarch64-linux-android"
export RUSTFLAGS="-C target-feature=+crt-static"
export CC_aarch64_linux_android="$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android29-clang"
export AR_aarch64_linux_android="$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-ar"

# Clear problematic environment variables
unset CFLAGS

echo -e "${YELLOW}Environment:${NC}"
echo "  Target: $TARGET"
echo "  Using simplified Cargo.toml for Termux"

# Build for Termux using simplified configuration
echo -e "${YELLOW}Building simplified version for Termux...${NC}"
cargo build --target $TARGET --release --manifest-path ./Cargo-termux.toml --bin litebike-proxy

# Check if build succeeded
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Build successful!${NC}"
    
    # Show binary info
    BINARY="target/$TARGET/release/litebike-proxy"
    if [ -f "$BINARY" ]; then
        echo -e "${YELLOW}Binary info:${NC}"
        ls -lh "$BINARY"
        file "$BINARY"
        
        # Create Termux package directory
        mkdir -p termux-package
        cp "$BINARY" termux-package/litebike-proxy-termux
        
        # Create installation script
        cat > termux-package/install.sh << 'EOF'
#!/data/data/com.termux/files/usr/bin/bash
# LiteBike Termux Installation Script

echo "🔥 Installing LiteBike Proxy for Termux 🔥"

# Copy binary to bin directory
cp litebike-proxy-termux $PREFIX/bin/litebike-proxy
chmod +x $PREFIX/bin/litebike-proxy

echo "✅ Installation complete!"
echo ""
echo "Usage:"
echo "  litebike-proxy                    # Start proxy server"
echo "  BIND_IP=0.0.0.0 litebike-proxy   # Bind to all interfaces"
echo ""
echo "Proxy endpoints:"
echo "  HTTP/HTTPS: localhost:8080"
echo "  SOCKS5: localhost:1080"
echo ""
echo "Configure your browser or apps to use these proxy settings."
EOF
        
        chmod +x termux-package/install.sh
        
        # Create README for Termux
        cat > termux-package/README-termux.md << 'EOF'
# LiteBike Proxy for Termux

A lightweight HTTP/HTTPS and SOCKS5 proxy server optimized for Termux.

## Installation

1. Copy `litebike-proxy-termux` to your Termux device
2. Run: `bash install.sh`

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
- HTTP/HTTPS Proxy: `<termux-ip>:8080`  
- SOCKS5 Proxy: `<termux-ip>:1080`

To find your Termux device IP:
```bash
ip route get 1 | awk '{print $7}'
```

## Features

- HTTP/HTTPS proxy with CONNECT tunneling
- SOCKS5 proxy support
- Lightweight and fast
- No external dependencies
- Perfect for mobile data sharing
EOF
        
        echo -e "${GREEN}✅ Termux package created in termux-package/${NC}"
        echo -e "${YELLOW}Package contents:${NC}"
        ls -la termux-package/
        
        echo -e "${GREEN}📱 Ready for Termux deployment! 📱${NC}"
        echo "Transfer the termux-package/ directory to your Android device and run install.sh"
        
    else
        echo -e "${RED}Error: Binary not found at $BINARY${NC}"
        exit 1
    fi
else
    echo -e "${RED}❌ Build failed${NC}"
    exit 1
fi

echo -e "${GREEN}🎉 Termux build complete! 🎉${NC}"