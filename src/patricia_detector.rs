// Patricia Trie based Protocol Detector for Universal Port 8080
// Efficiently identifies protocols from initial bytes using a radix tree

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Protocol {
    Http,
    Socks5,
    Tls,
    WebSocket,
    Unknown,
}

#[derive(Debug)]
struct TrieNode {
    children: HashMap<u8, Box<TrieNode>>,
    protocol: Option<Protocol>,
    prefix_len: usize,
}

impl TrieNode {
    fn new() -> Self {
        TrieNode {
            children: HashMap::new(),
            protocol: None,
            prefix_len: 0,
        }
    }
}

pub struct PatriciaDetector {
    root: TrieNode,
}

impl PatriciaDetector {
    pub fn new() -> Self {
        let mut detector = PatriciaDetector {
            root: TrieNode::new(),
        };
        detector.build_trie();
        detector
    }

    fn build_trie(&mut self) {
        // HTTP methods - all start with uppercase ASCII
        self.insert(b"GET ", Protocol::Http);
        self.insert(b"POST ", Protocol::Http);
        self.insert(b"PUT ", Protocol::Http);
        self.insert(b"DELETE ", Protocol::Http);
        self.insert(b"HEAD ", Protocol::Http);
        self.insert(b"OPTIONS ", Protocol::Http);
        self.insert(b"CONNECT ", Protocol::Http);
        self.insert(b"PATCH ", Protocol::Http);
        self.insert(b"TRACE ", Protocol::Http);

        // SOCKS5 - starts with version byte 0x05
        self.insert(&[0x05], Protocol::Socks5);

        // TLS/SSL - starts with handshake 0x16 followed by version
        self.insert(&[0x16, 0x03, 0x00], Protocol::Tls); // SSL 3.0
        self.insert(&[0x16, 0x03, 0x01], Protocol::Tls); // TLS 1.0
        self.insert(&[0x16, 0x03, 0x02], Protocol::Tls); // TLS 1.1
        self.insert(&[0x16, 0x03, 0x03], Protocol::Tls); // TLS 1.2
        self.insert(&[0x16, 0x03, 0x04], Protocol::Tls); // TLS 1.3

        // WebSocket - HTTP with Upgrade header, but starts like HTTP
        // Will be detected as HTTP first, then upgraded
    }

    fn insert(&mut self, prefix: &[u8], protocol: Protocol) {
        let mut current = &mut self.root;
        
        for &byte in prefix {
            current = current.children.entry(byte).or_insert_with(|| Box::new(TrieNode::new()));
        }
        
        current.protocol = Some(protocol);
        current.prefix_len = prefix.len();
    }

    pub fn detect(&self, buffer: &[u8]) -> Protocol {
        if buffer.is_empty() {
            return Protocol::Unknown;
        }

        let mut current = &self.root;
        let mut last_match = None;

        for (i, &byte) in buffer.iter().enumerate() {
            if let Some(node) = current.children.get(&byte) {
                current = node;
                if let Some(ref proto) = current.protocol {
                    last_match = Some((proto.clone(), i + 1));
                }
            } else {
                break;
            }
        }

        // Return the longest matching protocol
        last_match.map(|(proto, _)| proto).unwrap_or(Protocol::Unknown)
    }

    // Optimized detection that returns protocol and consumed bytes
    pub fn detect_with_length(&self, buffer: &[u8]) -> (Protocol, usize) {
        if buffer.is_empty() {
            return (Protocol::Unknown, 0);
        }

        let mut current = &self.root;
        let mut last_match = (Protocol::Unknown, 0);

        for (i, &byte) in buffer.iter().enumerate() {
            if let Some(node) = current.children.get(&byte) {
                current = node;
                if let Some(ref proto) = current.protocol {
                    last_match = (proto.clone(), i + 1);
                }
            } else {
                break;
            }
        }

        last_match
    }
}

// Fast bitwise protocol detection for common cases
#[inline]
pub fn quick_detect(buffer: &[u8]) -> Option<Protocol> {
    if buffer.len() < 2 {
        return None;
    }

    match buffer[0] {
        // SOCKS5 version
        0x05 => Some(Protocol::Socks5),
        
        // TLS handshake
        0x16 if buffer.len() >= 3 && buffer[1] == 0x03 => Some(Protocol::Tls),
        
        // HTTP methods start with uppercase letters
        b'G' | b'P' | b'H' | b'D' | b'O' | b'C' | b'T' => {
            // Quick check for space after method
            if buffer.len() >= 4 {
                match &buffer[0..4] {
                    b"GET " | b"PUT " => Some(Protocol::Http),
                    b"POST" | b"HEAD" if buffer.len() > 4 && buffer[4] == b' ' => Some(Protocol::Http),
                    _ => None,
                }
            } else {
                None
            }
        }
        
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_detection() {
        let detector = PatriciaDetector::new();
        
        assert!(matches!(detector.detect(b"GET / HTTP/1.1\r\n"), Protocol::Http));
        assert!(matches!(detector.detect(b"POST /api"), Protocol::Http));
        assert!(matches!(detector.detect(b"CONNECT example.com:443"), Protocol::Http));
    }

    #[test]
    fn test_socks5_detection() {
        let detector = PatriciaDetector::new();
        
        assert!(matches!(detector.detect(&[0x05, 0x01, 0x00]), Protocol::Socks5));
    }

    #[test]
    fn test_tls_detection() {
        let detector = PatriciaDetector::new();
        
        assert!(matches!(detector.detect(&[0x16, 0x03, 0x01]), Protocol::Tls));
        assert!(matches!(detector.detect(&[0x16, 0x03, 0x03]), Protocol::Tls));
    }

    #[test]
    fn test_quick_detect() {
        assert!(matches!(quick_detect(b"GET /"), Some(Protocol::Http)));
        assert!(matches!(quick_detect(&[0x05, 0x01]), Some(Protocol::Socks5)));
        assert!(matches!(quick_detect(&[0x16, 0x03, 0x01]), Some(Protocol::Tls)));
    }
}