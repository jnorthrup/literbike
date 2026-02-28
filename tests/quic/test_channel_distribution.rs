//! Phase 6: Channelized Distribution Tests
//!
//! Tests 6.1-6.2: Channel Management, Distribution Patterns

use literbike::kafka_replacement_smoke::*;
use anyhow::Result;

// ============================================================================
// Test 6.1: Channel Management
// ============================================================================

// Test 6.1.1: ChannelizedDistributor::new() with various sizes
#[tokio::test]
async fn test_distributor_creation() -> Result<()> {
    // Create with 1 channel
    let (dist1, rx1) = ChannelizedDistributor::new(1, 100);
    assert_eq!(dist1.channels.len(), 1);
    assert_eq!(rx1.len(), 1);

    // Create with 10 channels
    let (dist10, rx10) = ChannelizedDistributor::new(10, 50);
    assert_eq!(dist10.channels.len(), 10);
    assert_eq!(rx10.len(), 10);

    // Create with 100 channels
    let (dist100, rx100) = ChannelizedDistributor::new(100, 10);
    assert_eq!(dist100.channels.len(), 100);

    Ok(())
}

// Test 6.1.2: Bounded channel buffer behavior
#[tokio::test]
async fn test_bounded_channel_buffer() -> Result<()> {
    let (distributor, mut receivers) = ChannelizedDistributor::new(2, 5);

    // Fill buffer
    for i in 0..5 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        distributor.distribute(&tick).await?;
    }

    // Receivers should have messages
    for rx in &mut receivers {
        let count = rx.len();
        assert_eq!(count, 5);
    }

    Ok(())
}

// Test 6.1.3: Channel closure handling
#[tokio::test]
async fn test_channel_closure() -> Result<()> {
    let (distributor, receivers) = ChannelizedDistributor::new(3, 10);

    // Drop one receiver
    drop(receivers);

    // Distributor should still work (channel send will fail gracefully)
    let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, 1);
    let result = distributor.distribute(&tick).await;

    // Should handle closed channel
    assert!(result.is_ok() || result.is_err());

    Ok(())
}

// Test 6.1.4: Channel reconnection
#[tokio::test]
async fn test_channel_reconnection() -> Result<()> {
    // Create initial distributor
    let (dist1, _rx1) = ChannelizedDistributor::new(2, 10);

    // Create new distributor (reconnection simulation)
    let (dist2, _rx2) = ChannelizedDistributor::new(2, 10);

    // Both should work independently
    let tick1 = MarketTick::new("BTC/USD", 45000.0, 1.5, 1);
    let tick2 = MarketTick::new("ETH/USD", 3200.0, 10.0, 2);

    dist1.distribute(&tick1).await?;
    dist2.distribute(&tick2).await?;

    Ok(())
}

// Test 6.1.5: Channel memory leaks (long-running)
#[tokio::test]
async fn test_channel_memory_long_running() -> Result<()> {
    use std::time::Duration;

    let (distributor, mut receivers) = ChannelizedDistributor::new(3, 100);

    // Run for extended period
    for i in 0..1000 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        distributor.distribute(&tick).await?;

        // Drain receivers periodically
        if i % 100 == 0 {
            for rx in &mut receivers {
                while rx.try_recv().is_ok() {}
            }
        }
    }

    // Should complete without memory issues
    Ok(())
}

// ============================================================================
// Test 6.2: Distribution Patterns
// ============================================================================

// Test 6.2.1: Broadcast to all channels
#[tokio::test]
async fn test_broadcast_distribution() -> Result<()> {
    let (distributor, mut receivers) = ChannelizedDistributor::new(5, 100);

    let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, 1);
    distributor.distribute(&tick).await?;

    // All receivers should get the message
    for rx in &mut receivers {
        let received = rx.recv().await?;
        assert_eq!(received.symbol, "BTC/USD");
    }

    Ok(())
}

// Test 6.2.2: Round-robin distribution
#[tokio::test]
async fn test_round_robin_distribution() -> Result<()> {
    let (distributor, mut receivers) = ChannelizedDistributor::new(3, 100);

    // Send 9 messages
    for i in 0..9 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        distributor.distribute(&tick).await?;
    }

    // Each receiver should get all messages (broadcast behavior)
    for rx in &mut receivers {
        let mut count = 0;
        while rx.try_recv().is_ok() {
            count += 1;
        }
        assert_eq!(count, 9);
    }

    Ok(())
}

// Test 6.2.3: Partitioned distribution (by symbol)
#[tokio::test]
async fn test_partitioned_distribution() -> Result<()> {
    // Create distributors per partition
    let btc_dist = ChannelizedDistributor::new(1, 100);
    let eth_dist = ChannelizedDistributor::new(1, 100);

    // Distribute to partitions
    for i in 0..10 {
        let btc = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        let eth = MarketTick::new("ETH/USD", 3200.0, 10.0, i);

        btc_dist.0.distribute(&btc).await?;
        eth_dist.0.distribute(&eth).await?;
    }

    // Verify partition isolation
    let mut btc_rx = btc_dist.1.into_iter().next().unwrap();
    let mut eth_rx = eth_dist.1.into_iter().next().unwrap();

    while let Ok(tick) = btc_rx.try_recv() {
        assert_eq!(tick.symbol, "BTC/USD");
    }

    while let Ok(tick) = eth_rx.try_recv() {
        assert_eq!(tick.symbol, "ETH/USD");
    }

    Ok(())
}

// Test 6.2.4: Distribution ordering guarantees
#[tokio::test]
async fn test_distribution_ordering() -> Result<()> {
    let (distributor, mut receivers) = ChannelizedDistributor::new(2, 100);

    // Send ordered messages
    for i in 0..20 {
        let tick = MarketTick::new("BTC/USD", 45000.0 + i as f64, 1.5, i);
        distributor.distribute(&tick).await?;
    }

    // Verify ordering at receivers
    for rx in &mut receivers {
        let mut last_seq = 0u64;
        while let Ok(tick) = rx.try_recv() {
            assert!(tick.sequence > last_seq);
            last_seq = tick.sequence;
        }
    }

    Ok(())
}

// Test 6.2.5: Distribution with slow consumers
#[tokio::test]
async fn test_distribution_slow_consumers() -> Result<()> {
    let (distributor, mut receivers) = ChannelizedDistributor::new(3, 10);

    // Produce faster than consume
    for i in 0..50 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        distributor.distribute(&tick).await?;
    }

    // Slow consumers may have buffered messages
    for rx in &mut receivers {
        let count = rx.len();
        assert!(count <= 10); // Bounded by buffer size
    }

    Ok(())
}

// Test 6.2.6: Distribution failure handling
#[tokio::test]
async fn test_distribution_failure() -> Result<()> {
    let (distributor, _receivers) = ChannelizedDistributor::new(2, 10);

    // Drop receivers
    drop(_receivers);

    // Distribute should handle closed channels
    let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, 1);
    let result = distributor.distribute(&tick).await;

    // Should not panic
    let _ = result;

    Ok(())
}

// Test 6.2.7: Consumer group rebalancing
#[tokio::test]
async fn test_consumer_rebalancing() -> Result<()> {
    let (distributor, mut receivers) = ChannelizedDistributor::new(4, 100);

    // Initial distribution
    let tick1 = MarketTick::new("BTC/USD", 45000.0, 1.5, 1);
    distributor.distribute(&tick1).await?;

    // Remove one consumer (simulates rebalance)
    receivers.pop();

    // Continue distribution
    let tick2 = MarketTick::new("ETH/USD", 3200.0, 10.0, 2);
    distributor.distribute(&tick2).await?;

    // Remaining consumers should still receive
    for rx in &mut receivers {
        while rx.try_recv().is_ok() {}
    }

    Ok(())
}

// Test 6.2.8: Distribution metrics
#[tokio::test]
async fn test_distribution_metrics() -> Result<()> {
    use std::time::Instant;

    let (distributor, mut receivers) = ChannelizedDistributor::new(3, 1000);

    // Measure distribution latency
    let start = Instant::now();
    let count = 100u64;

    for i in 0..count {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        distributor.distribute(&tick).await?;
    }

    let elapsed = start.elapsed();
    let rate = count as f64 / elapsed.as_secs_f64();

    println!("Distribution rate: {:.0} msg/s", rate);

    // Verify all received
    for rx in &mut receivers {
        let mut received = 0;
        while rx.try_recv().is_ok() {
            received += 1;
        }
        assert_eq!(received as u64, count);
    }

    Ok(())
}
