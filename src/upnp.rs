use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::collections::HashMap;
use log::{debug, info, warn};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::UdpSocket;
#[cfg(feature = "upnp")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "upnp")]
use chrono;

use crate::types::{UpnpAction, StandardPort};
use crate::universal_listener::PrefixedStream;
use tokio::net::TcpStream;

const UPNP_MULTICAST_ADDR: &str = "239.255.255.250:1900";
const SSDP_ALIVE: &str = "ssdp:alive";
const SSDP_BYEBYE: &str = "ssdp:byebye";
const SSDP_DISCOVER: &str = "ssdp:discover";

#[cfg(feature = "upnp")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpnpDevice {
    pub uuid: String,
    pub device_type: String,
    pub friendly_name: String,
    pub manufacturer: String,
    pub model_name: String,
    pub location: String,
    pub services: Vec<UpnpService>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpnpService {
    pub service_type: String,
    pub service_id: String,
    pub control_url: String,
    pub event_sub_url: String,
    pub scpd_url: String,
}

pub struct UpnpServer {
    local_ip: Ipv4Addr,
    devices: HashMap<String, UpnpDevice>,
    port_mappings: HashMap<u16, PortMapping>,
}

#[derive(Debug, Clone)]
pub struct PortMapping {
    pub external_port: u16,
    pub internal_ip: Ipv4Addr,
    pub internal_port: u16,
    pub protocol: String,
    pub description: String,
    pub lease_duration: u32,
}

impl UpnpServer {
    pub fn new(local_ip: Ipv4Addr) -> Self {
        let mut server = Self {
            local_ip,
            devices: HashMap::new(),
            port_mappings: HashMap::new(),
        };
        server.add_default_device();
        server
    }

    fn add_default_device(&mut self) {
        let wan_service = UpnpService {
            service_type: "urn:schemas-upnp-org:service:WANIPConnection:1".to_string(),
            service_id: "urn:upnp-org:serviceId:WANIPConn1".to_string(),
            control_url: "/upnp/control/WANIPConn1".to_string(),
            event_sub_url: "/upnp/event/WANIPConn1".to_string(),
            scpd_url: "/upnp/scpd/WANIPConn1.xml".to_string(),
        };

        let device = UpnpDevice {
            uuid: format!("uuid:litebike-proxy-{}", self.local_ip),
            device_type: "urn:schemas-upnp-org:device:InternetGatewayDevice:1".to_string(),
            friendly_name: "LiteBike Proxy Gateway".to_string(),
            manufacturer: "LiteBike".to_string(),
            model_name: "Proxy Gateway".to_string(),
            location: format!("http://{}:1900/upnp/device.xml", self.local_ip),
            services: vec![wan_service],
        };

        self.devices.insert(device.uuid.clone(), device);
    }

    pub async fn handle_ssdp_request<S>(&mut self, stream: S, request: &str) -> io::Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        debug!("UPnP SSDP request: {}", request);

        let action = self.parse_ssdp_action(request);
        match action {
            UpnpAction::Search => self.handle_search_request(stream, request).await,
            UpnpAction::Subscribe => self.handle_subscribe_request(stream, request).await,
            UpnpAction::Unsubscribe => self.handle_unsubscribe_request(stream, request).await,
            UpnpAction::AddPortMapping => self.handle_port_mapping_request(stream, request).await,
            UpnpAction::DeletePortMapping => self.handle_delete_port_mapping_request(stream, request).await,
            _ => self.send_error_response(stream, 400, "Unsupported UPnP action").await,
        }
    }

    async fn handle_search_request<S>(&self, mut stream: S, request: &str) -> io::Result<()>
    where
        S: AsyncWrite + Unpin,
    {
        let st_header = self.extract_header(request, "ST");
        let search_target = st_header.unwrap_or("upnp:rootdevice");

        info!("UPnP search for: {}", search_target);

        for device in self.devices.values() {
            if self.matches_search_target(&device.device_type, search_target) {
                let response = format!(
                    "HTTP/1.1 200 OK\r\n\
                     CACHE-CONTROL: max-age=1800\r\n\
                     DATE: {}\r\n\
                     EXT:\r\n\
                     LOCATION: {}\r\n\
                     OPT: \"http://schemas.upnp.org/upnp/1/0/\"; ns=01\r\n\
                     01-NLS: 1\r\n\
                     SERVER: LiteBike/1.0 UPnP/1.0 LiteBike-Proxy/1.0\r\n\
                     ST: {}\r\n\
                     USN: {}::{}\r\n\
                     \r\n",
                    {
                        #[cfg(feature = "upnp")]
                        {
                            chrono::Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string()
                        }
                        #[cfg(not(feature = "upnp"))]
                        {
                            "Thu, 01 Jan 1970 00:00:00 GMT".to_string()
                        }
                    },
                    device.location,
                    search_target,
                    device.uuid,
                    device.device_type
                );
                stream.write_all(response.as_bytes()).await?;
                break;
            }
        }
        Ok(())
    }

    async fn handle_subscribe_request<S>(&self, mut stream: S, _request: &str) -> io::Result<()>
    where
        S: AsyncWrite + Unpin,
    {
        debug!("UPnP subscription request");
        let response = "HTTP/1.1 200 OK\r\n\
                       SID: uuid:subscription-1\r\n\
                       TIMEOUT: Second-1800\r\n\
                       \r\n";
        stream.write_all(response.as_bytes()).await
    }

    async fn handle_unsubscribe_request<S>(&self, mut stream: S, _request: &str) -> io::Result<()>
    where
        S: AsyncWrite + Unpin,
    {
        debug!("UPnP unsubscription request");
        let response = "HTTP/1.1 200 OK\r\n\r\n";
        stream.write_all(response.as_bytes()).await
    }

    async fn handle_port_mapping_request<S>(&mut self, mut stream: S, request: &str) -> io::Result<()>
    where
        S: AsyncWrite + Unpin,
    {
        debug!("UPnP port mapping request");
        
        if let Some(soap_body) = self.extract_soap_body(request) {
            if let Some(mapping) = self.parse_port_mapping(&soap_body) {
                self.port_mappings.insert(mapping.external_port, mapping.clone());
                info!("Added port mapping: {}:{} -> {}:{}", 
                     mapping.external_port, mapping.protocol,
                     mapping.internal_ip, mapping.internal_port);
                
                let response = "HTTP/1.1 200 OK\r\n\
                               Content-Type: text/xml; charset=utf-8\r\n\
                               \r\n\
                               <?xml version=\"1.0\"?>\r\n\
                               <s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\r\n\
                               <s:Body>\r\n\
                               <u:AddPortMappingResponse xmlns:u=\"urn:schemas-upnp-org:service:WANIPConnection:1\">\r\n\
                               </u:AddPortMappingResponse>\r\n\
                               </s:Body>\r\n\
                               </s:Envelope>\r\n";
                stream.write_all(response.as_bytes()).await?;
            } else {
                self.send_error_response(stream, 400, "Invalid port mapping request").await?;
            }
        } else {
            self.send_error_response(stream, 400, "No SOAP body found").await?;
        }
        Ok(())
    }

    async fn handle_delete_port_mapping_request<S>(&mut self, mut stream: S, request: &str) -> io::Result<()>
    where
        S: AsyncWrite + Unpin,
    {
        debug!("UPnP delete port mapping request");
        
        if let Some(soap_body) = self.extract_soap_body(request) {
            if let Some(external_port) = self.parse_delete_port_mapping(&soap_body) {
                self.port_mappings.remove(&external_port);
                info!("Deleted port mapping for port {}", external_port);
                
                let response = "HTTP/1.1 200 OK\r\n\
                               Content-Type: text/xml; charset=utf-8\r\n\
                               \r\n\
                               <?xml version=\"1.0\"?>\r\n\
                               <s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\r\n\
                               <s:Body>\r\n\
                               <u:DeletePortMappingResponse xmlns:u=\"urn:schemas-upnp-org:service:WANIPConnection:1\">\r\n\
                               </u:DeletePortMappingResponse>\r\n\
                               </s:Body>\r\n\
                               </s:Envelope>\r\n";
                stream.write_all(response.as_bytes()).await?;
            } else {
                self.send_error_response(stream, 400, "Invalid delete port mapping request").await?;
            }
        } else {
            self.send_error_response(stream, 400, "No SOAP body found").await?;
        }
        Ok(())
    }

    pub async fn start_ssdp_advertising(&self) -> io::Result<()> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        let multicast_addr: SocketAddr = UPNP_MULTICAST_ADDR.parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("Invalid UPnP address: {}", e)))?;

        for device in self.devices.values() {
            let notify_alive = format!(
                "NOTIFY * HTTP/1.1\r\n\
                 HOST: {}\r\n\
                 CACHE-CONTROL: max-age=1800\r\n\
                 LOCATION: {}\r\n\
                 NT: {}\r\n\
                 NTS: {}\r\n\
                 USN: {}::{}\r\n\
                 SERVER: LiteBike/1.0 UPnP/1.0 LiteBike-Proxy/1.0\r\n\
                 \r\n",
                UPNP_MULTICAST_ADDR,
                device.location,
                device.device_type,
                SSDP_ALIVE,
                device.uuid,
                device.device_type
            );

            socket.send_to(notify_alive.as_bytes(), multicast_addr).await?;
            info!("Sent UPnP NOTIFY alive for device: {}", device.friendly_name);
        }

        Ok(())
    }

    fn parse_ssdp_action(&self, request: &str) -> UpnpAction {
        if request.contains("M-SEARCH") {
            UpnpAction::Search
        } else if request.contains("SUBSCRIBE") {
            UpnpAction::Subscribe  
        } else if request.contains("UNSUBSCRIBE") {
            UpnpAction::Unsubscribe
        } else if request.contains("AddPortMapping") {
            UpnpAction::AddPortMapping
        } else if request.contains("DeletePortMapping") {
            UpnpAction::DeletePortMapping
        } else {
            UpnpAction::Notify
        }
    }

    fn extract_header<'a>(&self, request: &'a str, header_name: &str) -> Option<&'a str> {
        for line in request.lines() {
            if line.to_uppercase().starts_with(&format!("{}:", header_name.to_uppercase())) {
                return line.split(':').nth(1).map(|s| s.trim());
            }
        }
        None
    }

    fn extract_soap_body(&self, request: &str) -> Option<String> {
        if let Some(body_start) = request.find("\r\n\r\n") {
            Some(request[body_start + 4..].to_string())
        } else {
            None
        }
    }

    fn parse_port_mapping(&self, soap_body: &str) -> Option<PortMapping> {
        let external_port = self.extract_xml_value(soap_body, "NewExternalPort")?;
        let internal_ip = self.extract_xml_value(soap_body, "NewInternalClient")?;
        let internal_port = self.extract_xml_value(soap_body, "NewInternalPort")?;
        let protocol = self.extract_xml_value(soap_body, "NewProtocol")?;
        let description = self.extract_xml_value(soap_body, "NewPortMappingDescription").unwrap_or("LiteBike Mapping");
        let lease_duration = self.extract_xml_value(soap_body, "NewLeaseDuration").unwrap_or("0");

        Some(PortMapping {
            external_port: external_port.parse().ok()?,
            internal_ip: internal_ip.parse().ok()?,
            internal_port: internal_port.parse().ok()?,
            protocol: protocol.to_string(),
            description: description.to_string(),
            lease_duration: lease_duration.parse().unwrap_or(0),
        })
    }

    fn parse_delete_port_mapping(&self, soap_body: &str) -> Option<u16> {
        let external_port = self.extract_xml_value(soap_body, "NewExternalPort")?;
        external_port.parse().ok()
    }

    fn extract_xml_value<'a>(&self, xml: &'a str, tag: &str) -> Option<&'a str> {
        let start_tag = format!("<{}>", tag);
        let end_tag = format!("</{}>", tag);
        
        if let Some(start) = xml.find(&start_tag) {
            let content_start = start + start_tag.len();
            if let Some(end) = xml[content_start..].find(&end_tag) {
                return Some(&xml[content_start..content_start + end]);
            }
        }
        None
    }

    fn matches_search_target(&self, device_type: &str, search_target: &str) -> bool {
        search_target == "upnp:rootdevice" || 
        search_target == "urn:schemas-upnp-org:device:InternetGatewayDevice:1" ||
        search_target == device_type
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

pub async fn is_upnp_request(request: &str) -> bool {
    request.contains("M-SEARCH") || 
    request.contains("NOTIFY") ||
    request.contains("SUBSCRIBE") ||
    request.contains("UNSUBSCRIBE") ||
    (request.contains("POST") && (
        request.contains("AddPortMapping") || 
        request.contains("DeletePortMapping") ||
        request.contains("GetExternalIPAddress")
    ))
}

#[cfg(feature = "upnp")]
pub async fn setup_upnp_gateway(local_ip: Ipv4Addr) -> io::Result<()> {
    info!("Setting up UPnP gateway with IP {}", local_ip);
    
    let gateway = match tokio::task::spawn_blocking(move || {
        igd_next::search_gateway(Default::default())
    }).await {
        Ok(Ok(gw)) => gw,
        Ok(Err(e)) => {
            warn!("UPnP: Could not find gateway: {}. Will act as local gateway.", e);
            return Ok(());
        }
        Err(e) => {
            warn!("UPnP: Task failed: {}", e);
            return Ok(());
        }
    };
    
    info!("UPnP Gateway found: {}", gateway.addr);
    
    let http_addr = SocketAddr::new(IpAddr::V4(local_ip), StandardPort::HttpProxy as u16);
    if let Err(e) = gateway.add_port(
        igd_next::PortMappingProtocol::TCP, 
        StandardPort::HttpProxy as u16, 
        http_addr, 
        3600, 
        "LiteBike-HTTP"
    ) {
        warn!("UPnP: Failed to map HTTP port: {}", e);
    } else {
        info!("UPnP: Successfully mapped HTTP port {}", StandardPort::HttpProxy as u16);
    }

    let socks_addr = SocketAddr::new(IpAddr::V4(local_ip), StandardPort::Socks5 as u16);
    if let Err(e) = gateway.add_port(
        igd_next::PortMappingProtocol::TCP, 
        StandardPort::Socks5 as u16, 
        socks_addr, 
        3600, 
        "LiteBike-SOCKS5"
    ) {
        warn!("UPnP: Failed to map SOCKS5 port: {}", e);
    } else {
        info!("UPnP: Successfully mapped SOCKS5 port {}", StandardPort::Socks5 as u16);
    }

    Ok(())
}

#[cfg(not(feature = "upnp"))]
pub async fn setup_upnp_gateway(_local_ip: Ipv4Addr) -> io::Result<()> {
    debug!("UPnP support disabled - compile with 'upnp' feature to enable");
    Ok(())
}

/// Handler wrapper function for UPnP requests - called by universal listener
pub async fn handle_upnp_request(mut stream: PrefixedStream<TcpStream>) -> std::io::Result<()> {
    use tokio::io::AsyncReadExt;
    
    let mut buffer = [0u8; 1024];
    let n = stream.read(&mut buffer).await?;
    if n == 0 { return Ok(()); }

    let request = String::from_utf8_lossy(&buffer[..n]);
    debug!("UPnP request received: {}", request);

    // Extract local IP from stream or use default
    let local_ip = stream.inner.local_addr()
        .map(|addr| match addr.ip() {
            std::net::IpAddr::V4(ip) => ip,
            _ => std::net::Ipv4Addr::new(127, 0, 0, 1),
        })
        .unwrap_or_else(|_| std::net::Ipv4Addr::new(127, 0, 0, 1));

    let mut upnp_server = UpnpServer::new(local_ip);
    upnp_server.handle_ssdp_request(stream, &request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_is_upnp_request() {
        let search_request = "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\n";
        assert!(is_upnp_request(search_request).await);

        let port_mapping_request = "POST /upnp/control/WANIPConn1 HTTP/1.1\r\nSOAPAction: AddPortMapping\r\n";
        assert!(is_upnp_request(port_mapping_request).await);

        let regular_request = "GET / HTTP/1.1\r\n";
        assert!(!is_upnp_request(regular_request).await);
    }

    #[test]
    fn test_extract_xml_value() {
        let server = UpnpServer::new(Ipv4Addr::new(192, 168, 1, 100));
        let xml = "<NewExternalPort>8080</NewExternalPort><NewInternalPort>8080</NewInternalPort>";
        
        assert_eq!(server.extract_xml_value(xml, "NewExternalPort"), Some("8080"));
        assert_eq!(server.extract_xml_value(xml, "NewInternalPort"), Some("8080"));
        assert_eq!(server.extract_xml_value(xml, "NonExistent"), None);
    }
}