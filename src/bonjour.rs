use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::collections::HashMap;
use log::{debug, error, info, warn};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::UdpSocket;
use trust_dns_resolver::TokioAsyncResolver;
use trust_dns_resolver::proto::rr::{DNSClass, RecordType, RData};
use trust_dns_resolver::proto::op::{Message, Query};
use trust_dns_resolver::proto::serialize::binary::{BinDecodable, BinEncodable};

use crate::types::{TargetAddress, StandardPort};

const MDNS_MULTICAST_ADDR: &str = "224.0.0.251:5353";
const MDNS_TTL: u32 = 120;

#[derive(Debug, Clone)]
pub struct BonjourService {
    pub name: String,
    pub service_type: String,
    pub port: u16,
    pub txt_records: Vec<String>,
    pub ipv4_addr: Option<Ipv4Addr>,
    pub ipv6_addr: Option<std::net::Ipv6Addr>,
}

pub struct BonjourServer {
    local_ip: Ipv4Addr,
    hostname: String,
    services: HashMap<String, BonjourService>,
    resolver: TokioAsyncResolver,
}

impl BonjourServer {
    pub fn new(local_ip: Ipv4Addr, hostname: String, resolver: TokioAsyncResolver) -> Self {
        let mut server = Self {
            local_ip,
            hostname: if hostname.ends_with(".local") {
                hostname
            } else {
                format!("{}.local", hostname)
            },
            services: HashMap::new(),
            resolver,
        };
        server.add_default_services();
        server
    }

    fn add_default_services(&mut self) {
        let proxy_service = BonjourService {
            name: "LiteBike Proxy".to_string(),
            service_type: "_http._tcp.local".to_string(),
            port: StandardPort::HttpProxy as u16,
            txt_records: vec![
                "txtvers=1".to_string(),
                "proxy=true".to_string(),
                "protocols=http,https,socks5".to_string(),
                "doh=true".to_string(),
                "upnp=true".to_string(),
            ],
            ipv4_addr: Some(self.local_ip),
            ipv6_addr: None,
        };

        let socks_service = BonjourService {
            name: "LiteBike SOCKS5".to_string(),
            service_type: "_socks._tcp.local".to_string(),
            port: StandardPort::Socks5 as u16,
            txt_records: vec![
                "txtvers=1".to_string(),
                "version=5".to_string(),
                "auth=none".to_string(),
            ],
            ipv4_addr: Some(self.local_ip),
            ipv6_addr: None,
        };

        self.services.insert("_http._tcp.local".to_string(), proxy_service);
        self.services.insert("_socks._tcp.local".to_string(), socks_service);
    }

    pub async fn handle_mdns_request<S>(&self, mut stream: S, request: &str) -> io::Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        debug!("Bonjour mDNS request: {}", request);

        if request.contains(".local") {
            self.handle_local_domain_request(stream, request).await
        } else {
            self.proxy_dns_request(stream, request).await
        }
    }

    async fn handle_local_domain_request<S>(&self, mut stream: S, request: &str) -> io::Result<()>
    where
        S: AsyncWrite + Unpin,
    {
        let domain = self.extract_domain_from_request(request);
        if let Some(domain) = domain {
            debug!("Resolving local domain: {}", domain);

            if domain == self.hostname {
                self.send_hostname_response(stream).await
            } else if let Some(service) = self.find_service_by_name(&domain) {
                self.send_service_response(stream, service).await
            } else {
                self.send_nxdomain_response(stream).await
            }
        } else {
            self.send_error_response(stream, "Invalid mDNS request").await
        }
    }

    async fn proxy_dns_request<S>(&self, mut stream: S, request: &str) -> io::Result<()>
    where
        S: AsyncWrite + Unpin,
    {
        let domain = self.extract_domain_from_request(request);
        if let Some(domain) = domain {
            debug!("Proxying DNS request for: {}", domain);
            
            match self.resolver.lookup_ip(&domain).await {
                Ok(lookup) => {
                    if let Some(ip) = lookup.iter().next() {
                        self.send_ip_response(stream, &domain, ip).await
                    } else {
                        self.send_nxdomain_response(stream).await
                    }
                }
                Err(e) => {
                    error!("DNS resolution failed for {}: {}", domain, e);
                    self.send_nxdomain_response(stream).await
                }
            }
        } else {
            self.send_error_response(stream, "Invalid DNS request").await
        }
    }

    pub async fn start_mdns_advertising(&self) -> io::Result<()> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        let multicast_addr: SocketAddr = MDNS_MULTICAST_ADDR.parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("Invalid mDNS address: {}", e)))?;

        info!("Starting mDNS advertising for hostname: {}", self.hostname);

        for service in self.services.values() {
            let announcement = self.create_mdns_announcement(service);
            socket.send_to(announcement.as_bytes(), multicast_addr).await?;
            info!("Advertised Bonjour service: {} on port {}", service.name, service.port);
        }

        Ok(())
    }

    pub async fn resolve_local_domain(&self, domain: &str) -> io::Result<IpAddr> {
        if !domain.ends_with(".local") {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Not a .local domain"));
        }

        debug!("Attempting local resolution for: {}", domain);

        if domain == self.hostname {
            return Ok(IpAddr::V4(self.local_ip));
        }

        if let Some(service) = self.find_service_by_name(domain) {
            if let Some(ipv4) = service.ipv4_addr {
                return Ok(IpAddr::V4(ipv4));
            }
            if let Some(ipv6) = service.ipv6_addr {
                return Ok(IpAddr::V6(ipv6));
            }
        }

        match self.resolver.lookup_ip(domain).await {
            Ok(lookup) => {
                lookup.iter().next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, "No IP found for .local domain")
                })
            }
            Err(e) => {
                warn!("mDNS resolution failed for {}: {}", domain, e);
                let regular_domain = domain.trim_end_matches(".local");
                match self.resolver.lookup_ip(regular_domain).await {
                    Ok(lookup) => lookup.iter().next().ok_or_else(|| {
                        io::Error::new(io::ErrorKind::NotFound, "No IP found after fallback")
                    }),
                    Err(e) => Err(io::Error::new(io::ErrorKind::NotFound, e.to_string()))
                }
            }
        }
    }

    fn extract_domain_from_request(&self, request: &str) -> Option<String> {
        if request.contains("CONNECT ") {
            if let Some(host) = request.split_whitespace().nth(1) {
                return Some(host.split(':').next().unwrap_or(host).to_string());
            }
        }
        
        for line in request.lines() {
            if line.to_lowercase().starts_with("host:") {
                if let Some(host) = line.split(':').nth(1) {
                    return Some(host.trim().to_string());
                }
            }
        }
        None
    }

    fn find_service_by_name(&self, name: &str) -> Option<&BonjourService> {
        for service in self.services.values() {
            if name.contains(&service.service_type) || name.contains(&service.name) {
                return Some(service);
            }
        }
        None
    }

    fn create_mdns_announcement(&self, service: &BonjourService) -> String {
        format!(
            "NOTIFY * HTTP/1.1\r\n\
             HOST: {}\r\n\
             CACHE-CONTROL: max-age={}\r\n\
             NT: {}\r\n\
             NTS: ssdp:alive\r\n\
             SERVICE-TYPE: {}\r\n\
             SERVICE-NAME: {}\r\n\
             SERVICE-PORT: {}\r\n\
             SERVICE-TXT: {}\r\n\
             \r\n",
            MDNS_MULTICAST_ADDR,
            MDNS_TTL,
            service.service_type,
            service.service_type,
            service.name,
            service.port,
            service.txt_records.join(";")
        )
    }

    async fn send_hostname_response<S>(&self, mut stream: S) -> io::Result<()>
    where
        S: AsyncWrite + Unpin,
    {
        let response = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: application/dns-message\r\n\
             Cache-Control: max-age={}\r\n\
             \r\n\
             HOSTNAME: {}\r\n\
             IPv4: {}\r\n",
            MDNS_TTL,
            self.hostname,
            self.local_ip
        );
        stream.write_all(response.as_bytes()).await
    }

    async fn send_service_response<S>(&self, mut stream: S, service: &BonjourService) -> io::Result<()>
    where
        S: AsyncWrite + Unpin,
    {
        let response = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: application/dns-message\r\n\
             Cache-Control: max-age={}\r\n\
             \r\n\
             SERVICE: {}\r\n\
             TYPE: {}\r\n\
             PORT: {}\r\n\
             IPv4: {}\r\n\
             TXT: {}\r\n",
            MDNS_TTL,
            service.name,
            service.service_type,
            service.port,
            service.ipv4_addr.map(|ip| ip.to_string()).unwrap_or_else(|| "none".to_string()),
            service.txt_records.join(";")
        );
        stream.write_all(response.as_bytes()).await
    }

    async fn send_ip_response<S>(&self, mut stream: S, domain: &str, ip: IpAddr) -> io::Result<()>
    where
        S: AsyncWrite + Unpin,
    {
        let response = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: application/dns-message\r\n\
             Cache-Control: max-age={}\r\n\
             \r\n\
             DOMAIN: {}\r\n\
             IP: {}\r\n",
            MDNS_TTL,
            domain,
            ip
        );
        stream.write_all(response.as_bytes()).await
    }

    async fn send_nxdomain_response<S>(&self, mut stream: S) -> io::Result<()>
    where
        S: AsyncWrite + Unpin,
    {
        let response = "HTTP/1.1 404 Not Found\r\n\
                       Content-Type: text/plain\r\n\
                       \r\n\
                       NXDOMAIN\r\n";
        stream.write_all(response.as_bytes()).await
    }

    async fn send_error_response<S>(&self, mut stream: S, message: &str) -> io::Result<()>
    where
        S: AsyncWrite + Unpin,
    {
        let response = format!(
            "HTTP/1.1 400 Bad Request\r\n\
             Content-Type: text/plain\r\n\
             Content-Length: {}\r\n\
             \r\n\
             {}",
            message.len(),
            message
        );
        stream.write_all(response.as_bytes()).await
    }
}

pub async fn is_bonjour_request(request: &str) -> bool {
    request.contains(".local") ||
    request.contains("_tcp.local") ||
    request.contains("_udp.local") ||
    (request.contains("CONNECT") && request.contains(":5353")) ||
    request.contains("mDNS")
}

pub async fn bridge_mdns_query(query: &str, resolver: &TokioAsyncResolver) -> io::Result<Vec<IpAddr>> {
    debug!("Bridging mDNS query: {}", query);
    
    let domain = if query.ends_with(".local") {
        query.to_string()
    } else {
        format!("{}.local", query)
    };

    match resolver.lookup_ip(&domain).await {
        Ok(lookup) => {
            let ips: Vec<IpAddr> = lookup.iter().collect();
            if !ips.is_empty() {
                info!("Resolved {} to: {:?}", domain, ips);
                Ok(ips)
            } else {
                debug!("No IPs found for {}, trying without .local suffix", domain);
                let fallback_domain = query.trim_end_matches(".local");
                match resolver.lookup_ip(fallback_domain).await {
                    Ok(fallback_lookup) => {
                        let fallback_ips: Vec<IpAddr> = fallback_lookup.iter().collect();
                        Ok(fallback_ips)
                    }
                    Err(e) => Err(io::Error::new(io::ErrorKind::NotFound, e.to_string()))
                }
            }
        }
        Err(e) => {
            debug!("Direct mDNS lookup failed for {}: {}", domain, e);
            Err(io::Error::new(io::ErrorKind::NotFound, e.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trust_dns_resolver::config::{ResolverConfig, ResolverOpts};

    #[tokio::test]
    async fn test_is_bonjour_request() {
        assert!(is_bonjour_request("GET http://printer.local/ HTTP/1.1").await);
        assert!(is_bonjour_request("CONNECT device.local:80 HTTP/1.1").await);
        assert!(is_bonjour_request("GET /_tcp.local HTTP/1.1").await);
        assert!(!is_bonjour_request("GET http://example.com/ HTTP/1.1").await);
    }

    #[tokio::test] 
    async fn test_bonjour_server_creation() {
        let resolver = TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());
        let server = BonjourServer::new(
            Ipv4Addr::new(192, 168, 1, 100),
            "testhost".to_string(),
            resolver
        );
        
        assert_eq!(server.hostname, "testhost.local");
        assert_eq!(server.services.len(), 2);
        assert!(server.services.contains_key("_http._tcp.local"));
        assert!(server.services.contains_key("_socks._tcp.local"));
    }

    #[test]
    fn test_extract_domain_from_request() {
        let resolver = TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());
        let server = BonjourServer::new(
            Ipv4Addr::new(192, 168, 1, 100),
            "testhost".to_string(),
            resolver
        );
        
        let connect_request = "CONNECT printer.local:631 HTTP/1.1\r\n";
        assert_eq!(server.extract_domain_from_request(connect_request), Some("printer.local".to_string()));
        
        let host_request = "GET / HTTP/1.1\r\nHost: device.local\r\n";
        assert_eq!(server.extract_domain_from_request(host_request), Some("device.local".to_string()));
    }
}