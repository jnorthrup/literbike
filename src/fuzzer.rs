use std::io;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use rand::prelude::*;
use log::{debug, info, warn, error};
use tokio::io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};

use crate::types::{ProtocolType, BitFlags, ProtocolDetectionResult};
use crate::abstractions::{ProtocolDetector, UniversalProxy, UniversalConnection};

pub struct ProtocolFuzzer {
    target_host: String,
    target_port: u16,
    iterations: usize,
    max_payload_size: usize,
    protocols_to_fuzz: Vec<ProtocolType>,
    results: HashMap<ProtocolType, FuzzResult>,
}

#[derive(Debug, Clone, Default)]
pub struct FuzzResult {
    pub attempts: usize,
    pub successful_detections: usize,
    pub false_positives: usize,
    pub crashes: usize,
    pub timeouts: usize,
    pub avg_response_time: Duration,
    pub max_response_time: Duration,
    pub min_response_time: Duration,
    pub payload_sizes_tested: Vec<usize>,
    pub detection_confidence_scores: Vec<u8>,
}

impl ProtocolFuzzer {
    pub fn new(target_host: String, target_port: u16) -> Self {
        Self {
            target_host,
            target_port,
            iterations: 1000,
            max_payload_size: 8192,
            protocols_to_fuzz: vec![
                ProtocolType::Http,
                ProtocolType::Https,
                ProtocolType::Socks5,
                ProtocolType::Tls,
                ProtocolType::Doh,
                ProtocolType::Upnp,
                ProtocolType::Bonjour,
                ProtocolType::Shadowsocks,
                ProtocolType::WebRtc,
                ProtocolType::Quic,
                ProtocolType::Ssh,
                ProtocolType::Ftp,
                ProtocolType::Smtp,
                ProtocolType::Irc,
                ProtocolType::Websocket,
                ProtocolType::Mqtt,
                ProtocolType::Sip,
                ProtocolType::Rtsp,
            ],
            results: HashMap::new(),
        }
    }

    pub fn set_iterations(&mut self, iterations: usize) {
        self.iterations = iterations;
    }

    pub fn set_max_payload_size(&mut self, size: usize) {
        self.max_payload_size = size;
    }

    pub async fn run_comprehensive_fuzz(&mut self) -> io::Result<()> {
        info!("Starting comprehensive protocol fuzzing against {}:{}", 
              self.target_host, self.target_port);

        for protocol in &self.protocols_to_fuzz.clone() {
            info!("Fuzzing protocol: {}", protocol);
            let result = self.fuzz_protocol(*protocol).await?;
            self.results.insert(*protocol, result);
        }

        self.fuzz_malformed_payloads().await?;
        self.fuzz_edge_cases().await?;
        self.fuzz_timing_attacks().await?;
        self.fuzz_protocol_confusion().await?;

        self.print_results();
        Ok(())
    }

    async fn fuzz_protocol(&mut self, protocol: ProtocolType) -> io::Result<FuzzResult> {
        let mut result = FuzzResult::default();
        let mut rng = thread_rng();

        for i in 0..self.iterations {
            if i % 100 == 0 {
                debug!("Fuzzing {} - iteration {}/{}", protocol, i, self.iterations);
            }

            let payload = self.generate_protocol_payload(protocol, &mut rng);
            let start_time = Instant::now();

            match self.send_fuzz_payload(&payload).await {
                Ok(detection_result) => {
                    let elapsed = start_time.elapsed();
                    result.attempts += 1;
                    result.avg_response_time = 
                        (result.avg_response_time * (result.attempts - 1) as u32 + elapsed) / result.attempts as u32;
                    result.max_response_time = result.max_response_time.max(elapsed);
                    result.min_response_time = if result.min_response_time == Duration::ZERO {
                        elapsed
                    } else {
                        result.min_response_time.min(elapsed)
                    };

                    if detection_result.protocol == protocol {
                        result.successful_detections += 1;
                    } else if detection_result.protocol != ProtocolType::Raw {
                        result.false_positives += 1;
                    }

                    result.detection_confidence_scores.push(detection_result.confidence);
                    result.payload_sizes_tested.push(payload.len());
                }
                Err(e) => {
                    if e.kind() == io::ErrorKind::TimedOut {
                        result.timeouts += 1;
                    } else {
                        result.crashes += 1;
                        warn!("Fuzz test crashed for {}: {}", protocol, e);
                    }
                }
            }

            // Small delay to avoid overwhelming the target
            sleep(Duration::from_millis(1)).await;
        }

        Ok(result)
    }

    async fn fuzz_malformed_payloads(&mut self) -> io::Result<()> {
        info!("Fuzzing with malformed payloads");
        let mut rng = thread_rng();

        for _ in 0..200 {
            let size = rng.gen_range(1..self.max_payload_size);
            let mut payload = vec![0u8; size];
            rng.fill_bytes(&mut payload);

            // Inject common protocol markers at random positions
            self.inject_protocol_confusion(&mut payload, &mut rng);

            if let Err(e) = self.send_fuzz_payload(&payload).await {
                if e.kind() != io::ErrorKind::TimedOut {
                    warn!("Malformed payload caused error: {}", e);
                }
            }
        }

        Ok(())
    }

    async fn fuzz_edge_cases(&mut self) -> io::Result<()> {
        info!("Fuzzing edge cases");

        // Test empty payload
        let _ = self.send_fuzz_payload(&[]).await;

        // Test single bytes
        for byte in 0..=255u8 {
            let _ = self.send_fuzz_payload(&[byte]).await;
        }

        // Test maximum size payloads
        let max_payload = vec![0xAA; self.max_payload_size];
        let _ = self.send_fuzz_payload(&max_payload).await;

        // Test null bytes
        let null_payload = vec![0x00; 1024];
        let _ = self.send_fuzz_payload(&null_payload).await;

        // Test alternating patterns
        let alternating: Vec<u8> = (0..1024).map(|i| if i % 2 == 0 { 0xFF } else { 0x00 }).collect();
        let _ = self.send_fuzz_payload(&alternating).await;

        Ok(())
    }

    async fn fuzz_timing_attacks(&mut self) -> io::Result<()> {
        info!("Fuzzing timing attacks");

        // Send payloads with delays between bytes
        for protocol in &self.protocols_to_fuzz.clone() {
            let payload = self.generate_protocol_payload(*protocol, &mut thread_rng());
            
            if let Ok(mut stream) = TcpStream::connect((&self.target_host[..], self.target_port)).await {
                for byte in payload {
                    let _ = stream.write_u8(byte).await;
                    sleep(Duration::from_millis(10)).await;
                }
            }
        }

        Ok(())
    }

    async fn fuzz_protocol_confusion(&mut self) -> io::Result<()> {
        info!("Fuzzing protocol confusion attacks");
        let mut rng = thread_rng();

        for _ in 0..100 {
            // Combine multiple protocol headers
            let mut payload = Vec::new();
            
            // Add HTTP header
            payload.extend_from_slice(b"GET / HTTP/1.1\r\n");
            
            // Add SOCKS5 handshake
            payload.extend_from_slice(&[0x05, 0x01, 0x00]);
            
            // Add TLS handshake
            payload.extend_from_slice(&[0x16, 0x03, 0x03, 0x00, 0x00]);
            
            // Add SSH banner
            payload.extend_from_slice(b"SSH-2.0-OpenSSH\r\n");
            
            // Add random data
            let mut random_data = vec![0u8; rng.gen_range(50..200)];
            rng.fill_bytes(&mut random_data);
            payload.extend_from_slice(&random_data);

            let _ = self.send_fuzz_payload(&payload).await;
        }

        Ok(())
    }

    fn generate_protocol_payload(&self, protocol: ProtocolType, rng: &mut ThreadRng) -> Vec<u8> {
        match protocol {
            ProtocolType::Http => {
                let methods = ["GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", "CONNECT"];
                let method = methods[rng.gen_range(0..methods.len())];
                let path = format!("/{}", rng.gen::<u32>());
                format!("{} {} HTTP/1.1\r\nHost: example.com\r\n\r\n", method, path).into_bytes()
            }
            ProtocolType::Socks5 => {
                let nmethods = rng.gen_range(1..=5);
                let mut payload = vec![0x05, nmethods];
                for _ in 0..nmethods {
                    payload.push(rng.gen_range(0..=2));
                }
                payload
            }
            ProtocolType::Tls => {
                let mut payload = vec![0x16, 0x03, 0x03]; // TLS handshake, version
                let length = rng.gen_range(100..500);
                payload.extend_from_slice(&(length as u16).to_be_bytes());
                let mut random_data = vec![0u8; length];
                rng.fill_bytes(&mut random_data);
                payload.extend_from_slice(&random_data);
                payload
            }
            ProtocolType::Ssh => {
                format!("SSH-2.0-{}\r\n", rng.gen::<u32>()).into_bytes()
            }
            ProtocolType::Websocket => {
                format!(
                    "GET / HTTP/1.1\r\n\
                     Upgrade: websocket\r\n\
                     Connection: Upgrade\r\n\
                     Sec-WebSocket-Key: {}\r\n\r\n",
                    base64::encode(&rng.gen::<[u8; 16]>())
                ).into_bytes()
            }
            ProtocolType::Mqtt => {
                vec![0x10, 0x0A, 0x00, 0x04, b'M', b'Q', b'T', b'T', 0x04, 0x00]
            }
            ProtocolType::Quic => {
                let mut payload = vec![0x80]; // Long header
                payload.extend_from_slice(&0x00000001u32.to_be_bytes()); // QUIC v1
                let mut random_data = vec![0u8; rng.gen_range(50..200)];
                rng.fill_bytes(&mut random_data);
                payload.extend_from_slice(&random_data);
                payload
            }
            _ => {
                let size = rng.gen_range(10..200);
                let mut payload = vec![0u8; size];
                rng.fill_bytes(&mut payload);
                payload
            }
        }
    }

    fn inject_protocol_confusion(&self, payload: &mut [u8], rng: &mut ThreadRng) {
        if payload.len() < 10 {
            return;
        }

        let patterns = [
            b"HTTP/1.1",
            &[0x05, 0x01, 0x00], // SOCKS5
            &[0x16, 0x03, 0x03], // TLS
            b"SSH-2.0",
            b"GET ",
            b"POST ",
            &[0x80, 0x00, 0x00, 0x00, 0x01], // QUIC
        ];

        for _ in 0..rng.gen_range(1..4) {
            let pattern = patterns[rng.gen_range(0..patterns.len())];
            let pos = rng.gen_range(0..payload.len().saturating_sub(pattern.len()));
            
            if pos + pattern.len() <= payload.len() {
                payload[pos..pos + pattern.len()].copy_from_slice(pattern);
            }
        }
    }

    async fn send_fuzz_payload(&self, payload: &[u8]) -> io::Result<ProtocolDetectionResult> {
        let stream = timeout(
            Duration::from_secs(5),
            TcpStream::connect((&self.target_host[..], self.target_port))
        ).await??;

        let mut universal_conn = UniversalConnection::new(stream);
        let proxy = UniversalProxy::new();

        // Write payload
        universal_conn.write_all(payload).await?;

        // Try to detect protocol
        let detectors = vec![];
        universal_conn.detect_protocol(&detectors).await
    }

    fn print_results(&self) {
        info!("=== FUZZING RESULTS ===");
        
        for (protocol, result) in &self.results {
            info!("Protocol: {}", protocol);
            info!("  Attempts: {}", result.attempts);
            info!("  Successful detections: {} ({:.2}%)", 
                  result.successful_detections,
                  result.successful_detections as f64 / result.attempts as f64 * 100.0);
            info!("  False positives: {} ({:.2}%)", 
                  result.false_positives,
                  result.false_positives as f64 / result.attempts as f64 * 100.0);
            info!("  Crashes: {}", result.crashes);
            info!("  Timeouts: {}", result.timeouts);
            info!("  Avg response time: {:?}", result.avg_response_time);
            info!("  Max response time: {:?}", result.max_response_time);
            info!("  Min response time: {:?}", result.min_response_time);
            
            if !result.detection_confidence_scores.is_empty() {
                let avg_confidence: f64 = result.detection_confidence_scores.iter()
                    .map(|&x| x as f64).sum::<f64>() / result.detection_confidence_scores.len() as f64;
                info!("  Avg confidence score: {:.2}", avg_confidence);
            }
            
            info!("  Payload sizes tested: {} unique sizes", 
                  result.payload_sizes_tested.iter().collect::<std::collections::HashSet<_>>().len());
            info!("");
        }
        
        // Summary statistics
        let total_attempts: usize = self.results.values().map(|r| r.attempts).sum();
        let total_crashes: usize = self.results.values().map(|r| r.crashes).sum();
        let total_timeouts: usize = self.results.values().map(|r| r.timeouts).sum();
        
        info!("=== SUMMARY ===");
        info!("Total attempts: {}", total_attempts);
        info!("Total crashes: {} ({:.2}%)", total_crashes, 
              total_crashes as f64 / total_attempts as f64 * 100.0);
        info!("Total timeouts: {} ({:.2}%)", total_timeouts,
              total_timeouts as f64 / total_attempts as f64 * 100.0);
    }
}

pub struct StressTester {
    target_host: String,
    target_port: u16,
    concurrent_connections: usize,
    duration: Duration,
}

impl StressTester {
    pub fn new(target_host: String, target_port: u16) -> Self {
        Self {
            target_host,
            target_port,
            concurrent_connections: 100,
            duration: Duration::from_secs(60),
        }
    }

    pub async fn run_stress_test(&self) -> io::Result<()> {
        info!("Starting stress test with {} concurrent connections for {:?}",
              self.concurrent_connections, self.duration);

        let mut handles = Vec::new();
        let start_time = Instant::now();

        for i in 0..self.concurrent_connections {
            let host = self.target_host.clone();
            let port = self.target_port;
            let duration = self.duration;
            
            let handle = tokio::spawn(async move {
                let mut successful_connections = 0;
                let mut failed_connections = 0;
                
                while start_time.elapsed() < duration {
                    match TcpStream::connect((&host[..], port)).await {
                        Ok(mut stream) => {
                            successful_connections += 1;
                            // Send some data
                            let _ = stream.write_all(b"GET / HTTP/1.1\r\n\r\n").await;
                            sleep(Duration::from_millis(100)).await;
                        }
                        Err(_) => {
                            failed_connections += 1;
                        }
                    }
                    
                    sleep(Duration::from_millis(10)).await;
                }
                
                (successful_connections, failed_connections)
            });
            
            handles.push(handle);
        }

        let mut total_successful = 0;
        let mut total_failed = 0;

        for handle in handles {
            if let Ok((successful, failed)) = handle.await {
                total_successful += successful;
                total_failed += failed;
            }
        }

        info!("Stress test completed:");
        info!("  Successful connections: {}", total_successful);
        info!("  Failed connections: {}", total_failed);
        info!("  Success rate: {:.2}%", 
              total_successful as f64 / (total_successful + total_failed) as f64 * 100.0);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzer_creation() {
        let fuzzer = ProtocolFuzzer::new("127.0.0.1".to_string(), 8080);
        assert_eq!(fuzzer.target_host, "127.0.0.1");
        assert_eq!(fuzzer.target_port, 8080);
        assert_eq!(fuzzer.iterations, 1000);
    }

    #[test]
    fn test_payload_generation() {
        let fuzzer = ProtocolFuzzer::new("127.0.0.1".to_string(), 8080);
        let mut rng = thread_rng();
        
        let http_payload = fuzzer.generate_protocol_payload(ProtocolType::Http, &mut rng);
        assert!(String::from_utf8_lossy(&http_payload).contains("HTTP/1.1"));
        
        let socks5_payload = fuzzer.generate_protocol_payload(ProtocolType::Socks5, &mut rng);
        assert_eq!(socks5_payload[0], 0x05);
    }

    #[test]
    fn test_protocol_confusion_injection() {
        let fuzzer = ProtocolFuzzer::new("127.0.0.1".to_string(), 8080);
        let mut payload = vec![0x00; 100];
        let mut rng = thread_rng();
        
        fuzzer.inject_protocol_confusion(&mut payload, &mut rng);
        
        // Should contain some recognizable protocol patterns
        let payload_str = String::from_utf8_lossy(&payload);
        let has_patterns = payload_str.contains("HTTP") || 
                          payload.windows(3).any(|w| w == [0x05, 0x01, 0x00]) ||
                          payload.windows(3).any(|w| w == [0x16, 0x03, 0x03]);
        assert!(has_patterns);
    }
}