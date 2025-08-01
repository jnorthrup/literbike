use litebike::patricia_detector::{PatriciaDetector, Protocol, quick_detect};

fn main() {
    let detector = PatriciaDetector::new();
    
    // Test cases
    let test_cases = vec![
        (b"GET / HTTP/1.1\r\n".to_vec(), "HTTP GET"),
        (b"POST /api HTTP/1.1\r\n".to_vec(), "HTTP POST"),
        (b"CONNECT example.com:443 HTTP/1.1\r\n".to_vec(), "HTTP CONNECT"),
        (vec![0x05, 0x01, 0x00], "SOCKS5 handshake"),
        (vec![0x16, 0x03, 0x01, 0x00, 0x00], "TLS 1.0"),
        (vec![0x16, 0x03, 0x03, 0x00, 0x00], "TLS 1.2"),
        (b"INVALID DATA".to_vec(), "Unknown"),
    ];
    
    println!("Testing Patricia Trie Protocol Detection:");
    println!("-----------------------------------------");
    
    for (data, desc) in test_cases {
        let (protocol, len) = detector.detect_with_length(&data);
        println!("{}: {:?} (matched {} bytes)", desc, protocol, len);
        
        // Also test quick detect
        if let Some(quick) = quick_detect(&data) {
            println!("  Quick detect: {:?}", quick);
        }
    }
}