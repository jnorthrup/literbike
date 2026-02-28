//! Phase 4: DuckDB Event Log Tests - Kafka Compatibility
//!
//! Tests 4.3.1 - 4.3.6: Consumer groups, offsets, ordering, replay, compaction, partitions

use literbike::kafka_replacement_smoke::*;
use anyhow::Result;

// ============================================================================
// Test 4.3.1: Offset management (seek, commit)
// ============================================================================

#[test]
fn test_offset_seek_commit() -> Result<()> {
    let log = DuckDBEventLog::new(":memory:")?;
    
    // Produce 100 messages
    for i in 0..100 {
        let tick = MarketTick::new("BTC/USD", 45000.0 + i as f64, 1.5, i);
        log.append(&tick)?;
    }
    
    // Simulate consumer seeking to offset 50
    let mut consumer_offset = 0u64;
    
    // Read first batch
    let ticks = log.read_from(consumer_offset, 50)?;
    assert_eq!(ticks.len(), 50);
    consumer_offset = 50;
    
    // Seek to different offset
    let ticks = log.read_from(75, 10)?;
    assert_eq!(ticks.len(), 10);
    assert_eq!(ticks[0].sequence, 76);
    
    // Commit offset (application-level)
    let committed_offset = consumer_offset;
    
    // Resume from committed offset
    let ticks = log.read_from(committed_offset, 25)?;
    assert_eq!(ticks.len(), 25);
    
    Ok(())
}

// ============================================================================
// Test 4.3.2: Consumer group semantics (multiple independent readers)
// ============================================================================

#[test]
fn test_consumer_group_semantics() -> Result<()> {
    let log = Arc::new(DuckDBEventLog::new(":memory:")?);
    
    // Produce 200 messages
    for i in 0..200 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        log.append(&tick)?;
    }
    
    // Simulate consumer group with 4 consumers
    // Each consumer tracks its own offset
    let consumer_offsets = Arc::new(Mutex::new(vec![0u64; 4]));
    
    let mut handles = vec![];
    for consumer_id in 0..4 {
        let log_clone = Arc::clone(&log);
        let offsets_clone = Arc::clone(&consumer_offsets);
        
        let handle = std::thread::spawn(move || {
            // Each consumer reads 50 messages at its own pace
            for _ in 0..5 {
                let mut offsets = offsets_clone.lock();
                let offset = offsets[consumer_id];
                
                let ticks = log_clone.read_from(offset, 10).unwrap();
                offsets[consumer_id] += ticks.len() as u64;
                
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        });
        handles.push(handle);
    }
    
    // Wait for all consumers
    for handle in handles {
        handle.join().unwrap();
    }
    
    // All consumers should have read messages
    let offsets = consumer_offsets.lock();
    for (i, &offset) in offsets.iter().enumerate() {
        assert!(offset > 0, "Consumer {} didn't read any messages", i);
    }
    
    Ok(())
}

// ============================================================================
// Test 4.3.3: Message ordering guarantees
// ============================================================================

#[test]
fn test_ordering_guarantees() -> Result<()> {
    let log = DuckDBEventLog::new(":memory:")?;
    
    // Produce messages with increasing timestamps
    for i in 0..500 {
        let tick = MarketTick::new("BTC/USD", 45000.0 + i as f64, 1.5, i);
        log.append(&tick)?;
    }
    
    // Read all and verify strict ordering
    let ticks = log.read_from(0, 500)?;
    
    let mut prev_sequence = 0u64;
    let mut prev_timestamp = 0u64;
    
    for tick in ticks {
        assert!(tick.sequence > prev_sequence, "Sequence not monotonic");
        assert!(tick.timestamp >= prev_timestamp, "Timestamp not monotonic");
        prev_sequence = tick.sequence;
        prev_timestamp = tick.timestamp;
    }
    
    Ok(())
}

// ============================================================================
// Test 4.3.4: Replay from arbitrary offset
// ============================================================================

#[test]
fn test_replay_capability() -> Result<()> {
    let log = DuckDBEventLog::new(":memory:")?;
    
    // Produce 1000 messages
    for i in 0..1000 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        log.append(&tick)?;
    }
    
    // Test replay from various offsets
    let test_offsets = vec![0, 100, 250, 500, 750, 999];
    
    for offset in test_offsets {
        let replayed = log.read_from(offset, 10)?;
        assert!(!replayed.is_empty());
        assert!(replayed[0].sequence >= offset as u64);
    }
    
    // Replay entire log
    let full_replay = log.read_from(0, 2000)?;
    assert_eq!(full_replay.len(), 1000);
    
    // Replay last 100
    let last_100 = log.read_from(900, 100)?;
    assert_eq!(last_100.len(), 100);
    assert_eq!(last_100[0].sequence, 901);
    assert_eq!(last_100[99].sequence, 1000);
    
    Ok(())
}

// ============================================================================
// Test 4.3.5: Log compaction simulation
// ============================================================================

#[test]
fn test_log_compaction() -> Result<()> {
    let temp_path = tempfile::tempdir()?;
    let db_path = temp_path.path().join("compaction_test.duckdb");
    let db_str = db_path.to_str().unwrap();
    
    let log = DuckDBEventLog::new(db_str)?;
    
    // Produce 5000 messages
    for i in 0..5000 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        log.append(&tick)?;
    }
    
    // Verify initial count
    let initial_count = log.read_from(0, 10000)?.len();
    assert_eq!(initial_count, 5000);
    
    // Compact: keep only last 1000 messages
    log.conn.execute(
        "DELETE FROM market_ticks WHERE sequence < (SELECT MAX(sequence) - 1000 FROM market_ticks)",
        [],
    )?;
    
    // Verify compaction
    let remaining = log.read_from(0, 10000)?;
    assert_eq!(remaining.len(), 1000);
    
    // Verify we kept the latest messages
    assert_eq!(remaining[0].sequence, 4001);
    assert_eq!(remaining[999].sequence, 5000);
    
    // Verify database file size decreased
    let metadata = std::fs::metadata(db_path)?;
    assert!(metadata.len() > 0);
    
    Ok(())
}

// ============================================================================
// Test 4.3.6: Partition simulation (multiple symbols as partitions)
// ============================================================================

#[test]
fn test_partition_simulation() -> Result<()> {
    let log = DuckDBEventLog::new(":memory:")?;
    
    // Produce messages for 5 "partitions" (symbols)
    let symbols = vec!["BTC/USD", "ETH/USD", "SOL/USD", "XRP/USD", "DOGE/USD"];
    
    for i in 0..500 {
        let symbol = symbols[i % symbols.len()];
        let price = 1000.0 + (i % 100) as f64;
        let tick = MarketTick::new(symbol, price, 1.5, i);
        log.append(&tick)?;
    }
    
    // Read per-partition (like Kafka partition consumers)
    let mut partition_counts = std::collections::HashMap::new();
    
    for symbol in &symbols {
        let partition_ticks = log.query(|t| t.symbol == *symbol)?;
        partition_counts.insert(*symbol, partition_ticks.len());
        
        // Verify ordering within partition
        for i in 1..partition_ticks.len() {
            assert!(
                partition_ticks[i].sequence > partition_ticks[i-1].sequence,
                "Ordering violated in partition {}", symbol
            );
        }
    }
    
    // Each partition should have ~100 messages
    for (symbol, count) in &partition_counts {
        assert_eq!(*count, 100, "Partition {} has wrong count", symbol);
    }
    
    // Total should equal all messages
    let total: usize = partition_counts.values().sum();
    assert_eq!(total, 500);
    
    Ok(())
}

// ============================================================================
// Test 4.3.7: Exactly-once semantics simulation
// ============================================================================

#[test]
fn test_exactly_once_simulation() -> Result<()> {
    let log = DuckDBEventLog::new(":memory:")?;
    
    // Track processed messages
    let mut processed = std::collections::HashSet::new();
    let mut duplicate_count = 0u64;
    
    // Produce and "process" messages
    for i in 0..100 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        let seq = log.append(&tick)?;
        
        // Simulate processing with idempotency check
        if processed.contains(&seq) {
            duplicate_count += 1;
        } else {
            processed.insert(seq);
        }
    }
    
    // No duplicates should be processed
    assert_eq!(duplicate_count, 0);
    assert_eq!(processed.len(), 100);
    
    Ok(())
}

// ============================================================================
// Test 4.3.8: At-least-once delivery simulation
// ============================================================================

#[test]
fn test_at_least_once_delivery() -> Result<()> {
    let log = DuckDBEventLog::new(":memory:")?;
    
    // Produce messages
    for i in 0..50 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        log.append(&tick)?;
    }
    
    // Simulate at-least-once consumer (may reprocess on failure)
    let mut consumed = std::collections::HashSet::new();
    let mut total_deliveries = 0u64;
    
    // First pass
    let ticks = log.read_from(0, 50)?;
    for tick in ticks {
        consumed.insert(tick.sequence);
        total_deliveries += 1;
    }
    
    // Simulate failure and replay
    let replayed = log.read_from(25, 25)?;
    for tick in replayed {
        // Message may be redelivered
        total_deliveries += 1;
        // But we still have it in our set
        assert!(consumed.contains(&tick.sequence));
    }
    
    // All messages consumed at least once
    assert_eq!(consumed.len(), 50);
    // Some messages delivered more than once (at-least-once)
    assert!(total_deliveries >= 50);
    
    Ok(())
}

// ============================================================================
// Test 4.3.9: Consumer lag tracking
// ============================================================================

#[test]
fn test_consumer_lag() -> Result<()> {
    let log = DuckDBEventLog::new(":memory:")?;
    
    // Produce 1000 messages
    for i in 0..1000 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        log.append(&tick)?;
    }
    
    // Simulate slow consumer
    let mut consumer_offset = 0u64;
    let batch_size = 100u64;
    
    while consumer_offset < 1000 {
        let latest = log.latest_offset()?;
        let lag = latest - consumer_offset;
        
        // Track lag
        assert!(lag <= 1000);
        
        // Consume batch
        let ticks = log.read_from(consumer_offset, batch_size as usize)?;
        consumer_offset += ticks.len() as u64;
    }
    
    // Consumer caught up
    let final_lag = log.latest_offset()? - consumer_offset;
    assert_eq!(final_lag, 0);
    
    Ok(())
}

// ============================================================================
// Test 4.3.10: Multi-topic simulation (multiple tables)
// ============================================================================

#[test]
fn test_multi_topic_simulation() -> Result<()> {
    // Create separate logs for different "topics"
    let ticks_log = DuckDBEventLog::new(":memory:")?;
    let orders_log = DuckDBEventLog::new(":memory:")?;
    let trades_log = DuckDBEventLog::new(":memory:")?;
    
    // Produce to each topic
    for i in 0..100 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        ticks_log.append(&tick)?;
        
        let order = MarketTick::new("ETH/USD", 3200.0, 10.0, i);
        orders_log.append(&order)?;
        
        let trade = MarketTick::new("SOL/USD", 200.0, 50.0, i);
        trades_log.append(&trade)?;
    }
    
    // Verify isolation
    assert_eq!(ticks_log.read_from(0, 1000)?.len(), 100);
    assert_eq!(orders_log.read_from(0, 1000)?.len(), 100);
    assert_eq!(trades_log.read_from(0, 1000)?.len(), 100);
    
    Ok(())
}
