#!/bin/bash
# Termux ARM64 Build Script
# Build LiteBike proxy for Termux on Android ARM64

set -e

echo "🔥 Building LiteBike for Termux ARM64 🔥"

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
export CXX_aarch64_linux_android="$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android29-clang++"
export AR_aarch64_linux_android="$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-ar"

# Clear macOS CFLAGS for Android build
unset CFLAGS

echo -e "${YELLOW}Environment:${NC}"
echo "  Target: $TARGET"
echo "  NDK: $ANDROID_NDK_ROOT"
echo "  Rustflags: $RUSTFLAGS"

# Check if linker exists
LINKER="$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/darwin-x86_64/bin/aarch64-linux-android29-clang"
if [ ! -f "$LINKER" ]; then
    echo -e "${RED}Error: Android NDK linker not found at $LINKER${NC}"
    exit 1
fi

echo -e "${GREEN}✅ NDK linker found${NC}"

# Build for Termux
echo -e "${YELLOW}Building for Termux...${NC}"
cargo build --target $TARGET --release --bin litebike

# Check if build succeeded
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Build successful!${NC}"
    
    # Show binary info
    BINARY="target/$TARGET/release/litebike"
    if [ -f "$BINARY" ]; then
        echo -e "${YELLOW}Binary info:${NC}"
        ls -lh "$BINARY"
        file "$BINARY"
        
        # Create Termux package directory
        mkdir -p termux-package
        cp "$BINARY" termux-package/litebike
        cp README.md termux-package/
        cp scripts/proxy-bridge termux-package/
        
        echo -e "${GREEN}✅ Termux package created in termux-package/${NC}"
        echo -e "${YELLOW}Installation on Termux:${NC}"
        echo "  1. Copy litebike to \$PREFIX/bin/"
        echo "  2. Copy proxy-bridge to \$PREFIX/bin/"
        echo "  3. chmod +x \$PREFIX/bin/litebike-proxy"
        echo "  4. chmod +x \$PREFIX/bin/proxy-bridge"
        echo ""
        echo -e "${GREEN}Usage in Termux:${NC}"
        echo "  litebike                    # Start proxy server"
        echo "  BIND_IP=0.0.0.0 litebike   # Bind to all interfaces"
        echo "  proxy-bridge                     # Full bridge setup"
        
    else
        echo -e "${RED}Error: Binary not found at $BINARY${NC}"
        exit 1
    fi
else
    echo -e "${RED}❌ Build failed${NC}"
    exit 1
fi

echo -e "${GREEN}🎉 Termux build complete! 🎉${NC}"