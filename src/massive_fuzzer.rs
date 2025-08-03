use std::time::{Duration, Instant};
use log::{debug, info, warn, error};
use tokio::io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};
use crate::types::{ProtocolType, ProtocolDetectionResult};
use crate::abstractions::ProtocolHandler;
use crate::stubs::*;

pub struct MassiveFuzzer {
    mutation_strategies: Vec<MutationStrategy>,
    test_cases: Vec<TestCase>,
    stats: FuzzingStats,
}

#[derive(Clone)]
pub struct TestCase {
    pub name: String,
    pub protocol: ProtocolType,
    pub payload: Vec<u8>,
    pub expected_behavior: ExpectedBehavior,
}

#[derive(Clone, Debug)]
pub enum ExpectedBehavior {
    Accept,
    Reject,
    Timeout,
    Error,
    Crash,
    Any,
}

#[derive(Clone)]
pub enum MutationStrategy {
    BitFlip { positions: usize },
    ByteFlip { positions: usize },
    ByteIncrement,
    ByteDecrement,
    ByteInsert { positions: Vec<usize> },
    ByteDelete { positions: Vec<usize> },
    ByteSwap { pairs: usize },
    BlockShuffle { block_size: usize },
    LengthExtension { extra_bytes: usize },
    LengthTruncation { remove_bytes: usize },
    NullInjection,
    OverflowInjection,
    UnderflowInjection,
    FormatStringInjection,
    SQLInjection,
    XSSInjection,
    PathTraversalInjection,
    BufferOverflow { target_size: usize },
    IntegerOverflow,
    UnicodeCorruption,
    CompressionBomb,
    ProtocolConfusion,
    TimingAttack,
    ReplayAttack,
    DowngradeAttack,
}

#[derive(Default)]
pub struct FuzzingStats {
    pub total_tests: u64,
    pub crashes: u64,
    pub hangs: u64,
    pub errors: u64,
    pub timeouts: u64,
    pub accepted: u64,
    pub rejected: u64,
    pub unique_crashes: HashMap<String, u64>,
    pub test_duration: Duration,
    pub throughput: f64,
}

impl MassiveFuzzer {
    pub fn new() -> Self {
        Self {
            mutation_strategies: Self::create_mutation_strategies(),
            test_cases: Self::create_base_test_cases(),
            stats: FuzzingStats::default(),
        }
    }

    // Static protocol handler registry as an array of (ProtocolType, constructor)
    pub const PROTOCOL_HANDLERS: &'static [(ProtocolType, fn() -> Box<dyn ProtocolHandler + Send + Sync>)] = &[
        (ProtocolType::Shadowsocks, || Box::new(ShadowsocksHandler::new())),
        (ProtocolType::Https, || Box::new(HttpsSpoofingHandler)),
        (ProtocolType::WebRtc, || Box::new(WebRtcHandler)),
        (ProtocolType::Quic, || Box::new(QuicHandler)),
        (ProtocolType::Ssh, || Box::new(SshHandler)),
        (ProtocolType::Ftp, || Box::new(FtpHandler)),
        (ProtocolType::Smtp, || Box::new(SmtpHandler)),
        (ProtocolType::Irc, || Box::new(IrcHandler)),
        (ProtocolType::Websocket, || Box::new(WebSocketHandler)),
        (ProtocolType::Mqtt, || Box::new(MqttHandler)),
        (ProtocolType::Sip, || Box::new(SipHandler)),
        (ProtocolType::Rtsp, || Box::new(RtspHandler)),
    ];

    // Example: static protocol subset
    pub const FUZZ_PROTOCOLS: &'static [ProtocolType] = &[
        ProtocolType::Shadowsocks,
        ProtocolType::Https,
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
    ];

    fn create_mutation_strategies() -> Vec<MutationStrategy> {
        vec![
            MutationStrategy::BitFlip { positions: 1 },
            MutationStrategy::BitFlip { positions: 8 },
            MutationStrategy::BitFlip { positions: 16 },
            MutationStrategy::ByteFlip { positions: 1 },
            MutationStrategy::ByteFlip { positions: 4 },
            MutationStrategy::ByteIncrement,
            MutationStrategy::ByteDecrement,
            MutationStrategy::ByteInsert { positions: vec![0, 1, 2, 4, 8, 16] },
            MutationStrategy::ByteDelete { positions: vec![0, 1, 2, 4, 8, 16] },
            MutationStrategy::ByteSwap { pairs: 2 },
            MutationStrategy::ByteSwap { pairs: 4 },
            MutationStrategy::BlockShuffle { block_size: 4 },
            MutationStrategy::BlockShuffle { block_size: 8 },
            MutationStrategy::LengthExtension { extra_bytes: 1024 },
            MutationStrategy::LengthExtension { extra_bytes: 65536 },
            MutationStrategy::LengthTruncation { remove_bytes: 1 },
            MutationStrategy::LengthTruncation { remove_bytes: 8 },
            MutationStrategy::NullInjection,
            MutationStrategy::OverflowInjection,
            MutationStrategy::UnderflowInjection,
            MutationStrategy::FormatStringInjection,
            MutationStrategy::SQLInjection,
            MutationStrategy::XSSInjection,
            MutationStrategy::PathTraversalInjection,
            MutationStrategy::BufferOverflow { target_size: 4096 },
            MutationStrategy::BufferOverflow { target_size: 65536 },
            MutationStrategy::IntegerOverflow,
            MutationStrategy::UnicodeCorruption,
            MutationStrategy::CompressionBomb,
            MutationStrategy::ProtocolConfusion,
            MutationStrategy::TimingAttack,
            MutationStrategy::ReplayAttack,
            MutationStrategy::DowngradeAttack,
        ]
    }

    fn create_base_test_cases() -> Vec<TestCase> {
        vec![
            // HTTP Test Cases
            TestCase {
                name: "HTTP GET".to_string(),
                protocol: ProtocolType::Http,
                payload: b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec(),
                expected_behavior: ExpectedBehavior::Accept,
            },
            TestCase {
                name: "HTTP POST".to_string(),
                protocol: ProtocolType::Http,
                payload: b"POST /api HTTP/1.1\r\nContent-Length: 0\r\n\r\n".to_vec(),
                expected_behavior: ExpectedBehavior::Accept,
            },
            
            // SOCKS5 Test Cases
            TestCase {
                name: "SOCKS5 Handshake".to_string(),
                protocol: ProtocolType::Socks5,
                payload: vec![0x05, 0x01, 0x00],
                expected_behavior: ExpectedBehavior::Accept,
            },
            TestCase {
                name: "SOCKS5 Connect".to_string(),
                protocol: ProtocolType::Socks5,
                payload: vec![0x05, 0x01, 0x00, 0x01, 127, 0, 0, 1, 0x00, 0x50],
                expected_behavior: ExpectedBehavior::Accept,
            },
            
            // TLS Test Cases
            TestCase {
                name: "TLS 1.2 Client Hello".to_string(),
                protocol: ProtocolType::Tls,
                payload: vec![0x16, 0x03, 0x03, 0x00, 0x10, 0x01, 0x00, 0x00, 0x0c, 0x03, 0x03],
                expected_behavior: ExpectedBehavior::Accept,
            },
            TestCase {
                name: "TLS 1.3 Client Hello".to_string(),
                protocol: ProtocolType::Tls,
                payload: vec![0x16, 0x03, 0x01, 0x00, 0x10, 0x01, 0x00, 0x00, 0x0c, 0x03, 0x04],
                expected_behavior: ExpectedBehavior::Accept,
            },
            
            // Shadowsocks Test Cases
            TestCase {
                name: "Shadowsocks IPv4".to_string(),
                protocol: ProtocolType::Shadowsocks,
                payload: vec![0x01, 127, 0, 0, 1, 0x00, 0x50],
                expected_behavior: ExpectedBehavior::Accept,
            },
            TestCase {
                name: "Shadowsocks Domain".to_string(),
                protocol: ProtocolType::Shadowsocks,
                payload: {
                    let mut payload = vec![0x03, 0x0b];
                    payload.extend(b"example.com");
                    payload.extend(&[0x00, 0x50]);
                    payload
                },
                expected_behavior: ExpectedBehavior::Accept,
            },
            
            // WebSocket Test Cases
            TestCase {
                name: "WebSocket Upgrade".to_string(),
                protocol: ProtocolType::Websocket,
                payload: b"GET /chat HTTP/1.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n".to_vec(),
                expected_behavior: ExpectedBehavior::Accept,
            },
            
            // SSH Test Cases
            TestCase {
                name: "SSH Version Exchange".to_string(),
                protocol: ProtocolType::Ssh,
                payload: b"SSH-2.0-OpenSSH_8.0\r\n".to_vec(),
                expected_behavior: ExpectedBehavior::Accept,
            },
            
            // Malformed Test Cases
            TestCase {
                name: "Empty Payload".to_string(),
                protocol: ProtocolType::Http,
                payload: vec![],
                expected_behavior: ExpectedBehavior::Reject,
            },
            TestCase {
                name: "Random Garbage".to_string(),
                protocol: ProtocolType::Http,
                payload: vec![0xFF, 0xFE, 0xFD, 0xFC, 0xFB],
                expected_behavior: ExpectedBehavior::Reject,
            },
            TestCase {
                name: "Null Bytes".to_string(),
                protocol: ProtocolType::Http,
                payload: vec![0x00, 0x00, 0x00, 0x00],
                expected_behavior: ExpectedBehavior::Reject,
            },
        ]
    }

    pub async fn run_comprehensive_fuzzing(&mut self, duration: Duration) -> &FuzzingStats {
        info!("Starting comprehensive fuzzing for {:?}", duration);
        let start_time = Instant::now();
        let end_time = start_time + duration;

        while Instant::now() < end_time {
            for test_case in self.test_cases.clone() {
                if Instant::now() >= end_time {
                    break;
                }
                
                // Test original case
                self.test_single_case(&test_case).await;
                
                // Apply mutations
                for strategy in &self.mutation_strategies.clone() {
                    if Instant::now() >= end_time {
                        break;
                    }
                    
                    let mutated_case = self.apply_mutation(&test_case, strategy);
                    self.test_single_case(&mutated_case).await;
                }
            }
            
            // Generate completely random test cases
            for _ in 0..100 {
                if Instant::now() >= end_time {
                    break;
                }
                
                let random_case = self.generate_random_test_case();
                self.test_single_case(&random_case).await;
            }
        }

        self.stats.test_duration = start_time.elapsed();
        self.stats.throughput = self.stats.total_tests as f64 / self.stats.test_duration.as_secs_f64();
        
        info!("Fuzzing completed. Total tests: {}, Crashes: {}, Hangs: {}", 
              self.stats.total_tests, self.stats.crashes, self.stats.hangs);
        
        &self.stats
    }

    async fn test_single_case(&mut self, test_case: &TestCase) {
        self.stats.total_tests += 1;
        
        let handler = match Self::PROTOCOL_HANDLERS.iter().find(|(pt,_)| pt == &test_case.protocol) {
            Some((_, ctor)) => ctor(),
            None => {
                warn!("No handler for protocol {:?}", test_case.protocol);
                return;
            }
        };

        // Create mock stream
        let mut mock_stream = MockStream::new(test_case.payload.clone());
        let detection = ProtocolDetectionResult {
            protocol: test_case.protocol,
            confidence: 1.0,
            bytes_consumed: test_case.payload.len(),
            metadata: None,
        };

        let test_start = Instant::now();
        let timeout = Duration::from_millis(1000);

        let result = tokio::time::timeout(timeout, handler.handle(&mut mock_stream, detection)).await;

        match result {
            Ok(Ok(())) => {
                self.stats.accepted += 1;
                debug!("Test '{}' completed successfully", test_case.name);
            }
            Ok(Err(e)) => {
                self.stats.errors += 1;
                debug!("Test '{}' returned error: {}", test_case.name, e);
            }
            Err(_) => {
                self.stats.timeouts += 1;
                warn!("Test '{}' timed out", test_case.name);
            }
        }

        if test_start.elapsed() > Duration::from_millis(500) {
            self.stats.hangs += 1;
            warn!("Test '{}' took too long: {:?}", test_case.name, test_start.elapsed());
        }
    }

    fn apply_mutation(&self, test_case: &TestCase, strategy: &MutationStrategy) -> TestCase {
        let mut mutated = test_case.clone();
        mutated.name = format!("{}_mutated_{:?}", test_case.name, strategy);
        mutated.expected_behavior = ExpectedBehavior::Any;

        match strategy {
            MutationStrategy::BitFlip { positions } => {
                for _ in 0..*positions {
                    if !mutated.payload.is_empty() {
                        let byte_idx = fast_random() % mutated.payload.len();
                        let bit_idx = fast_random() % 8;
                        mutated.payload[byte_idx] ^= 1 << bit_idx;
                    }
                }
            }
            
            MutationStrategy::ByteFlip { positions } => {
                for _ in 0..*positions {
                    if !mutated.payload.is_empty() {
                        let idx = fast_random() % mutated.payload.len();
                        mutated.payload[idx] = !mutated.payload[idx];
                    }
                }
            }
            
            MutationStrategy::ByteIncrement => {
                if !mutated.payload.is_empty() {
                    let idx = fast_random() % mutated.payload.len();
                    mutated.payload[idx] = mutated.payload[idx].wrapping_add(1);
                }
            }
            
            MutationStrategy::ByteDecrement => {
                if !mutated.payload.is_empty() {
                    let idx = fast_random() % mutated.payload.len();
                    mutated.payload[idx] = mutated.payload[idx].wrapping_sub(1);
                }
            }
            
            MutationStrategy::ByteInsert { positions } => {
                for &pos in positions {
                    if pos < mutated.payload.len() {
                        let random_byte = (fast_random() % 256) as u8;
                        mutated.payload.insert(pos, random_byte);
                    }
                }
            }
            
            MutationStrategy::ByteDelete { positions } => {
                for &pos in positions.iter().rev() {
                    if pos < mutated.payload.len() {
                        mutated.payload.remove(pos);
                    }
                }
            }
            
            MutationStrategy::LengthExtension { extra_bytes } => {
                let extension: Vec<u8> = (0..*extra_bytes).map(|_| (fast_random() % 256) as u8).collect();
                mutated.payload.extend(extension);
            }
            
            MutationStrategy::LengthTruncation { remove_bytes } => {
                let new_len = mutated.payload.len().saturating_sub(*remove_bytes);
                mutated.payload.truncate(new_len);
            }
            
            MutationStrategy::NullInjection => {
                if !mutated.payload.is_empty() {
                    let idx = fast_random() % mutated.payload.len();
                    mutated.payload[idx] = 0x00;
                }
            }
            
            MutationStrategy::OverflowInjection => {
                if !mutated.payload.is_empty() {
                    let idx = fast_random() % mutated.payload.len();
                    mutated.payload[idx] = 0xFF;
                }
            }
            
            MutationStrategy::FormatStringInjection => {
                let format_strings = [b"%s", b"%x", b"%n", b"%p"];
                let chosen = format_strings[fast_random() % format_strings.len()];
                mutated.payload.extend_from_slice(chosen);
            }
            
            MutationStrategy::BufferOverflow { target_size } => {
                let overflow_data: Vec<u8> = (0..*target_size).map(|i| (i % 256) as u8).collect();
                mutated.payload = overflow_data;
            }
            
            MutationStrategy::IntegerOverflow => {
                let overflow_values = [0xFFFFFFFF, 0x80000000, 0x7FFFFFFF];
                let chosen = overflow_values[fast_random() % overflow_values.len()];
                let bytes = chosen.to_le_bytes();
                mutated.payload.extend_from_slice(&bytes);
            }
            
            MutationStrategy::ProtocolConfusion => {
                // Mix protocols together
                let other_protocols = [
                    b"GET / HTTP/1.1\r\n".to_vec(),
                    vec![0x05, 0x01, 0x00],
                    vec![0x16, 0x03, 0x03],
                ];
                let chosen = &other_protocols[fast_random() % other_protocols.len()];
                mutated.payload.extend_from_slice(chosen);
            }
            
            _ => {
                // For strategies not implemented, just flip random bits
                if !mutated.payload.is_empty() {
                    let idx = fast_random() % mutated.payload.len();
                    mutated.payload[idx] ^= 0xFF;
                }
            }
        }

        mutated
    }

    fn generate_random_test_case(&self) -> TestCase {
        let protocols = [
            ProtocolType::Http,
            ProtocolType::Socks5,
            ProtocolType::Tls,
            ProtocolType::Shadowsocks,
            ProtocolType::WebRtc,
            ProtocolType::Quic,
            ProtocolType::Ssh,
            ProtocolType::Websocket,
        ];
        
        let protocol = protocols[fast_random() % protocols.len()];
        let payload_len = 1 + (fast_random() % 4096);
        let payload: Vec<u8> = (0..payload_len).map(|_| (fast_random() % 256) as u8).collect();

        TestCase {
            name: format!("Random_{:x}", fast_random()),
            protocol,
            payload,
            expected_behavior: ExpectedBehavior::Any,
        }
    }

    pub fn print_stats(&self) {
        println!("=== MASSIVE FUZZING RESULTS ===");
        println!("Duration: {:?}", self.stats.test_duration);
        println!("Total Tests: {}", self.stats.total_tests);
        println!("Throughput: {:.2} tests/sec", self.stats.throughput);
        println!("Accepted: {} ({:.2}%)", self.stats.accepted, 
                 (self.stats.accepted as f64 / self.stats.total_tests as f64) * 100.0);
        println!("Rejected: {} ({:.2}%)", self.stats.rejected,
                 (self.stats.rejected as f64 / self.stats.total_tests as f64) * 100.0);
        println!("Errors: {} ({:.2}%)", self.stats.errors,
                 (self.stats.errors as f64 / self.stats.total_tests as f64) * 100.0);
        println!("Timeouts: {} ({:.2}%)", self.stats.timeouts,
                 (self.stats.timeouts as f64 / self.stats.total_tests as f64) * 100.0);
        println!("Hangs: {} ({:.2}%)", self.stats.hangs,
                 (self.stats.hangs as f64 / self.stats.total_tests as f64) * 100.0);
        println!("Crashes: {} ({:.2}%)", self.stats.crashes,
                 (self.stats.crashes as f64 / self.stats.total_tests as f64) * 100.0);
        
        if !self.stats.unique_crashes.is_empty() {
            println!("\nUnique Crashes:");
            for (crash_type, count) in &self.stats.unique_crashes {
                println!("  {}: {}", crash_type, count);
            }
        }
    }
}

// Mock stream for testing
pub struct MockStream {
    data: Vec<u8>,
    position: usize,
    write_buffer: Vec<u8>,
}

impl MockStream {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            position: 0,
            write_buffer: Vec::new(),
        }
    }
}

impl AsyncRead for MockStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let available = self.data.len().saturating_sub(self.position);
        let to_read = std::cmp::min(available, buf.remaining());
        
        if to_read > 0 {
            buf.put_slice(&self.data[self.position..self.position + to_read]);
            self.position += to_read;
        }
        
        std::task::Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for MockStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        self.write_buffer.extend_from_slice(buf);
        std::task::Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }
}

// Fast pseudo-random number generator
fn fast_random() -> usize {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static SEED: AtomicUsize = AtomicUsize::new(0xdeadbeef);
    
    let current = SEED.load(Ordering::Relaxed);
    let next = current.wrapping_mul(1103515245).wrapping_add(12345);
    SEED.store(next, Ordering::Relaxed);
    next
}