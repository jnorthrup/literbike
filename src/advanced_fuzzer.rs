use std::io;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use rand::prelude::*;
use log::{debug, info, warn, error};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};

use crate::types::ProtocolType;

pub struct UniversalPortAbuser {
    target_host: String,
    target_port: u16,
    attack_vectors: Vec<AttackVector>,
    payload_mutations: Vec<MutationType>,
    results: HashMap<String, AttackResult>,
}

#[derive(Debug, Clone)]
pub enum AttackVector {
    ProtocolConfusion,
    ProtocolSpoofing,
    HeaderInjection,
    StateConfusion,
    TimingAttack,
    ResourceExhaustion,
    ProtocolDowngrade,
    ProtocolUpgrade,
    BitFlipping,
    LengthManipulation,
    SequenceBreaking,
    HandshakeCorruption,
}

#[derive(Debug, Clone)]
pub enum MutationType {
    BitFlip(f32),      // Probability of flipping each bit
    ByteSwap(usize),   // Swap bytes at distance
    Truncate(f32),     // Truncate payload by percentage
    Extend(usize),     // Extend with random bytes
    Corrupt(f32),      // Corrupt percentage of bytes
    Duplicate(usize),  // Duplicate sections
    Insert(usize),     // Insert random data
    Shuffle(usize),    // Shuffle byte order
}

#[derive(Debug, Clone, Default)]
pub struct AttackResult {
    pub attempts: usize,
    pub crashes: usize,
    pub hangs: usize,
    pub unexpected_responses: usize,
    pub protocol_confusion_successes: usize,
    pub resource_consumption: Vec<Duration>,
    pub error_types: HashMap<String, usize>,
}

impl UniversalPortAbuser {
    pub fn new(target_host: String, target_port: u16) -> Self {
        Self {
            target_host,
            target_port,
            attack_vectors: vec![
                AttackVector::ProtocolConfusion,
                AttackVector::ProtocolSpoofing,
                AttackVector::HeaderInjection,
                AttackVector::StateConfusion,
                AttackVector::TimingAttack,
                AttackVector::ResourceExhaustion,
                AttackVector::ProtocolDowngrade,
                AttackVector::BitFlipping,
                AttackVector::LengthManipulation,
                AttackVector::SequenceBreaking,
                AttackVector::HandshakeCorruption,
            ],
            payload_mutations: vec![
                MutationType::BitFlip(0.01),
                MutationType::ByteSwap(4),
                MutationType::Truncate(0.5),
                MutationType::Extend(100),
                MutationType::Corrupt(0.1),
                MutationType::Duplicate(8),
                MutationType::Insert(50),
                MutationType::Shuffle(16),
            ],
            results: HashMap::new(),
        }
    }

    pub async fn abuse_universal_port(&mut self, iterations: usize) -> io::Result<()> {
        info!("Starting universal port abuse against {}:{} with {} iterations", 
              self.target_host, self.target_port, iterations);

        for vector in &self.attack_vectors.clone() {
            info!("Executing attack vector: {:?}", vector);
            let result = self.execute_attack_vector(vector, iterations / self.attack_vectors.len()).await?;
            self.results.insert(format!("{:?}", vector), result);
        }

        self.multi_vector_chaos_attack(100).await?;
        self.protocol_state_machine_attack(50).await?;
        self.resource_exhaustion_attack(25).await?;

        self.print_abuse_results();
        Ok(())
    }

    async fn execute_attack_vector(&self, vector: &AttackVector, iterations: usize) -> io::Result<AttackResult> {
        let mut result = AttackResult::default();
        let mut rng = thread_rng();

        for i in 0..iterations {
            if i % 50 == 0 {
                debug!("Attack vector {:?} - iteration {}/{}", vector, i, iterations);
            }

            let payload = self.generate_attack_payload(vector, &mut rng);
            let mutated_payload = self.mutate_payload(payload, &mut rng);
            
            let start_time = Instant::now();
            match self.send_malicious_payload(&mutated_payload).await {
                Ok(response) => {
                    let elapsed = start_time.elapsed();
                    result.resource_consumption.push(elapsed);
                    result.attempts += 1;

                    if self.is_unexpected_response(&response) {
                        result.unexpected_responses += 1;
                    }

                    if self.indicates_protocol_confusion(&response) {
                        result.protocol_confusion_successes += 1;
                    }
                }
                Err(e) => {
                    result.attempts += 1;
                    let error_key = format!("{:?}", e.kind());
                    *result.error_types.entry(error_key).or_insert(0) += 1;

                    if e.kind() == io::ErrorKind::TimedOut {
                        result.hangs += 1;
                    } else if e.kind() == io::ErrorKind::ConnectionRefused ||
                             e.kind() == io::ErrorKind::ConnectionReset {
                        result.crashes += 1;
                    }
                }
            }

            // Randomize timing to avoid detection
            sleep(Duration::from_millis(rng.gen_range(1..=10))).await;
        }

        Ok(result)
    }

    fn generate_attack_payload(&self, vector: &AttackVector, rng: &mut ThreadRng) -> Vec<u8> {
        match vector {
            AttackVector::ProtocolConfusion => self.create_protocol_confusion_payload(rng),
            AttackVector::ProtocolSpoofing => self.create_protocol_spoofing_payload(rng),
            AttackVector::HeaderInjection => self.create_header_injection_payload(rng),
            AttackVector::StateConfusion => self.create_state_confusion_payload(rng),
            AttackVector::TimingAttack => self.create_timing_attack_payload(rng),
            AttackVector::ResourceExhaustion => self.create_resource_exhaustion_payload(rng),
            AttackVector::ProtocolDowngrade => self.create_protocol_downgrade_payload(rng),
            AttackVector::ProtocolUpgrade => self.create_protocol_upgrade_payload(rng),
            AttackVector::BitFlipping => self.create_bit_flipping_payload(rng),
            AttackVector::LengthManipulation => self.create_length_manipulation_payload(rng),
            AttackVector::SequenceBreaking => self.create_sequence_breaking_payload(rng),
            AttackVector::HandshakeCorruption => self.create_handshake_corruption_payload(rng),
        }
    }

    fn create_protocol_confusion_payload(&self, rng: &mut ThreadRng) -> Vec<u8> {
        let mut payload = Vec::new();
        
        // Layer multiple protocol headers
        payload.extend_from_slice(b"GET / HTTP/1.1\r\n");           // HTTP
        payload.extend_from_slice(&[0x05, 0x01, 0x00]);            // SOCKS5
        payload.extend_from_slice(&[0x16, 0x03, 0x03, 0x00, 0x10]); // TLS
        payload.extend_from_slice(b"SSH-2.0-Evil\r\n");            // SSH
        payload.extend_from_slice(&[0x13]);                        // BitTorrent length
        payload.extend_from_slice(b"BitTorrent protocol");         // BitTorrent
        payload.extend_from_slice(b"\xFF\xFE\x00\x00");           // SMB2
        payload.extend_from_slice(&[0x80, 0x00, 0x00, 0x00, 0x01]); // QUIC
        
        // Add random garbage
        let garbage_size = rng.gen_range(50..500);
        let mut garbage = vec![0u8; garbage_size];
        rng.fill_bytes(&mut garbage);
        payload.extend_from_slice(&garbage);
        
        payload
    }

    fn create_protocol_spoofing_payload(&self, rng: &mut ThreadRng) -> Vec<u8> {
        let protocols = [
            (b"HTTP/1.1 200 OK\r\n\r\n", "HTTP Response as Request"),
            (b"\x05\x00\x00\x01\x00\x00\x00\x00\x00\x00", "SOCKS5 Malformed"),
            (b"220 Welcome to Evil FTP\r\n", "FTP Greeting as Request"),
            (b"RFB 003.008\n", "VNC Handshake"),
            (&[0x03, 0x00, 0x00, 0x0B, 0x06, 0xE0, 0x00, 0x00, 0x00, 0x00, 0x00], "Malformed RDP"),
        ];
        
        let (proto_bytes, _desc) = protocols[rng.gen_range(0..protocols.len())];
        let mut payload = proto_bytes.to_vec();
        
        // Corrupt some bytes
        for _ in 0..rng.gen_range(1..5) {
            if !payload.is_empty() {
                let idx = rng.gen_range(0..payload.len());
                payload[idx] = rng.gen();
            }
        }
        
        payload
    }

    fn create_header_injection_payload(&self, rng: &mut ThreadRng) -> Vec<u8> {
        let injections = [
            "\r\n\r\nGET /evil HTTP/1.1\r\n",
            "\x00\r\nHTTP/1.1 200 OK\r\n",
            "Host: evil.com\r\nX-Injected: true\r\n\r\n",
            "\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n",
        ];
        
        let mut payload = b"GET / HTTP/1.1\r\nHost: ".to_vec();
        let injection = injections[rng.gen_range(0..injections.len())];
        payload.extend_from_slice(injection.as_bytes());
        payload.extend_from_slice(b"\r\n\r\n");
        
        payload
    }

    fn create_state_confusion_payload(&self, rng: &mut ThreadRng) -> Vec<u8> {
        let mut payload = Vec::new();
        
        // Send protocol handshake fragments out of order
        let fragments = [
            &[0x16, 0x03, 0x03] as &[u8],     // TLS handshake start
            b"HTTP/1.1 ",                      // HTTP method
            &[0x05],                          // SOCKS5 version
            b"Host: test.com\r\n",            // HTTP header
            &[0x01, 0x00],                    // SOCKS5 nmethods
            b"\r\n\r\n",                      // HTTP end
        ];
        
        // Shuffle fragments
        let mut indices: Vec<usize> = (0..fragments.len()).collect();
        indices.shuffle(rng);
        
        for &i in &indices {
            payload.extend_from_slice(fragments[i]);
        }
        
        payload
    }

    fn create_timing_attack_payload(&self, _rng: &mut ThreadRng) -> Vec<u8> {
        // This payload is designed to be sent with specific timing
        b"TIMING_ATTACK_MARKER".to_vec()
    }

    fn create_resource_exhaustion_payload(&self, rng: &mut ThreadRng) -> Vec<u8> {
        let size = rng.gen_range(1024..65536);
        let mut payload = vec![0x41; size]; // Large payload of 'A's
        
        // Add some structure to avoid simple filtering
        for i in (0..payload.len()).step_by(100) {
            if i + 10 < payload.len() {
                payload[i..i+10].copy_from_slice(b"EXHAUSTION");
            }
        }
        
        payload
    }

    fn create_protocol_downgrade_payload(&self, rng: &mut ThreadRng) -> Vec<u8> {
        let downgrades = [
            b"GET / HTTP/0.9\r\n\r\n",                    // HTTP/0.9
            &[0x16, 0x02, 0x00],                          // TLS 1.0 (try to force downgrade)
            b"SSH-1.5-Downgrade\r\n",                     // Old SSH version
            &[0x04, 0x01, 0x00],                          // SOCKS4 instead of SOCKS5
        ];
        
        downgrades[rng.gen_range(0..downgrades.len())].to_vec()
    }

    fn create_protocol_upgrade_payload(&self, _rng: &mut ThreadRng) -> Vec<u8> {
        b"GET / HTTP/1.1\r\nConnection: Upgrade\r\nUpgrade: h2c\r\nHTTP2-Settings: \r\n\r\n".to_vec()
    }

    fn create_bit_flipping_payload(&self, rng: &mut ThreadRng) -> Vec<u8> {
        let base = b"GET / HTTP/1.1\r\nHost: test.com\r\n\r\n";
        let mut payload = base.to_vec();
        
        // Flip random bits
        for _ in 0..rng.gen_range(1..10) {
            if !payload.is_empty() {
                let byte_idx = rng.gen_range(0..payload.len());
                let bit_idx = rng.gen_range(0..8);
                payload[byte_idx] ^= 1 << bit_idx;
            }
        }
        
        payload
    }

    fn create_length_manipulation_payload(&self, rng: &mut ThreadRng) -> Vec<u8> {
        let mut payload = Vec::new();
        
        // HTTP with Content-Length manipulation
        payload.extend_from_slice(b"POST / HTTP/1.1\r\n");
        payload.extend_from_slice(b"Host: test.com\r\n");
        
        let claimed_length = rng.gen_range(1..1000);
        let actual_length = rng.gen_range(1..1000);
        
        payload.extend_from_slice(format!("Content-Length: {}\r\n\r\n", claimed_length).as_bytes());
        
        // Send different amount than claimed
        let body = vec![b'X'; actual_length];
        payload.extend_from_slice(&body);
        
        payload
    }

    fn create_sequence_breaking_payload(&self, rng: &mut ThreadRng) -> Vec<u8> {
        let mut payload = Vec::new();
        
        // SOCKS5 sequence breaking
        payload.push(0x05); // Version
        payload.push(0x02); // 2 methods
        payload.push(0x00); // No auth
        payload.push(0x02); // Username/password
        
        // Skip handshake, jump to request
        payload.extend_from_slice(&[0x05, 0x01, 0x00, 0x01]); // CONNECT IPv4
        payload.extend_from_slice(&rng.gen::<[u8; 4]>()); // Random IP
        payload.extend_from_slice(&rng.gen::<[u8; 2]>()); // Random port
        
        payload
    }

    fn create_handshake_corruption_payload(&self, rng: &mut ThreadRng) -> Vec<u8> {
        let mut payload = Vec::new();
        
        // Corrupt TLS handshake
        payload.extend_from_slice(&[0x16, 0x03, 0x03]); // Content type, version
        payload.extend_from_slice(&(rng.gen::<u16>()).to_be_bytes()); // Random length
        payload.extend_from_slice(&[0x01]); // Handshake type: Client Hello
        payload.extend_from_slice(&[0x00, 0x00, 0x00]); // Length (corrupted)
        payload.extend_from_slice(&rng.gen::<[u8; 32]>()); // Random client version + random
        payload.extend_from_slice(&[0x00]); // Session ID length
        payload.extend_from_slice(&(rng.gen::<u16>()).to_be_bytes()); // Cipher suites length
        
        payload
    }

    fn mutate_payload(&self, mut payload: Vec<u8>, rng: &mut ThreadRng) -> Vec<u8> {
        if payload.is_empty() {
            return payload;
        }

        let mutation = &self.payload_mutations[rng.gen_range(0..self.payload_mutations.len())];
        
        match mutation {
            MutationType::BitFlip(prob) => {
                for byte in &mut payload {
                    if rng.gen::<f32>() < *prob {
                        let bit = rng.gen_range(0..8);
                        *byte ^= 1 << bit;
                    }
                }
            }
            MutationType::ByteSwap(distance) => {
                if payload.len() > *distance {
                    for i in 0..payload.len() - distance {
                        payload.swap(i, i + distance);
                    }
                }
            }
            MutationType::Truncate(ratio) => {
                let new_len = (payload.len() as f32 * ratio) as usize;
                payload.truncate(new_len);
            }
            MutationType::Extend(size) => {
                let mut extension = vec![0u8; *size];
                rng.fill_bytes(&mut extension);
                payload.extend_from_slice(&extension);
            }
            MutationType::Corrupt(ratio) => {
                let corrupt_count = (payload.len() as f32 * ratio) as usize;
                for _ in 0..corrupt_count {
                    if !payload.is_empty() {
                        let idx = rng.gen_range(0..payload.len());
                        payload[idx] = rng.gen();
                    }
                }
            }
            MutationType::Duplicate(size) => {
                if payload.len() >= *size {
                    let start = rng.gen_range(0..payload.len() - size);
                    let duplicate = payload[start..start + size].to_vec();
                    payload.extend_from_slice(&duplicate);
                }
            }
            MutationType::Insert(size) => {
                let mut insertion = vec![0u8; *size];
                rng.fill_bytes(&mut insertion);
                let pos = rng.gen_range(0..=payload.len());
                payload.splice(pos..pos, insertion);
            }
            MutationType::Shuffle(chunk_size) => {
                if payload.len() >= *chunk_size {
                    let chunks: Vec<_> = payload.chunks_mut(*chunk_size).collect();
                    for chunk in chunks {
                        chunk.shuffle(rng);
                    }
                }
            }
        }
        
        payload
    }

    async fn send_malicious_payload(&self, payload: &[u8]) -> io::Result<Vec<u8>> {
        let mut stream = timeout(
            Duration::from_secs(5),
            TcpStream::connect((&self.target_host[..], self.target_port))
        ).await??;

        stream.write_all(payload).await?;
        
        let mut response = Vec::new();
        let mut buffer = [0u8; 4096];
        
        match timeout(Duration::from_secs(2), stream.read(&mut buffer)).await {
            Ok(Ok(n)) => {
                response.extend_from_slice(&buffer[..n]);
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(io::Error::new(io::ErrorKind::TimedOut, "Read timeout")),
        }
        
        Ok(response)
    }

    fn is_unexpected_response(&self, response: &[u8]) -> bool {
        let response_str = String::from_utf8_lossy(response);
        
        // Look for error messages, debug info, or unexpected protocols
        response_str.contains("error") ||
        response_str.contains("debug") ||
        response_str.contains("internal") ||
        response_str.contains("exception") ||
        response_str.contains("stack") ||
        response.len() > 8192 || // Unusually large response
        (response.len() < 10 && !response.is_empty()) // Unusually small response
    }

    fn indicates_protocol_confusion(&self, response: &[u8]) -> bool {
        let response_str = String::from_utf8_lossy(response);
        
        // Multiple protocol indicators in single response
        let mut protocol_indicators = 0;
        
        if response_str.contains("HTTP/") { protocol_indicators += 1; }
        if response.windows(3).any(|w| w == [0x05, 0x00, 0x00] || w == [0x05, 0x01, 0x00]) { protocol_indicators += 1; }
        if response.windows(3).any(|w| w == [0x16, 0x03, 0x03]) { protocol_indicators += 1; }
        if response_str.contains("SSH-") { protocol_indicators += 1; }
        if response_str.contains("220 ") { protocol_indicators += 1; }
        
        protocol_indicators > 1
    }

    async fn multi_vector_chaos_attack(&self, iterations: usize) -> io::Result<()> {
        info!("Executing multi-vector chaos attack");
        let mut rng = thread_rng();
        
        for _ in 0..iterations {
            // Combine multiple attack vectors in single payload
            let mut payload = Vec::new();
            let num_vectors = rng.gen_range(2..5);
            
            for _ in 0..num_vectors {
                let vector = &self.attack_vectors[rng.gen_range(0..self.attack_vectors.len())];
                let sub_payload = self.generate_attack_payload(vector, &mut rng);
                payload.extend_from_slice(&sub_payload);
            }
            
            let mutated = self.mutate_payload(payload, &mut rng);
            let _ = self.send_malicious_payload(&mutated).await;
            
            sleep(Duration::from_millis(rng.gen_range(1..=5))).await;
        }
        
        Ok(())
    }

    async fn protocol_state_machine_attack(&self, iterations: usize) -> io::Result<()> {
        info!("Executing protocol state machine attack");
        let mut rng = thread_rng();
        
        for _ in 0..iterations {
            if let Ok(mut stream) = TcpStream::connect((&self.target_host[..], self.target_port)).await {
                // Send protocol messages in invalid states
                let messages = [
                    b"220 I am server\r\n",           // FTP server greeting as client
                    &[0x05, 0x00],                    // SOCKS5 response without request
                    b"HTTP/1.1 200 OK\r\n\r\n",      // HTTP response without request
                    b"SSH-2.0-Server\r\n",           // SSH server banner as client
                    &[0x16, 0x03, 0x03, 0x00, 0x02, 0x02, 0x00], // TLS server hello
                ];
                
                for msg in &messages {
                    let _ = stream.write_all(msg).await;
                    sleep(Duration::from_millis(rng.gen_range(10..100))).await;
                }
            }
        }
        
        Ok(())
    }

    async fn resource_exhaustion_attack(&self, iterations: usize) -> io::Result<()> {
        info!("Executing resource exhaustion attack");
        let mut handles = Vec::new();
        
        for _ in 0..iterations {
            let host = self.target_host.clone();
            let port = self.target_port;
            
            let handle = tokio::spawn(async move {
                for _ in 0..10 {
                    if let Ok(mut stream) = TcpStream::connect((&host[..], port)).await {
                        // Send large payload slowly (slow loris style)
                        let large_payload = vec![b'A'; 65536];
                        for chunk in large_payload.chunks(1) {
                            let _ = stream.write_all(chunk).await;
                            sleep(Duration::from_millis(100)).await;
                        }
                    }
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for some to complete, others will keep connections open
        sleep(Duration::from_secs(10)).await;
        
        Ok(())
    }

    fn print_abuse_results(&self) {
        info!("=== UNIVERSAL PORT ABUSE RESULTS ===");
        
        for (vector, result) in &self.results {
            info!("Attack Vector: {}", vector);
            info!("  Total attempts: {}", result.attempts);
            info!("  Crashes detected: {} ({:.2}%)", 
                  result.crashes, 
                  result.crashes as f64 / result.attempts as f64 * 100.0);
            info!("  Hangs detected: {} ({:.2}%)", 
                  result.hangs,
                  result.hangs as f64 / result.attempts as f64 * 100.0);
            info!("  Unexpected responses: {} ({:.2}%)", 
                  result.unexpected_responses,
                  result.unexpected_responses as f64 / result.attempts as f64 * 100.0);
            info!("  Protocol confusion successes: {} ({:.2}%)", 
                  result.protocol_confusion_successes,
                  result.protocol_confusion_successes as f64 / result.attempts as f64 * 100.0);
            
            if !result.resource_consumption.is_empty() {
                let avg_time: Duration = result.resource_consumption.iter().sum::<Duration>() / result.resource_consumption.len() as u32;
                let max_time = result.resource_consumption.iter().max().unwrap_or(&Duration::ZERO);
                info!("  Avg response time: {:?}", avg_time);
                info!("  Max response time: {:?}", max_time);
            }
            
            if !result.error_types.is_empty() {
                info!("  Error distribution:");
                for (error_type, count) in &result.error_types {
                    info!("    {}: {} ({:.2}%)", error_type, count, 
                          *count as f64 / result.attempts as f64 * 100.0);
                }
            }
            
            info!("");
        }
        
        let total_attempts: usize = self.results.values().map(|r| r.attempts).sum();
        let total_anomalies: usize = self.results.values().map(|r| {
            r.crashes + r.hangs + r.unexpected_responses + r.protocol_confusion_successes
        }).sum();
        
        info!("=== SUMMARY ===");
        info!("Total attack attempts: {}", total_attempts);
        info!("Total anomalies detected: {} ({:.2}%)", 
              total_anomalies,
              total_anomalies as f64 / total_attempts as f64 * 100.0);
        
        if total_anomalies > 0 {
            warn!("⚠️  Universal port shows vulnerabilities to abuse!");
        } else {
            info!("✅ Universal port appears robust against tested attack vectors");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abuser_creation() {
        let abuser = UniversalPortAbuser::new("127.0.0.1".to_string(), 8080);
        assert_eq!(abuser.target_host, "127.0.0.1");
        assert_eq!(abuser.target_port, 8080);
        assert!(!abuser.attack_vectors.is_empty());
    }

    #[tokio::test]
    async fn test_payload_generation() {
        let abuser = UniversalPortAbuser::new("127.0.0.1".to_string(), 8080);
        let mut rng = thread_rng();
        
        let payload = abuser.generate_attack_payload(&AttackVector::ProtocolConfusion, &mut rng);
        assert!(!payload.is_empty());
        
        let payload_str = String::from_utf8_lossy(&payload);
        assert!(payload_str.contains("HTTP") || payload.windows(3).any(|w| w == [0x05, 0x01, 0x00]));
    }

    #[test]
    fn test_payload_mutation() {
        let abuser = UniversalPortAbuser::new("127.0.0.1".to_string(), 8080);
        let mut rng = thread_rng();
        
        let original = b"Hello, World!".to_vec();
        let mutated = abuser.mutate_payload(original.clone(), &mut rng);
        
        // Mutation should change something (high probability)
        assert!(mutated != original || mutated.len() != original.len());
    }
}