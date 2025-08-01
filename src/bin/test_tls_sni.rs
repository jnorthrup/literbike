// Test TLS SNI extraction
use std::io::Write;

fn main() {
    // Minimal TLS Client Hello with SNI for "example.com"
    let tls_hello_with_sni = vec![
        // TLS Record Header
        0x16,               // Content Type: Handshake
        0x03, 0x01,         // TLS Version 1.0
        0x00, 0x45,         // Length: 69 bytes
        
        // Handshake Header  
        0x01,               // Handshake Type: Client Hello
        0x00, 0x00, 0x41,   // Length: 65 bytes
        
        // Client Hello
        0x03, 0x03,         // Client Version: TLS 1.2
        // Random (32 bytes)
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
        0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
        0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F,
        
        0x00,               // Session ID Length: 0
        
        0x00, 0x02,         // Cipher Suites Length: 2
        0x00, 0x35,         // Cipher Suite: TLS_RSA_WITH_AES_256_CBC_SHA
        
        0x01,               // Compression Methods Length: 1
        0x00,               // Compression Method: null
        
        0x00, 0x14,         // Extensions Length: 20 bytes (16 for SNI + 4 for header)
        
        // SNI Extension
        0x00, 0x00,         // Extension Type: server_name
        0x00, 0x10,         // Extension Length: 16 bytes
        0x00, 0x0E,         // Server Name List Length: 14 bytes
        0x00,               // Server Name Type: host_name
        0x00, 0x0B,         // Server Name Length: 11 bytes
        // "example.com" (11 bytes)
        b'e', b'x', b'a', b'm', b'p', b'l', b'e', b'.', b'c', b'o', b'm',
        
    ];
    
    let full_hello = tls_hello_with_sni;
    
    println!("Testing TLS SNI Extraction");
    println!("==========================");
    println!();
    
    // Load the litebike module and test
    let sni = extract_sni_hostname(&full_hello);
    match sni {
        Some(hostname) => println!("✅ SNI extracted: {}", hostname),
        None => println!("❌ Failed to extract SNI"),
    }
    
    // Test without SNI
    let tls_hello_no_sni = vec![
        0x16, 0x03, 0x01, 0x00, 0x2C,
        0x01, 0x00, 0x00, 0x28,
        0x03, 0x03,
        // Random (32 bytes)
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
        0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
        0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F,
        0x00,           // Session ID Length: 0
        0x00, 0x02,     // Cipher Suites Length: 2
        0x00, 0x35,     // Cipher Suite
        0x01,           // Compression Methods Length: 1
        0x00,           // Compression Method: null
        0x00, 0x00,     // Extensions Length: 0
    ];
    
    let sni2 = extract_sni_hostname(&tls_hello_no_sni);
    match sni2 {
        Some(hostname) => println!("❌ Unexpected SNI: {}", hostname),
        None => println!("✅ Correctly detected no SNI"),
    }
    
    // Test malformed
    let malformed = vec![0x16, 0x03, 0x01, 0x00, 0x05, 0x01];
    let sni3 = extract_sni_hostname(&malformed);
    match sni3 {
        Some(hostname) => println!("❌ Unexpected SNI from malformed: {}", hostname),
        None => println!("✅ Correctly rejected malformed TLS"),
    }
}

// Copy of the SNI extraction function for testing
fn extract_sni_hostname(buffer: &[u8]) -> Option<String> {
    // Minimum size: 5 (record) + 4 (handshake) + 2 (client version) + 32 (random) = 43
    if buffer.len() < 43 {
        return None;
    }
    
    // Verify this is a TLS handshake (0x16) and Client Hello (0x01)
    if buffer[0] != 0x16 || buffer[5] != 0x01 {
        return None;
    }
    
    // Skip fixed-length fields to get to session ID
    let mut pos = 43;
    
    // Skip session ID
    if pos >= buffer.len() {
        return None;
    }
    let session_id_len = buffer[pos] as usize;
    pos += 1 + session_id_len;
    
    // Skip cipher suites
    if pos + 2 > buffer.len() {
        return None;
    }
    let cipher_suites_len = u16::from_be_bytes([buffer[pos], buffer[pos + 1]]) as usize;
    pos += 2 + cipher_suites_len;
    
    // Skip compression methods
    if pos >= buffer.len() {
        return None;
    }
    let compression_len = buffer[pos] as usize;
    pos += 1 + compression_len;
    
    // Extensions length
    if pos + 2 > buffer.len() {
        return None;
    }
    let extensions_len = u16::from_be_bytes([buffer[pos], buffer[pos + 1]]) as usize;
    pos += 2;
    
    let extensions_end = pos + extensions_len;
    if extensions_end > buffer.len() {
        return None;
    }
    
    // Parse extensions to find SNI (type 0x0000)
    while pos + 4 <= extensions_end {
        let ext_type = u16::from_be_bytes([buffer[pos], buffer[pos + 1]]);
        let ext_len = u16::from_be_bytes([buffer[pos + 2], buffer[pos + 3]]) as usize;
        pos += 4;
        
        if pos + ext_len > extensions_end {
            break;
        }
        
        if ext_type == 0x0000 {  // SNI extension
            // SNI format: list length (2) + type (1) + hostname length (2) + hostname
            if ext_len >= 5 && pos + 5 <= buffer.len() {
                let name_type = buffer[pos + 2];
                if name_type == 0x00 {  // host_name type
                    let name_len = u16::from_be_bytes([buffer[pos + 3], buffer[pos + 4]]) as usize;
                    let name_start = pos + 5;
                    if name_start + name_len <= buffer.len() {
                        return String::from_utf8(buffer[name_start..name_start + name_len].to_vec()).ok();
                    }
                }
            }
            break;
        }
        
        pos += ext_len;
    }
    
    None
}