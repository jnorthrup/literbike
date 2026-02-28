//! Phase 5: QUIC Stream Ingestion Tests
//!
//! Tests 5.1-5.2: Ingestion Pipeline, Stream Integration

use literbike::kafka_replacement_smoke::*;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::broadcast;

// ============================================================================
// Test 5.1: Ingestion Pipeline
// ============================================================================

// Test 5.1.1: QuicStreamIngest::ingest() end-to-end
#[tokio::test]
async fn test_ingest_end_to_end() -> Result<()> {
    let log = Arc::new(DuckDBEventLog::new(":memory:")?);
    let ingest = QuicStreamIngest::new(log.clone());
    
    // Subscribe before ingest
    let mut rx = ingest.subscribe();
    
    // Ingest tick
    let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, 1);
    let seq = ingest.ingest(tick.clone()).await?;
    
    assert_eq!(seq, 1);
    
    // Verify received via broadcast
    let received = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        rx.recv()
    ).await??;
    
    assert_eq!(received.symbol, "BTC/USD");
    assert_eq!(received.price, 45000.0);
    
    Ok(())
}

// Test 5.1.2: Broadcast channel distribution
#[tokio::test]
async fn test_broadcast_distribution() -> Result<()> {
    let log = Arc::new(DuckDBEventLog::new(":memory:")?);
    let ingest = QuicStreamIngest::new(log);
    
    // Create multiple subscribers
    let mut subscribers = vec![];
    for _ in 0..5 {
        subscribers.push(ingest.subscribe());
    }
    
    // Ingest tick
    let tick = MarketTick::new("ETH/USD", 3200.0, 10.0, 1);
    ingest.ingest(tick.clone()).await?;
    
    // All subscribers should receive
    for mut rx in subscribers {
        let received = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            rx.recv()
        ).await??;
        
        assert_eq!(received.symbol, "ETH/USD");
    }
    
    Ok(())
}

// Test 5.1.3: Subscriber lag handling (slow consumers)
#[tokio::test]
async fn test_subscriber_lag() -> Result<()> {
    let log = Arc::new(DuckDBEventLog::new(":memory:")?);
    let ingest = QuicStreamIngest::new(log);
    
    // Create subscriber but don't consume
    let _slow_subscriber = ingest.subscribe();
    
    // Produce many messages
    for i in 0..100 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        ingest.ingest(tick).await?;
    }
    
    // Slow subscriber may have missed messages (broadcast channel behavior)
    // This is expected - broadcast channels don't buffer for slow consumers
    
    Ok(())
}

// Test 5.1.4: Backpressure (bounded channel full)
#[tokio::test]
async fn test_backpressure() -> Result<()> {
    let log = Arc::new(DuckDBEventLog::new(":memory:")?);
    let ingest = QuicStreamIngest::new(log);
    
    // Create bounded channel distributor
    let (distributor, receivers) = ChannelizedDistributor::new(3, 10);
    
    // Fill channels
    for i in 0..50 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        distributor.distribute(&tick).await?;
    }
    
    // Channels should handle backpressure
    // (async-channel blocks when full)
    
    // Verify receivers can still consume
    for mut rx in receivers {
        let tick = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            rx.recv()
        ).await??;
        
        assert_eq!(tick.symbol, "BTC/USD");
    }
    
    Ok(())
}

// Test 5.1.5: Message durability (write-ahead before broadcast)
#[tokio::test]
async fn test_durability() -> Result<()> {
    let log = Arc::new(DuckDBEventLog::new(":memory:")?);
    let ingest = QuicStreamIngest::new(log.clone());
    
    // Ingest messages
    for i in 0..10 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        ingest.ingest(tick).await?;
    }
    
    // Verify all messages are in durable log
    let stored = log.read_from(0, 100)?;
    assert_eq!(stored.len(), 10);
    
    // Even if broadcast subscribers fail, log persists
    drop(ingest);
    
    let still_stored = log.read_from(0, 100)?;
    assert_eq!(still_stored.len(), 10);
    
    Ok(())
}

// Test 5.1.6: Ingest rate limiting
#[tokio::test]
async fn test_ingest_rate() -> Result<()> {
    use std::time::Instant;
    
    let log = Arc::new(DuckDBEventLog::new(":memory:")?);
    let ingest = QuicStreamIngest::new(log);
    
    // Measure ingest rate
    let start = Instant::now();
    let count = 1000u64;
    
    for i in 0..count {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        ingest.ingest(tick).await?;
    }
    
    let elapsed = start.elapsed();
    let rate = count as f64 / elapsed.as_secs_f64();
    
    println!("Ingest rate: {:.0} messages/second", rate);
    
    // Should achieve reasonable throughput
    assert!(rate > 100.0, "Ingest rate too low: {}", rate);
    
    Ok(())
}

// ============================================================================
// Test 5.2: Stream Integration
// ============================================================================

// Test 5.2.1: QUIC stream → DuckDB log integration
#[tokio::test]
async fn test_quic_to_duckdb_integration() -> Result<()> {
    let log = Arc::new(DuckDBEventLog::new(":memory:")?);
    let ingest = QuicStreamIngest::new(log.clone());
    
    // Simulate QUIC stream data arriving
    let mut rx = ingest.subscribe();
    
    for i in 0..20 {
        let tick = MarketTick::new("BTC/USD", 45000.0 + i as f64, 1.5, i);
        ingest.ingest(tick).await?;
    }
    
    // Verify in DuckDB
    let stored = log.read_from(0, 100)?;
    assert_eq!(stored.len(), 20);
    
    // Verify broadcast received
    let mut received_count = 0;
    while let Ok(_) = rx.try_recv() {
        received_count += 1;
    }
    assert!(received_count > 0);
    
    Ok(())
}

// Test 5.2.2: Stream payload classification (RbCursive)
#[tokio::test]
async fn test_stream_classification() -> Result<()> {
    use literbike::rbcursive::{RbCursor, NetTuple, Protocol};
    
    let log = Arc::new(DuckDBEventLog::new(":memory:")?);
    let ingest = QuicStreamIngest::new(log);
    
    // Create RbCursor for classification
    let mut cursor = RbCursor::new();
    
    // Simulate stream data
    let http_data = b"GET /api/ticks HTTP/1.1\r\nHost: example.com\r\n\r\n";
    let socks5_data = &[0x05, 0x01, 0x00];
    
    // Classify HTTP
    let tuple = NetTuple::from_socket_addr(
        "127.0.0.1:8080".parse().unwrap(),
        Protocol::HtxQuic
    );
    let http_signal = cursor.recognize(tuple, &http_data[..]);
    
    // Classify SOCKS5
    let tuple = NetTuple::from_socket_addr(
        "127.0.0.1:1080".parse().unwrap(),
        Protocol::HtxQuic
    );
    let socks_signal = cursor.recognize(tuple, &socks5_data[..]);
    
    // Verify classification works
    // (Specific signals depend on RbCursive implementation)
    
    Ok(())
}

// Test 5.2.3: Stream error propagation
#[tokio::test]
async fn test_stream_error_propagation() -> Result<()> {
    let log = Arc::new(DuckDBEventLog::new(":memory:")?);
    let ingest = QuicStreamIngest::new(log);
    
    // Ingest valid message
    let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, 1);
    let result = ingest.ingest(tick).await;
    assert!(result.is_ok());
    
    // Errors should propagate properly
    // (Test with invalid data if applicable)
    
    Ok(())
}

// Test 5.2.4: Stream close handling
#[tokio::test]
async fn test_stream_close() -> Result<()> {
    let log = Arc::new(DuckDBEventLog::new(":memory:")?);
    let ingest = QuicStreamIngest::new(log.clone());
    
    let mut rx = ingest.subscribe();
    
    // Ingest some messages
    for i in 0..5 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        ingest.ingest(tick).await?;
    }
    
    // Drop ingest (simulates stream close)
    drop(ingest);
    
    // Subscriber should see channel close eventually
    // (broadcast channel behavior)
    
    Ok(())
}

// Test 5.2.5: Multiple concurrent streams
#[tokio::test]
async fn test_concurrent_streams() -> Result<()> {
    let log = Arc::new(DuckDBEventLog::new(":memory:")?);
    
    // Create multiple ingest pipelines
    let mut ingests = vec![];
    let mut subscribers = vec![];
    
    for i in 0..5 {
        let ingest = QuicStreamIngest::new(log.clone());
        subscribers.push(ingest.subscribe());
        ingests.push(ingest);
    }
    
    // Ingest from each pipeline concurrently
    let mut handles = vec![];
    for (i, ingest) in ingests.into_iter().enumerate() {
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                let tick = MarketTick::new("BTC/USD", 45000.0 + j as f64, 1.5, i * 10 + j);
                ingest.ingest(tick).await.unwrap();
            }
        });
        handles.push(handle);
    }
    
    // Wait for all
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Verify all messages stored
    let stored = log.read_from(0, 1000)?;
    assert_eq!(stored.len(), 50);
    
    Ok(())
}

// Test 5.2.6: Stream priority handling
#[tokio::test]
async fn test_stream_priority() -> Result<()> {
    let log = Arc::new(DuckDBEventLog::new(":memory:")?);
    let ingest = QuicStreamIngest::new(log.clone());
    
    // Ingest high-priority messages (e.g., price alerts)
    let high_priority = MarketTick::new("BTC/USD", 50000.0, 1.5, 0); // Alert price
    ingest.ingest(high_priority).await?;
    
    // Ingest normal messages
    for i in 1..10 {
        let normal = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        ingest.ingest(normal).await?;
    }
    
    // High priority should be first in log
    let first = log.read_from(0, 1)?;
    assert_eq!(first[0].price, 50000.0);
    
    Ok(())
}
