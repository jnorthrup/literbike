#!/data/data/com.termux/files/usr/bin/bash
# LiteBike Termux Installation Script

echo "🔥 Installing LiteBike Proxy for Termux 🔥"

# Copy binary to bin directory
cp litebike-proxy $PREFIX/bin/litebike-proxy
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