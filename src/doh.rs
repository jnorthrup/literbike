use std::io;
use log::{debug, error, info};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use trust_dns_resolver::TokioAsyncResolver;
use trust_dns_resolver::proto::rr::{DNSClass, RecordType};
use trust_dns_resolver::proto::op::{Message, Query};
use trust_dns_resolver::proto::serialize::binary::{BinDecodable, BinEncodable};

use crate::types::{ProtocolType, TargetAddress};

const DOH_CONTENT_TYPE: &str = "application/dns-message";
const DOH_PATH: &str = "/dns-query";

pub struct DohServer {
    resolver: TokioAsyncResolver,
}

impl DohServer {
    pub fn new(resolver: TokioAsyncResolver) -> Self {
        Self { resolver }
    }

    pub async fn handle_request<S>(&self, mut stream: S, request: &str) -> io::Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        debug!("DoH request: {}", request);

        if request.starts_with("GET") && request.contains(DOH_PATH) {
            self.handle_get_request(stream, request).await
        } else if request.starts_with("POST") && request.contains(DOH_PATH) {
            self.handle_post_request(stream, request).await
        } else {
            self.send_error_response(stream, 400, "Bad Request").await
        }
    }

    async fn handle_get_request<S>(&self, mut stream: S, request: &str) -> io::Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let dns_param = extract_dns_parameter(request);
        if let Some(dns_data) = dns_param.and_then(|p| base64_decode(p)) {
            self.process_dns_query(stream, &dns_data).await
        } else {
            self.send_error_response(stream, 400, "Invalid DNS parameter").await
        }
    }

    async fn handle_post_request<S>(&self, mut stream: S, request: &str) -> io::Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        if !request.contains(&format!("Content-Type: {}", DOH_CONTENT_TYPE)) {
            return self.send_error_response(stream, 415, "Unsupported Media Type").await;
        }

        let content_length = extract_content_length(request).unwrap_or(0);
        if content_length == 0 || content_length > 4096 {
            return self.send_error_response(stream, 400, "Invalid Content Length").await;
        }

        let mut body = vec![0u8; content_length];
        tokio::io::AsyncReadExt::read_exact(&mut stream, &mut body).await?;

        self.process_dns_query(stream, &body).await
    }

    async fn process_dns_query<S>(&self, mut stream: S, dns_data: &[u8]) -> io::Result<()>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let query_message = match Message::from_bytes(dns_data) {
            Ok(msg) => msg,
            Err(e) => {
                error!("Failed to parse DNS message: {}", e);
                return self.send_error_response(stream, 400, "Invalid DNS message").await;
            }
        };

        let response = self.resolve_dns_query(query_message).await?;
        let response_bytes = match response.to_bytes() {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to serialize DNS response: {}", e);
                return self.send_error_response(stream, 500, "Internal Server Error").await;
            }
        };

        let http_response = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: {}\r\n\
             Content-Length: {}\r\n\
             Cache-Control: max-age=300\r\n\
             \r\n",
            DOH_CONTENT_TYPE,
            response_bytes.len()
        );

        stream.write_all(http_response.as_bytes()).await?;
        stream.write_all(&response_bytes).await?;
        Ok(())
    }

    async fn resolve_dns_query(&self, query: Message) -> io::Result<Message> {
        let mut response = Message::new();
        response.set_id(query.id());
        response.set_message_type(trust_dns_resolver::proto::op::MessageType::Response);
        response.set_op_code(query.op_code());
        response.set_authoritative(false);
        response.set_recursion_desired(query.recursion_desired());
        response.set_recursion_available(true);

        for query in query.queries() {
            response.add_query(query.clone());
            
            let name = query.name();
            let record_type = query.query_type();
            
            debug!("Resolving {} {} query", name, record_type);

            match record_type {
                RecordType::A => {
                    if let Ok(lookup) = self.resolver.ipv4_lookup(name.clone()).await {
                        for ip in lookup.iter() {
                            let record = trust_dns_resolver::proto::rr::Record::from_rdata(
                                name.clone(),
                                300,
                                trust_dns_resolver::proto::rr::RData::A(*ip)
                            );
                            response.add_answer(record);
                        }
                    }
                }
                RecordType::AAAA => {
                    if let Ok(lookup) = self.resolver.ipv6_lookup(name.clone()).await {
                        for ip in lookup.iter() {
                            let record = trust_dns_resolver::proto::rr::Record::from_rdata(
                                name.clone(),
                                300,
                                trust_dns_resolver::proto::rr::RData::AAAA(*ip)
                            );
                            response.add_answer(record);
                        }
                    }
                }
                RecordType::MX => {
                    if let Ok(lookup) = self.resolver.mx_lookup(name.clone()).await {
                        for mx in lookup.iter() {
                            let record = trust_dns_resolver::proto::rr::Record::from_rdata(
                                name.clone(),
                                300,
                                trust_dns_resolver::proto::rr::RData::MX(mx.clone())
                            );
                            response.add_answer(record);
                        }
                    }
                }
                RecordType::TXT => {
                    if let Ok(lookup) = self.resolver.txt_lookup(name.clone()).await {
                        for txt in lookup.iter() {
                            let record = trust_dns_resolver::proto::rr::Record::from_rdata(
                                name.clone(),
                                300,
                                trust_dns_resolver::proto::rr::RData::TXT(txt.clone())
                            );
                            response.add_answer(record);
                        }
                    }
                }
                _ => {
                    info!("Unsupported query type: {}", record_type);
                }
            }
        }

        Ok(response)
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

pub async fn is_doh_request(request: &str) -> bool {
    (request.starts_with("GET") || request.starts_with("POST")) 
        && request.contains(DOH_PATH)
        && (request.contains("dns=") || request.contains(DOH_CONTENT_TYPE))
}

pub async fn resolve_local_domain(domain: &str, resolver: &TokioAsyncResolver) -> io::Result<std::net::IpAddr> {
    if domain.ends_with(".local") {
        debug!("Attempting mDNS resolution for {}", domain);
        match resolver.lookup_ip(domain).await {
            Ok(lookup) => {
                if let Some(ip) = lookup.iter().next() {
                    Ok(ip)
                } else {
                    Err(io::Error::new(io::ErrorKind::NotFound, "No IP found for .local domain"))
                }
            }
            Err(_) => {
                debug!("mDNS resolution failed for {}, trying regular DNS", domain);
                let regular_domain = domain.trim_end_matches(".local");
                match resolver.lookup_ip(regular_domain).await {
                    Ok(lookup) => lookup.iter().next().ok_or_else(|| {
                        io::Error::new(io::ErrorKind::NotFound, "No IP found")
                    }),
                    Err(e) => Err(io::Error::new(io::ErrorKind::NotFound, e.to_string()))
                }
            }
        }
    } else {
        match resolver.lookup_ip(domain).await {
            Ok(lookup) => lookup.iter().next().ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "No IP found")
            }),
            Err(e) => Err(io::Error::new(io::ErrorKind::NotFound, e.to_string()))
        }
    }
}

fn extract_dns_parameter(request: &str) -> Option<&str> {
    for line in request.lines() {
        if line.starts_with("GET") && line.contains("dns=") {
            if let Some(start) = line.find("dns=") {
                let param_start = start + 4;
                let param_end = line[param_start..].find(&[' ', '&', '#'][..]).unwrap_or(line.len() - param_start);
                return Some(&line[param_start..param_start + param_end]);
            }
        }
    }
    None
}

fn extract_content_length(request: &str) -> Option<usize> {
    for line in request.lines() {
        if line.to_lowercase().starts_with("content-length:") {
            if let Some(value) = line.split(':').nth(1) {
                return value.trim().parse().ok();
            }
        }
    }
    None
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(input).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_dns_parameter() {
        let request = "GET /dns-query?dns=AAABAAABAAAAAAAAA3d3dwdleGFtcGxlA2NvbQAAAQAB HTTP/1.1\r\n";
        let param = extract_dns_parameter(request);
        assert_eq!(param, Some("AAABAAABAAAAAAAAA3d3dwdleGFtcGxlA2NvbQAAAQAB"));
    }

    #[test]
    fn test_extract_content_length() {
        let request = "POST /dns-query HTTP/1.1\r\nContent-Length: 32\r\n";
        let length = extract_content_length(request);
        assert_eq!(length, Some(32));
    }

    #[tokio::test]
    async fn test_is_doh_request() {
        let get_request = "GET /dns-query?dns=test HTTP/1.1\r\n";
        assert!(is_doh_request(get_request).await);

        let post_request = "POST /dns-query HTTP/1.1\r\nContent-Type: application/dns-message\r\n";
        assert!(is_doh_request(post_request).await);

        let regular_request = "GET / HTTP/1.1\r\n";
        assert!(!is_doh_request(regular_request).await);
    }
}