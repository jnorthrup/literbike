# Qwen Agent: QUIC Implementation Team

## Assignment
Complete QUIC protocol implementation for production use.

## Branches
- `quic/p0-tls-handshake` - TLS 1.3 with rustls
- `quic/p0-ack-management` - ACK frames, retransmission
- `quic/p0-congestion-control` - CUBIC/BBR algorithm
- `quic/p0-loss-recovery` - PTO, FACK, packet spaces
- `quic/p0-integration-tests` - Client-server tests

## Priority
**P0 - Critical** (Blocks Kafka replacement deployment)

---

## Task 1: TLS 1.3 Handshake

**Branch:** `quic/p0-tls-handshake`

**Current state:**
- Dummy connection IDs
- No encryption
- No certificate handling

**Implementation:**
```rust
// Add to Cargo.toml
rustls = "0.21"
rustls-pemfile = "1.0"

// In src/quic/quic_engine.rs
use rustls::{ClientConfig, ServerConfig};

// Implement TLS handshake
async fn tls_handshake(&mut self) -> Result<(), QuicError> {
    // 1. ClientHello
    // 2. ServerHello + Certificate
    // 3. Client Key Exchange
    // 4. Finished
}
```

**Test:**
```bash
cargo test --features quic quic_tls
```

---

## Task 2: ACK Management

**Branch:** `quic/p0-ack-management`

**Current state:**
- ACK frames generated but not tracked
- No retransmission logic

**Implementation:**
```rust
// In src/quic/quic_engine.rs
struct AckManager {
    ack_ranges: Vec<AckRange>,
    largest_acked: u64,
    ack_delay: Duration,
}

impl AckManager {
    fn on_packet_received(&mut self, packet_number: u64);
    fn generate_ack_frame(&self) -> AckFrame;
    fn needs_retransmit(&self, packet_number: u64) -> bool;
}
```

**Test:**
```bash
cargo test --features quic quic_ack
```

---

## Task 3: Congestion Control

**Branch:** `quic/p0-congestion-control`

**Current state:**
- No flow control enforcement
- No congestion window

**Implementation:**
```rust
// In src/quic/quic_engine.rs
struct CongestionController {
    cwnd: u64,  // Congestion window
    ssthresh: u64,
    bytes_in_flight: u64,
    algorithm: CcAlgorithm,  // Cubic or BBR
}

impl CongestionController {
    fn on_ack(&mut self, bytes_acked: u64);
    fn on_loss(&mut self, bytes_lost: u64);
    fn can_send(&self) -> bool;
}
```

**Test:**
```bash
cargo test --features quic quic_congestion
```

---

## Task 4: Loss Recovery

**Branch:** `quic/p0-loss-recovery`

**Current state:**
- No retransmission
- No packet number spaces

**Implementation:**
```rust
// In src/quic/quic_engine.rs
struct LossRecovery {
    sent_packets: HashMap<u64, SentPacket>,
    pto: Duration,  // Probe Timeout
    time_threshold: f64,
}

impl LossRecovery {
    fn on_packet_sent(&mut self, packet: SentPacket);
    fn on_ack_received(&mut self, acked: Vec<u64>);
    fn detect_loss(&mut self) -> Vec<u64>;
    fn get_pto(&self) -> Duration;
}
```

**Test:**
```bash
cargo test --features quic quic_loss
```

---

## Task 5: Integration Tests

**Branch:** `quic/p0-integration-tests`

**Test scenarios:**
1. Client-server handshake
2. Stream I/O (send/recv)
3. Packet loss simulation
4. Congestion control under load
5. Multiple streams multiplexing

**Implementation:**
```rust
// In tests/quic_integration.rs
#[tokio::test]
async fn test_quic_handshake() {
    let server = QuicServer::bind("127.0.0.1:0".parse().unwrap()).await?;
    let client = QuicClient::connect(server.local_addr()).await?;
    assert!(client.is_connected());
}

#[tokio::test]
async fn test_quic_stream_io() {
    // Create connection
    // Open stream
    // Send data
    // Receive data
    // Verify
}
```

---

## Success Criteria

- [ ] TLS 1.3 handshake works with real certificates
- [ ] ACK frames properly tracked and generated
- [ ] Congestion control prevents network saturation
- [ ] Lost packets are retransmitted
- [ ] Integration tests pass
- [ ] Can connect to external QUIC servers (optional)

---

## Merge Order

1. `quic/p0-tls-handshake` → master
2. `quic/p0-ack-management` → master (depends on TLS)
3. `quic/p0-congestion-control` → master (depends on ACK)
4. `quic/p0-loss-recovery` → master (depends on ACK)
5. `quic/p0-integration-tests` → master (depends on all above)

---

## Dependencies

- rustls crate for TLS
- ring crate for crypto primitives
- Cleanup branches must merge first (type renames)

---

**Created:** 2026-02-24  
**Status:** Ready to start (after cleanup merges)
