// Analyze why tiling doesn't make sense for protocol detection

fn main() {
    println!("Protocol Detection Byte Requirements:\n");
    
    println!("SOCKS5:          1 byte   (0x05)");
    println!("TLS:             3 bytes  (0x16 0x03 0x??)");
    println!("HTTP GET:        4 bytes  (GET )");
    println!("HTTP POST:       5 bytes  (POST )");
    println!("HTTP PROXY:      6 bytes  (PROXY )");
    println!("HTTP DELETE:     7 bytes  (DELETE )");
    println!("HTTP OPTIONS:    8 bytes  (OPTIONS )");
    println!("HTTP/2 preface: 14 bytes  (PRI * HTTP/2.0)");
    
    println!("\nCache Analysis:");
    println!("- L1 cache line: 64 bytes");
    println!("- Max protocol detection: 14 bytes");
    println!("- All protocols fit in 1/4 of a cache line");
    
    println!("\nWhy tiling is pointless here:");
    println!("1. Data already fits in L1 cache");
    println!("2. Single sequential read pattern");
    println!("3. No spatial locality to exploit");
    println!("4. No repeated access patterns");
    
    println!("\nWhere tiling would help:");
    println!("- Processing 1MB+ of HTTP body data");
    println!("- Parsing large JSON/XML responses");
    println!("- Video streaming through the proxy");
    println!("- NOT for protocol detection");
}