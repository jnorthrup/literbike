use std::io;
use std::net::Ipv4Addr;
use log::{debug, info};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
#[cfg(feature = "auto-discovery")]
use serde::{Deserialize, Serialize};

use crate::types::StandardPort;
use crate::universal_listener::PrefixedStream;
use tokio::net::TcpStream;

#[cfg(feature = "auto-discovery")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacConfig {
    pub proxy_host: String,
    pub proxy_port: u16,
    pub socks_port: u16,
    pub direct_domains: Vec<String>,
    pub proxy_domains: Vec<String>,
    pub bypass_private: bool,
    pub bypass_local: bool,
}

#[cfg(feature = "auto-discovery")]
impl Default for PacConfig {
    fn default() -> Self {
        Self {
            proxy_host: "localhost".to_string(),
            proxy_port: StandardPort::HttpProxy as u16,
            socks_port: StandardPort::Socks5 as u16,
            direct_domains: vec![
                "localhost".to_string(),
                "127.0.0.1".to_string(),
                "*.local".to_string(),
            ],
            proxy_domains: vec![],
            bypass_private: true,
            bypass_local: true,
        }
    }
}

pub struct PacServer {
    config: PacConfig,
    local_ip: Ipv4Addr,
}

impl PacServer {
    pub fn new(local_ip: Ipv4Addr, config: PacConfig) -> Self {
        let mut pac_config = config;
        pac_config.proxy_host = local_ip.to_string();
        Self {
            config: pac_config,
            local_ip,
        }
    }

    pub async fn handle_request<S>(&mut self, stream: S, request: &str) -> io::Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        debug!("PAC request: {}", request);

        if request.starts_with("GET /proxy.pac") || request.starts_with("GET /wpad.dat") {
            self.serve_pac_file(stream).await
        } else if request.starts_with("GET /pac-config") {
            self.serve_pac_config(stream).await
        } else if request.starts_with("POST /pac-config") {
            self.update_pac_config(stream, request).await
        } else {
            self.send_error_response(stream, 404, "Not Found").await
        }
    }

    async fn serve_pac_file<S>(&self, mut stream: S) -> io::Result<()>
    where
        S: AsyncWrite + Unpin,
    {
        let pac_content = self.generate_pac_script();
        let response = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: application/x-ns-proxy-autoconfig\r\n\
             Content-Length: {}\r\n\
             Cache-Control: max-age=3600\r\n\
             \r\n\
             {}",
            pac_content.len(),
            pac_content
        );
        
        info!("Serving PAC file to client");
        stream.write_all(response.as_bytes()).await
    }

    async fn serve_pac_config<S>(&self, mut stream: S) -> io::Result<()>
    where
        S: AsyncWrite + Unpin,
    {
        let config_json = serde_json::to_string_pretty(&self.config)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        
        let response = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Access-Control-Allow-Origin: *\r\n\
             \r\n\
             {}",
            config_json.len(),
            config_json
        );
        
        stream.write_all(response.as_bytes()).await
    }

    async fn update_pac_config<S>(&mut self, mut stream: S, request: &str) -> io::Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let content_length = self.extract_content_length(request).unwrap_or(0);
        if content_length == 0 || content_length > 8192 {
            return self.send_error_response(stream, 400, "Invalid Content Length").await;
        }

        let mut body = vec![0u8; content_length];
        tokio::io::AsyncReadExt::read_exact(&mut stream, &mut body).await?;
        
        let body_str = String::from_utf8_lossy(&body);
        match serde_json::from_str::<PacConfig>(&body_str) {
            Ok(new_config) => {
                self.config = new_config;
                info!("PAC configuration updated");
                let response = "HTTP/1.1 200 OK\r\n\
                               Content-Type: application/json\r\n\
                               \r\n\
                               {\"status\":\"updated\"}";
                stream.write_all(response.as_bytes()).await
            }
            Err(e) => {
                self.send_error_response(stream, 400, &format!("Invalid JSON: {}", e)).await
            }
        }
    }

    fn generate_pac_script(&self) -> String {
        let direct_conditions = self.generate_direct_conditions();
        let proxy_conditions = self.generate_proxy_conditions();
        
        format!(
            r#"function FindProxyForURL(url, host) {{
    // Normalize host
    host = host.toLowerCase();
    
    // Direct connections for localhost and private networks
    if (host == "localhost" || 
        host == "127.0.0.1" || 
        isInNet(host, "127.0.0.0", "255.0.0.0")) {{
        return "DIRECT";
    }}
    
    // Private network bypass
    if ({bypass_private} && (
        isInNet(host, "10.0.0.0", "255.0.0.0") ||
        isInNet(host, "172.16.0.0", "255.240.0.0") ||
        isInNet(host, "192.168.0.0", "255.255.0.0"))) {{
        return "DIRECT";
    }}
    
    // Local domain bypass (.local)
    if ({bypass_local} && dnsDomainIs(host, ".local")) {{
        return "DIRECT";
    }}
    
    // Direct domain conditions
    {direct_conditions}
    
    // Proxy domain conditions  
    {proxy_conditions}
    
    // Default proxy chain with fallback
    return "PROXY {proxy_host}:{proxy_port}; SOCKS5 {proxy_host}:{socks_port}; DIRECT";
}}"#,
            bypass_private = if self.config.bypass_private { "true" } else { "false" },
            bypass_local = if self.config.bypass_local { "true" } else { "false" },
            direct_conditions = direct_conditions,
            proxy_conditions = proxy_conditions,
            proxy_host = self.config.proxy_host,
            proxy_port = self.config.proxy_port,
            socks_port = self.config.socks_port,
        )
    }

    fn generate_direct_conditions(&self) -> String {
        if self.config.direct_domains.is_empty() {
            return String::new();
        }

        let mut conditions = Vec::new();
        for domain in &self.config.direct_domains {
            if domain.starts_with("*.") {
                let suffix = &domain[2..];
                conditions.push(format!("dnsDomainIs(host, \"{}\")", suffix));
            } else if domain.contains("*") {
                conditions.push(format!("shExpMatch(host, \"{}\")", domain));
            } else {
                conditions.push(format!("host == \"{}\"", domain));
            }
        }

        format!(
            "    // Direct domains\n    if ({}) {{\n        return \"DIRECT\";\n    }}\n",
            conditions.join(" || ")
        )
    }

    fn generate_proxy_conditions(&self) -> String {
        if self.config.proxy_domains.is_empty() {
            return String::new();
        }

        let mut conditions = Vec::new();
        for domain in &self.config.proxy_domains {
            if domain.starts_with("*.") {
                let suffix = &domain[2..];
                conditions.push(format!("dnsDomainIs(host, \"{}\")", suffix));
            } else if domain.contains("*") {
                conditions.push(format!("shExpMatch(host, \"{}\")", domain));
            } else {
                conditions.push(format!("host == \"{}\"", domain));
            }
        }

        format!(
            "    // Proxy-only domains\n    if ({}) {{\n        return \"PROXY {}:{}\";\n    }}\n",
            conditions.join(" || "),
            self.config.proxy_host,
            self.config.proxy_port
        )
    }

    fn extract_content_length(&self, request: &str) -> Option<usize> {
        for line in request.lines() {
            if line.to_lowercase().starts_with("content-length:") {
                if let Some(value) = line.split(':').nth(1) {
                    return value.trim().parse().ok();
                }
            }
        }
        None
    }

    async fn send_error_response<S>(&self, mut stream: S, status: u16, message: &str) -> io::Result<()>
    where
        S: AsyncWrite + Unpin,
    {
        let response = format!(
            "HTTP/1.1 {} {}\r\n\
             Content-Type: text/plain\r\n\
             Content-Length: {}\r\n\
             \r\n\
             {}",
            status, message, message.len(), message
        );
        stream.write_all(response.as_bytes()).await
    }
}

pub async fn is_pac_request(request: &str) -> bool {
    request.contains("/proxy.pac") || 
    request.contains("/wpad.dat") ||
    request.contains("/pac-config")
}

pub fn generate_macos_proxy_script(proxy_host: &str) -> String {
    format!(
        r#"#!/bin/bash
# macOS System Proxy Configuration Script
# Generated by LiteBike Proxy

PROXY_HOST="{}"
PROXY_PORT="8080"

# Get the active network service
NETWORK_SERVICE=$(networksetup -listnetworkserviceorder | grep "(Hardware Port:" | head -1 | sed 's/.*Hardware Port: //' | sed 's/,.*//')

if [ -z "$NETWORK_SERVICE" ]; then
    echo "Error: Could not determine active network service"
    exit 1
fi

echo "Configuring proxy for network service: $NETWORK_SERVICE"

# Configure HTTP proxy
networksetup -setwebproxy "$NETWORK_SERVICE" "$PROXY_HOST" "$PROXY_PORT"
networksetup -setwebproxystate "$NETWORK_SERVICE" on

# Configure HTTPS proxy
networksetup -setsecurewebproxy "$NETWORK_SERVICE" "$PROXY_HOST" "$PROXY_PORT"
networksetup -setsecurewebproxystate "$NETWORK_SERVICE" on

# Configure SOCKS proxy
networksetup -setsocksfirewallproxy "$NETWORK_SERVICE" "$PROXY_HOST" "1080"
networksetup -setsocksfirewallproxystate "$NETWORK_SERVICE" on

# Configure PAC file
networksetup -setautoproxyurl "$NETWORK_SERVICE" "http://$PROXY_HOST:$PROXY_PORT/proxy.pac"
networksetup -setautoproxystate "$NETWORK_SERVICE" on

echo "Proxy configuration complete!"
echo "HTTP/HTTPS Proxy: $PROXY_HOST:$PROXY_PORT"
echo "SOCKS Proxy: $PROXY_HOST:1080"
echo "PAC URL: http://$PROXY_HOST:$PROXY_PORT/proxy.pac"
"#,
        proxy_host
    )
}

pub fn generate_disable_proxy_script() -> String {
    r#"#!/bin/bash
# Disable macOS System Proxy Configuration
# Generated by LiteBike Proxy

# Get the active network service
NETWORK_SERVICE=$(networksetup -listnetworkserviceorder | grep "(Hardware Port:" | head -1 | sed 's/.*Hardware Port: //' | sed 's/,.*//')

if [ -z "$NETWORK_SERVICE" ]; then
    echo "Error: Could not determine active network service"
    exit 1
fi

echo "Disabling proxy for network service: $NETWORK_SERVICE"

# Disable all proxy configurations
networksetup -setwebproxystate "$NETWORK_SERVICE" off
networksetup -setsecurewebproxystate "$NETWORK_SERVICE" off
networksetup -setsocksfirewallproxystate "$NETWORK_SERVICE" off
networksetup -setautoproxystate "$NETWORK_SERVICE" off

echo "Proxy configuration disabled!"
"#.to_string()
}

/// Handler wrapper function for PAC requests - called by universal listener
pub async fn handle_pac_request(mut stream: PrefixedStream<TcpStream>) -> std::io::Result<()> {
    use tokio::io::AsyncReadExt;
    
    let mut buffer = [0u8; 1024];
    let n = stream.read(&mut buffer).await?;
    if n == 0 { return Ok(()); }

    let request = String::from_utf8_lossy(&buffer[..n]);
    debug!("PAC request received: {}", request);

    // Extract local IP from stream or use default
    let local_ip = stream.inner.local_addr()
        .map(|addr| match addr.ip() {
            std::net::IpAddr::V4(ip) => ip,
            _ => std::net::Ipv4Addr::new(127, 0, 0, 1),
        })
        .unwrap_or_else(|_| std::net::Ipv4Addr::new(127, 0, 0, 1));

    let pac_config = PacConfig::default();
    let mut pac_server = PacServer::new(local_ip, pac_config);
    
    pac_server.handle_request(stream, &request).await
}

/// Handler wrapper function for WPAD requests - called by universal listener  
pub async fn handle_wpad_request(mut stream: PrefixedStream<TcpStream>) -> std::io::Result<()> {
    use tokio::io::AsyncReadExt;
    
    let mut buffer = [0u8; 1024];
    let n = stream.read(&mut buffer).await?;
    if n == 0 { return Ok(()); }

    let request = String::from_utf8_lossy(&buffer[..n]);
    debug!("WPAD request received: {}", request);

    // Extract local IP from stream or use default
    let local_ip = stream.inner.local_addr()
        .map(|addr| match addr.ip() {
            std::net::IpAddr::V4(ip) => ip,
            _ => std::net::Ipv4Addr::new(127, 0, 0, 1),
        })
        .unwrap_or_else(|_| std::net::Ipv4Addr::new(127, 0, 0, 1));

    let pac_config = PacConfig::default();
    let mut pac_server = PacServer::new(local_ip, pac_config);
    
    // For WPAD, we serve the same PAC content but ensure it's accessible via /wpad.dat
    let wpad_request = if request.contains("/wpad.dat") {
        request.replace("/wpad.dat", "/proxy.pac")
    } else {
        request.to_string()
    };
    
    pac_server.handle_request(stream, &wpad_request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_is_pac_request() {
        assert!(is_pac_request("GET /proxy.pac HTTP/1.1").await);
        assert!(is_pac_request("GET /wpad.dat HTTP/1.1").await);
        assert!(is_pac_request("GET /pac-config HTTP/1.1").await);
        assert!(!is_pac_request("GET / HTTP/1.1").await);
    }

    #[test]
    fn test_pac_script_generation() {
        let config = PacConfig::default();
        let server = PacServer::new(Ipv4Addr::new(192, 168, 1, 100), config);
        let pac_script = server.generate_pac_script();
        
        assert!(pac_script.contains("function FindProxyForURL"));
        assert!(pac_script.contains("192.168.1.100:8080"));
        assert!(pac_script.contains("SOCKS5"));
        assert!(pac_script.contains("DIRECT"));
    }

    #[test]
    fn test_direct_conditions_generation() {
        let mut config = PacConfig::default();
        config.direct_domains = vec![
            "example.com".to_string(),
            "*.github.com".to_string(),
            "test*.local".to_string(),
        ];
        
        let server = PacServer::new(Ipv4Addr::new(127, 0, 0, 1), config);
        let conditions = server.generate_direct_conditions();
        
        assert!(conditions.contains("host == \"example.com\""));
        assert!(conditions.contains("dnsDomainIs(host, \"github.com\")"));
        assert!(conditions.contains("shExpMatch(host, \"test*.local\")"));
    }

    #[test]
    fn test_macos_script_generation() {
        let script = generate_macos_proxy_script("192.168.1.100");
        
        assert!(script.contains("PROXY_HOST=\"192.168.1.100\""));
        assert!(script.contains("networksetup -setwebproxy"));
        assert!(script.contains("networksetup -setsocksfirewallproxy"));
        assert!(script.contains("networksetup -setautoproxyurl"));
    }
}