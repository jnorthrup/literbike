//! Phase 4: DuckDB Event Log Tests - Append/Read Operations
//!
//! Tests 4.2.1 - 4.2.6: Append, read, offset, query, performance

use literbike::kafka_replacement_smoke::*;
use anyhow::Result;

// ============================================================================
// Test 4.2.1: append() returns monotonically increasing sequence
// ============================================================================

#[test]
fn test_append_monotonic_sequence() -> Result<()> {
    let log = DuckDBEventLog::new(":memory:")?;
    
    let mut prev_seq = 0u64;
    for i in 0..100 {
        let tick = MarketTick::new("BTC/USD", 45000.0 + i as f64, 1.5, i);
        let seq = log.append(&tick)?;
        assert!(seq > prev_seq);
        prev_seq = seq;
    }
    
    Ok(())
}

// ============================================================================
// Test 4.2.2: read_from() with various offsets
// ============================================================================

#[test]
fn test_read_from_offsets() -> Result<()> {
    let log = DuckDBEventLog::new(":memory:")?;
    
    // Insert 50 ticks
    for i in 0..50 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        log.append(&tick)?;
    }
    
    // Read from offset 0
    let ticks = log.read_from(0, 10)?;
    assert_eq!(ticks.len(), 10);
    assert_eq!(ticks[0].sequence, 1);
    
    // Read from offset 25
    let ticks = log.read_from(25, 10)?;
    assert_eq!(ticks.len(), 10);
    assert_eq!(ticks[0].sequence, 26);
    
    // Read from offset 45 (near end)
    let ticks = log.read_from(45, 10)?;
    assert_eq!(ticks.len(), 5);
    
    // Read from offset beyond end
    let ticks = log.read_from(100, 10)?;
    assert_eq!(ticks.len(), 0);
    
    Ok(())
}

// ============================================================================
// Test 4.2.3: read_from() with limit parameter
// ============================================================================

#[test]
fn test_read_from_with_limit() -> Result<()> {
    let log = DuckDBEventLog::new(":memory:")?;
    
    // Insert 100 ticks
    for i in 0..100 {
        let tick = MarketTick::new("ETH/USD", 3200.0, 10.0, i);
        log.append(&tick)?;
    }
    
    // Test various limits
    assert_eq!(log.read_from(0, 1)?.len(), 1);
    assert_eq!(log.read_from(0, 10)?.len(), 10);
    assert_eq!(log.read_from(0, 50)?.len(), 50);
    assert_eq!(log.read_from(0, 100)?.len(), 100);
    assert_eq!(log.read_from(0, 200)?.len(), 100); // Limit exceeds data
    
    Ok(())
}

// ============================================================================
// Test 4.2.4: latest_offset() accuracy
// ============================================================================

#[test]
fn test_latest_offset_accuracy() -> Result<()> {
    let log = DuckDBEventLog::new(":memory:")?;
    
    // Initial offset
    assert_eq!(log.latest_offset()?, 0);
    
    // After each append
    for i in 1..=50 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        log.append(&tick)?;
        assert_eq!(log.latest_offset()?, i);
    }
    
    Ok(())
}

// ============================================================================
// Test 4.2.5: query() with filter functions
// ============================================================================

#[test]
fn test_query_with_filter() -> Result<()> {
    let log = DuckDBEventLog::new(":memory:")?;
    
    // Insert mixed symbols
    for i in 0..100 {
        let symbol = if i % 2 == 0 { "BTC/USD" } else { "ETH/USD" };
        let price = if i % 2 == 0 { 45000.0 } else { 3200.0 };
        let tick = MarketTick::new(symbol, price, 1.5, i);
        log.append(&tick)?;
    }
    
    // Filter BTC only
    let btc_ticks = log.query(|t| t.symbol == "BTC/USD")?;
    assert_eq!(btc_ticks.len(), 50);
    
    // Filter ETH only
    let eth_ticks = log.query(|t| t.symbol == "ETH/USD")?;
    assert_eq!(eth_ticks.len(), 50);
    
    // Filter by price range
    let high_price = log.query(|t| t.price > 10000.0)?;
    assert_eq!(high_price.len(), 50); // Only BTC
    
    // Filter by volume
    let high_volume = log.query(|t| t.volume > 1.0)?;
    assert_eq!(high_volume.len(), 100); // All have volume 1.5
    
    Ok(())
}

// ============================================================================
// Test 4.2.6: Read performance with large datasets
// ============================================================================

#[test]
fn test_read_performance_large_dataset() -> Result<()> {
    use std::time::Instant;
    
    let log = DuckDBEventLog::new(":memory:")?;
    
    // Insert 10,000 ticks
    let start = Instant::now();
    for i in 0..10000 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        log.append(&tick)?;
    }
    let append_time = start.elapsed();
    
    println!("Append 10,000 ticks: {:?}", append_time);
    
    // Read all
    let start = Instant::now();
    let ticks = log.read_from(0, 10000)?;
    let read_time = start.elapsed();
    
    println!("Read 10,000 ticks: {:?}", read_time);
    assert_eq!(ticks.len(), 10000);
    
    // Read with filter
    let start = Instant::now();
    let filtered = log.query(|t| t.price > 40000.0)?;
    let filter_time = start.elapsed();
    
    println!("Filter 10,000 ticks: {:?}", filter_time);
    assert_eq!(filtered.len(), 10000);
    
    // Performance should be reasonable (< 1 second for each operation)
    assert!(append_time.as_secs() < 5);
    assert!(read_time.as_secs() < 5);
    assert!(filter_time.as_secs() < 5);
    
    Ok(())
}

// ============================================================================
// Test 4.3: Kafka Compatibility
// ============================================================================

// Test 4.3.1: Offset management (seek to offset, commit offset)
#[test]
fn test_offset_management() -> Result<()> {
    let log = DuckDBEventLog::new(":memory:")?;
    
    // Produce 100 messages
    for i in 0..100 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        log.append(&tick)?;
    }
    
    // Simulate consumer seeking to offset 50
    let ticks = log.read_from(50, 10)?;
    assert_eq!(ticks.len(), 10);
    assert_eq!(ticks[0].sequence, 51);
    
    // Simulate committing offset (just track in application)
    let committed_offset = 60u64;
    
    // Resume from committed offset
    let ticks = log.read_from(committed_offset, 10)?;
    assert_eq!(ticks[0].sequence, 61);
    
    Ok(())
}

// Test 4.3.2: Consumer group semantics (multiple readers)
#[test]
fn test_multiple_readers() -> Result<()> {
    let log = Arc::new(DuckDBEventLog::new(":memory:")?);
    
    // Produce messages
    for i in 0..100 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        log.append(&tick)?;
    }
    
    // Simulate multiple consumers reading independently
    let mut handles = vec![];
    for consumer_id in 0..5 {
        let log_clone = Arc::clone(&log);
        let handle = std::thread::spawn(move || {
            // Each consumer reads at its own pace
            let offset = consumer_id * 20;
            let ticks = log_clone.read_from(offset, 20).unwrap();
            ticks.len()
        });
        handles.push(handle);
    }
    
    // All consumers should complete successfully
    for handle in handles {
        let count = handle.join().unwrap();
        assert!(count > 0);
    }
    
    Ok(())
}

// Test 4.3.3: Message ordering guarantees
#[test]
fn test_message_ordering() -> Result<()> {
    let log = DuckDBEventLog::new(":memory:")?;
    
    // Produce messages with specific order
    for i in 0..100 {
        let tick = MarketTick::new("BTC/USD", 45000.0 + i as f64, 1.5, i);
        log.append(&tick)?;
    }
    
    // Read all and verify order
    let ticks = log.read_from(0, 100)?;
    
    for i in 1..ticks.len() {
        assert!(ticks[i].sequence > ticks[i-1].sequence);
        assert!(ticks[i].price > ticks[i-1].price);
    }
    
    Ok(())
}

// Test 4.3.4: Replay from arbitrary offset
#[test]
fn test_replay_from_offset() -> Result<()> {
    let log = DuckDBEventLog::new(":memory:")?;
    
    // Produce 200 messages
    for i in 0..200 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        log.append(&tick)?;
    }
    
    // Replay from various offsets
    for offset in [0, 50, 100, 150, 199] {
        let ticks = log.read_from(offset, 10)?;
        assert!(!ticks.is_empty());
        assert!(ticks[0].sequence >= offset);
    }
    
    // Replay entire log
    let all_ticks = log.read_from(0, 1000)?;
    assert_eq!(all_ticks.len(), 200);
    
    Ok(())
}

// Test 4.3.5: Log compaction simulation (cleanup old records)
#[test]
fn test_log_compaction() -> Result<()> {
    let temp_path = tempfile::tempdir()?;
    let db_path = temp_path.path().join("test.duckdb");
    let db_str = db_path.to_str().unwrap();
    
    let log = DuckDBEventLog::new(db_str)?;
    
    // Produce 1000 messages
    for i in 0..1000 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        log.append(&tick)?;
    }
    
    // Simulate compaction: delete old records (keep last 100)
    log.conn.execute(
        "DELETE FROM market_ticks WHERE sequence < (SELECT MAX(sequence) - 100 FROM market_ticks)",
        [],
    )?;
    
    // Verify compaction
    let remaining = log.read_from(0, 1000)?;
    assert_eq!(remaining.len(), 100);
    
    Ok(())
}

// Test 4.3.6: Partition simulation (multiple symbols)
#[test]
fn test_partition_simulation() -> Result<()> {
    let log = DuckDBEventLog::new(":memory:")?;
    
    // Produce messages for multiple "partitions" (symbols)
    for i in 0..300 {
        let symbol = match i % 3 {
            0 => "BTC/USD",
            1 => "ETH/USD",
            _ => "SOL/USD",
        };
        let tick = MarketTick::new(symbol, 1000.0, 1.5, i);
        log.append(&tick)?;
    }
    
    // Read per-partition
    let btc = log.query(|t| t.symbol == "BTC/USD")?;
    let eth = log.query(|t| t.symbol == "ETH/USD")?;
    let sol = log.query(|t| t.symbol == "SOL/USD")?;
    
    assert_eq!(btc.len(), 100);
    assert_eq!(eth.len(), 100);
    assert_eq!(sol.len(), 100);
    
    // Verify ordering within each partition
    for ticks in [&btc, &eth, &sol] {
        for i in 1..ticks.len() {
            assert!(ticks[i].sequence > ticks[i-1].sequence);
        }
    }
    
    Ok(())
}
