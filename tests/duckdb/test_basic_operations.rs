//! Phase 4: DuckDB Event Log Tests - Basic Operations
//!
//! Tests 4.1.1 - 4.1.6: Database creation, schema, sequence init, concurrency, recovery

use literbike::kafka_replacement_smoke::*;
use anyhow::Result;

// ============================================================================
// Test 4.1.1: DuckDBEventLog::new() with :memory: database
// ============================================================================

#[test]
fn test_duckdb_memory_database() -> Result<()> {
    let log = DuckDBEventLog::new(":memory:")?;
    
    // Verify log is created
    assert!(log.latest_offset()? >= 0);
    
    Ok(())
}

// ============================================================================
// Test 4.1.2: DuckDBEventLog::new() with file database
// ============================================================================

#[test]
fn test_duckdb_file_database() -> Result<()> {
    let temp_path = tempfile::tempdir()?;
    let db_path = temp_path.path().join("test.duckdb");
    let db_str = db_path.to_str().unwrap();
    
    // Create database
    let log = DuckDBEventLog::new(db_str)?;
    
    // Verify file exists
    assert!(db_path.exists());
    
    // Verify we can write
    let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, 1);
    log.append(&tick)?;
    
    // Verify we can read
    let ticks = log.read_from(0, 10)?;
    assert_eq!(ticks.len(), 1);
    
    Ok(())
}

// ============================================================================
// Test 4.1.3: Database schema creation
// ============================================================================

#[test]
fn test_schema_creation() -> Result<()> {
    let log = DuckDBEventLog::new(":memory:")?;
    
    // Insert and verify schema is correct
    let tick = MarketTick::new("ETH/USD", 3200.0, 10.0, 1);
    let seq = log.append(&tick)?;
    
    assert_eq!(seq, 1);
    
    // Verify all columns exist by reading
    let ticks = log.read_from(0, 10)?;
    assert_eq!(ticks.len(), 1);
    assert_eq!(ticks[0].symbol, "ETH/USD");
    assert_eq!(ticks[0].price, 3200.0);
    assert_eq!(ticks[0].volume, 10.0);
    
    Ok(())
}

// ============================================================================
// Test 4.1.4: Sequence number initialization from existing data
// ============================================================================

#[test]
fn test_sequence_initialization() -> Result<()> {
    let temp_path = tempfile::tempdir()?;
    let db_path = temp_path.path().join("test.duckdb");
    let db_str = db_path.to_str().unwrap();
    
    // Create log and add some data
    {
        let log = DuckDBEventLog::new(db_str)?;
        for i in 0..10 {
            let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
            log.append(&tick)?;
        }
    }
    
    // Reopen database
    let log2 = DuckDBEventLog::new(db_str)?;
    
    // Verify sequence continues from where it left off
    let next_seq = log2.latest_offset()?;
    assert_eq!(next_seq, 10);
    
    // Add more data
    let tick = MarketTick::new("ETH/USD", 3200.0, 10.0, 11);
    let new_seq = log2.append(&tick)?;
    assert_eq!(new_seq, 11);
    
    Ok(())
}

// ============================================================================
// Test 4.1.5: Concurrent database access
// ============================================================================

#[test]
fn test_concurrent_access() -> Result<()> {
    let log = Arc::new(DuckDBEventLog::new(":memory:")?);
    
    let mut handles = vec![];
    
    // Spawn multiple writers
    for i in 0..10 {
        let log_clone = Arc::clone(&log);
        let handle = std::thread::spawn(move || {
            for j in 0..10 {
                let tick = MarketTick::new("BTC/USD", 45000.0 + j as f64, 1.5, i * 10 + j);
                log_clone.append(&tick).unwrap();
            }
        });
        handles.push(handle);
    }
    
    // Wait for all writers
    for handle in handles {
        handle.join().unwrap();
    }
    
    // Verify all writes succeeded
    let latest = log.latest_offset()?;
    assert_eq!(latest, 100);
    
    // Verify we can read all
    let ticks = log.read_from(0, 200)?;
    assert_eq!(ticks.len(), 100);
    
    Ok(())
}

// ============================================================================
// Test 4.1.6: Database recovery after crash (WAL replay)
// ============================================================================

#[test]
fn test_wal_recovery() -> Result<()> {
    let temp_path = tempfile::tempdir()?;
    let db_path = temp_path.path().join("test.duckdb");
    let db_str = db_path.to_str().unwrap();
    
    // Create log and add data
    {
        let log = DuckDBEventLog::new(db_str)?;
        for i in 0..50 {
            let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
            log.append(&tick)?;
        }
        // Note: We don't explicitly flush - DuckDB handles this
    }
    
    // Reopen (simulates recovery after crash)
    let log2 = DuckDBEventLog::new(db_str)?;
    
    // Verify all data is preserved
    let latest = log2.latest_offset()?;
    assert_eq!(latest, 50);
    
    let ticks = log2.read_from(0, 100)?;
    assert_eq!(ticks.len(), 50);
    
    Ok(())
}
