# Hybrid Architecture: Atomics + Content Engineering

**Status:** Production-Ready Design  
**Principle:** Atomics for hot path, content-addressed for durability/recovery

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    HOT PATH (< 500 ns)                           │
│  AtomicU64 counters, CAS state, lock-free queues                │
├─────────────────────────────────────────────────────────────────┤
│  QuicEngine::send_stream_data()                                 │
│    → pkt_num = atomic_seq.fetch_add(1, AcqRel)  // 2 ns         │
│    → flow_control.fetch_sub(len, AcqRel)        // 2 ns         │
│    → send(packet)                               // network       │
│                                                                  │
│  QuicEngine::process_packet()                                   │
│    → ack_bitmap.fetch_or(mask, AcqRel)          // 2 ns         │
│    → bytes_received.fetch_add(len, Relaxed)     // 2 ns         │
│    → deliver(payload)                           // app           │
└─────────────────────────────────────────────────────────────────┘
                            │
                            │ async channel (lock-free MPSC)
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                 WARM PATH (1-10 µs background)                   │
│  Content-addressed logging, dedup, Merkle batching              │
├─────────────────────────────────────────────────────────────────┤
│  ContentLogger::log_packet(pkt_num, &data)                      │
│    → hash = sha256(data)                        // 150 ns       │
│    → tx.send((pkt_num, hash, data))             // MPSC         │
│                                                                  │
│  Background flush thread (100 µs batch)                         │
│    → build_merkle_tree(batch)                   // 5 µs         │
│    → duckdb.insert_batch(...)                   // 50 µs        │
│    → publish_root(atomic_ptr.store(root))       // 2 ns         │
└─────────────────────────────────────────────────────────────────┘
```

---

## Component Implementation

### 1. QUIC Engine (Hot Path: Atomics)

```rust
// src/quic/quic_engine_hybrid.rs

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

pub struct QuicEngineHybrid {
    // HOT PATH: Atomic counters (keep these!)
    packet_sequence: AtomicU64,
    ack_bitmap: AtomicU128,  // Last 128 packets
    bytes_in_flight: AtomicUsize,
    connection_state: AtomicU32,
    
    // WARM PATH: Content logger (async, background)
    content_logger: ContentLogger,
}

impl QuicEngineHybrid {
    pub fn send_stream_data(&self, stream_id: u64, data: &[u8]) -> QuicPacket {
        // HOT PATH: Atomic sequence (2 ns)
        let pkt_num = self.packet_sequence.fetch_add(1, Ordering::AcqRel);
        
        // HOT PATH: Flow control (2 ns)
        self.bytes_in_flight.fetch_add(data.len(), Ordering::AcqRel);
        
        // Build packet
        let packet = QuicPacket {
            packet_number: pkt_num,
            data: data.to_vec(),
            ..
        };
        
        // WARM PATH: Async content log (non-blocking MPSC send)
        self.content_logger.log_packet(pkt_num, data);
        
        packet
    }
    
    pub fn process_ack(&self, ack_num: u64) {
        // HOT PATH: Update ACK bitmap (2 ns)
        let bit = 1u128 << (ack_num % 128);
        self.ack_bitmap.fetch_or(bit, Ordering::AcqRel);
        
        // HOT PATH: Update flow control (2 ns)
        let freed = estimate_freed_bytes(ack_num);
        self.bytes_in_flight.fetch_sub(freed, Ordering::Relaxed);
    }
    
    // Recovery: Read from content-addressed log
    pub fn recover_from_crash(&self, db_path: &str) -> Result<()> {
        let store = ContentAddressedStore::new(db_path)?;
        
        // Get latest Merkle root (published via atomic)
        let root_ptr = self.content_logger.root_ptr.load(Ordering::Acquire);
        let root = unsafe { &*root_ptr };
        
        // Replay from content log
        for event in store.replay_from_root(root)? {
            self.apply_event(event);
        }
        
        Ok(())
    }
}
```

### 2. Content Logger (Warm Path: Content-Addressed)

```rust
// src/content_logger.rs

use crossbeam::channel::{bounded, Sender, Receiver};
use std::thread;

pub struct ContentLogger {
    tx: Sender<LogEntry>,
    root_ptr: AtomicPtr<MerkleRoot>,
}

struct LogEntry {
    packet_number: u64,
    content_hash: ContentHash,
    data: Vec<u8>,
}

impl ContentLogger {
    pub fn new(duckdb_path: &str) -> Self {
        let (tx, rx) = bounded(1024); // MPSC channel
        
        // Background flush thread
        let store = ContentAddressedStore::new(duckdb_path).unwrap();
        let root_ptr = AtomicPtr::new(std::ptr::null_mut());
        
        thread::spawn(move || {
            let mut batch = Vec::with_capacity(100);
            
            loop {
                // Collect batch (100 µs timeout)
                while batch.len() < 100 {
                    match rx.recv_timeout(Duration::from_micros(100)) {
                        Ok(entry) => batch.push(entry),
                        Err(_) => break,
                    }
                }
                
                if batch.is_empty() {
                    continue;
                }
                
                // Build Merkle tree for batch
                let hashes: Vec<ContentHash> = batch.iter()
                    .map(|e| e.content_hash)
                    .collect();
                let tree = MerkleNode::build_tree(&hashes);
                let root = tree.map(|t| t.root()).unwrap_or_default();
                
                // Store to DuckDB (batch insert)
                store.store_batch(&batch).unwrap();
                store.store_merkle_root(&root, batch.len()).unwrap();
                
                // Publish root atomically (2 ns)
                let root_box = Box::new(root);
                let root_ptr = Box::into_raw(root_box);
                let old_ptr = root_ptr.swap(root_ptr, Ordering::AcqRel);
                if !old_ptr.is_null() {
                    drop(unsafe { Box::from_raw(old_ptr) });
                }
                
                batch.clear();
            }
        });
        
        Self { tx, root_ptr }
    }
    
    pub fn log_packet(&self, pkt_num: u64, data: &[u8]) {
        let content_hash = Sha256::digest(data).into();
        let entry = LogEntry {
            packet_number: pkt_num,
            content_hash,
            data: data.to_vec(),
        };
        
        // Non-blocking send (drops if full - acceptable for logging)
        let _ = self.tx.try_send(entry);
    }
}
```

### 3. DuckDB Content-Addressed WAL

```sql
-- Optimized for batch inserts, not single-row UPSERT

-- Content blobs (immutable, deduplicated)
CREATE TABLE content_blobs (
    hash BLOB PRIMARY KEY,
    content BLOB NOT NULL,
    size INTEGER NOT NULL,
    first_seen TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Batch event log (append-only, ordered by batch)
CREATE TABLE event_log_batch (
    batch_id BIGINT PRIMARY KEY,
    merkle_root BLOB NOT NULL,
    packet_count INTEGER NOT NULL,
    first_pkt_num BIGINT NOT NULL,
    last_pkt_num BIGINT NOT NULL,
    timestamp BIGINT DEFAULT (strftime('%s', 'now'))
);

-- Packet index (for recovery lookup)
CREATE TABLE packet_index (
    packet_num BIGINT PRIMARY KEY,
    content_hash BLOB NOT NULL,
    batch_id BIGINT NOT NULL,
    FOREIGN KEY (batch_id) REFERENCES event_log_batch(batch_id)
);

-- Materialized view for latest state
CREATE VIEW latest_merkle_root AS
SELECT merkle_root, batch_id, last_pkt_num
FROM event_log_batch
ORDER BY batch_id DESC
LIMIT 1;
```

### 4. Kafka Replacement (Hybrid Sequencing)

```rust
// src/hybrid_kafka.rs

use std::sync::atomic::{AtomicU64, Ordering};

pub struct HybridEventLog {
    // HOT: Atomic sequence (monotonic, unique)
    sequence: AtomicU64,
    
    // WARM: Content-addressed storage (dedup, recovery)
    store: ContentAddressedStore,
}

impl HybridEventLog {
    pub fn append(&self, event_data: &[u8]) -> u64 {
        // HOT: Get monotonic sequence (2 ns)
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst);
        
        // WARM: Content-addressed storage (async, non-blocking)
        let content_hash = Sha256::digest(event_data).into();
        let blob = ContentBlob::with_hash(event_data.to_vec(), content_hash);
        
        // Idempotency check: same content = same hash = dedup
        let _ = self.store.store(&blob);
        
        // Store sequence → hash mapping
        let _ = self.store.store_ref(
            &format!("seq:{}", seq),
            "event_sequence",
            &blob,
        );
        
        seq  // Return to caller immediately
    }
    
    pub fn read_from(&self, offset: u64, limit: usize) -> Vec<Event> {
        // Read from content-addressed store
        (0..limit)
            .filter_map(|i| {
                let seq = offset + i as u64;
                self.store.retrieve_ref(&format!("seq:{}", seq)).ok().flatten()
            })
            .collect()
    }
}
```

### 5. Knox Proxy (Sharded Atomics + Content Audit)

```rust
// src/hybrid_proxy.rs

use std::sync::atomic::{AtomicUsize, Ordering};

const NUM_SHARDS: usize = 256;

pub struct HybridConnectionCounter {
    // HOT: Sharded atomics (reduce contention)
    shards: Vec<AtomicUsize>,
    
    // WARM: Content audit log (compliance)
    audit_logger: ContentLogger,
}

impl HybridConnectionCounter {
    pub fn new() -> Self {
        let shards = (0..NUM_SHARDS)
            .map(|_| AtomicUsize::new(0))
            .collect();
        
        Self {
            shards,
            audit_logger: ContentLogger::new("proxy_audit.duckdb"),
        }
    }
    
    pub fn increment(&self, connection_id: &str) -> usize {
        // HOT: Hash-based shard selection (reduces contention)
        let shard_idx = fast_hash(connection_id) % NUM_SHARDS;
        
        // HOT: Atomic increment on shard (2 ns, no cache-line ping-pong)
        let count = self.shards[shard_idx].fetch_add(1, Ordering::AcqRel);
        
        // WARM: Async audit log (non-blocking)
        self.audit_logger.log_connection(connection_id, count);
        
        count
    }
    
    pub fn total(&self) -> usize {
        // Periodic aggregation (not hot path)
        self.shards.iter()
            .map(|s| s.load(Ordering::Relaxed))
            .sum()
    }
}

fn fast_hash(s: &str) -> usize {
    // FxHash or similar (fast, not cryptographic)
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish() as usize
}
```

---

## Performance Comparison

| Operation | Pure Atomics | Pure Content | Hybrid |
|-----------|-------------|--------------|--------|
| Packet send | 2 ns | 500 ns | 2 ns + async |
| ACK process | 2 ns | 200 ns | 2 ns + async |
| Connection count | 2 ns | 100 ns | 2 ns + async |
| Crash recovery | Manual state | Full replay | Merkle proof |
| Deduplication | None | Automatic | Automatic |
| Audit trail | None | Full | Full |

---

## Migration Checklist

- [ ] **Phase 0:** Keep all existing atomics (do not remove!)
- [ ] **Phase 1:** Add background content logger (dual-write, no hot-path change)
- [ ] **Phase 2:** Implement crash recovery from content log
- [ ] **Phase 3:** Add Merkle proofs for cross-node verification
- [ ] **Phase 4:** Optimize batch sizes, flush intervals

---

## When NOT to Use Content Engineering

❌ Packet sequence numbers (must be monotonic)  
❌ Flow control windows (must be O(1) arithmetic)  
❌ ACK bitmaps (must be bitwise ops)  
❌ Connection counters (must be sharded atomics)  
❌ State machine transitions (must be CAS)  

## When TO Use Content Engineering

✅ Durable WAL / crash recovery  
✅ Cross-node consistency proofs  
✅ Retransmission deduplication cache  
✅ Audit/compliance logging  
✅ Historical event replay  

---

**Bottom Line:** Atomics for speed, content for durability. Hybrid wins.
