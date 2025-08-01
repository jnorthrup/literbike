#!/data/data/com.termux/files/usr/bin/bash
# Build LiteBike directly on Termux (no cross-compilation needed!)

set -e

echo "🔥 Building LiteBike natively on Termux 🔥"

# Install Rust if not present
if ! command -v rustc &> /dev/null; then
    echo "Installing Rust..."
    pkg install rust -y
fi

# Install required packages
pkg install git -y

# Clone or use existing source
if [ ! -d "litebike" ]; then
    echo "Cloning LiteBike source..."
    git clone https://github.com/jnorthrup/litebike.git
fi

cd litebike

# Build the Termux-optimized version
echo "Building for Termux..."
cargo build --release --bin litebike-proxy

# Install
echo "Installing..."
cp target/release/litebike-proxy $PREFIX/bin/litebike-proxy
chmod +x $PREFIX/bin/litebike-proxy

echo "✅ LiteBike installed successfully!"
echo ""
echo "Usage:"
echo "  litebike-proxy                    # Start proxy server"  
echo "  BIND_IP=0.0.0.0 litebike-proxy   # Bind to all interfaces"
echo ""
echo "Proxy endpoints:"
echo "  HTTP/HTTPS: localhost:8080"
echo "  SOCKS5: localhost:1080"