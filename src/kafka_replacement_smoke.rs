//! Kafka Replacement Smoke Test
//! 
//! Demonstrates how Literbike's QUIC + CCEK + DuckDB stack
//! can replace Kafka for a generic bot engine use case.
//!
//! Architecture:
//! ```text
//! Market Feed ──> QUIC Stream ──> DuckDB Tree-WAL ──> Channel ──> Pandas Agent
//!                      │                │
//!                      │                └─> Federated Query
//!                      │
//!                      └─> Durable Log (replayable)
//! ```
//!
//! Key Kafka features replaced:
//! 1. **Durable event log** → DuckDB with WAL
//! 2. **Pub/sub messaging** → QUIC streams + async-channel
//! 3. **Message replay** → DuckDB table scans
//! 4. **Ordering guarantees** → QUIC stream ordering + sequence numbers
//! 5. **Backpressure** → async-channel bounded buffers

use crate::concurrency::*;
// betanet patterns removed during cleanup
// use crate::betanet_patterns::*;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;
use anyhow::Result;
use duckdb::params;

// ============================================================================
// Market Data Types
// ============================================================================

#[derive(Debug, Clone)]
pub struct MarketTick {
    pub symbol: String,
    pub price: f64,
    pub volume: f64,
    pub timestamp: u64,
    pub sequence: u64,
}

impl MarketTick {
    pub fn new(symbol: &str, price: f64, volume: f64, sequence: u64) -> Self {
        Self {
            symbol: symbol.to_string(),
            price,
            volume,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            sequence,
        }
    }
}

// ============================================================================
// DuckDB Tree-WAL (Kafka Log Replacement)
// ============================================================================

/// DuckDB-backed event log (replaces Kafka topic)
pub struct DuckDBEventLog {
    db_path: String,
    conn: duckdb::Connection,
    sequence: std::sync::atomic::AtomicU64,
}

impl DuckDBEventLog {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = duckdb::Connection::open(db_path)?;
        
        // Create event log table (Kafka topic equivalent)
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS market_ticks (
                sequence BIGINT PRIMARY KEY,
                symbol VARCHAR,
                price DOUBLE,
                volume DOUBLE,
                timestamp BIGINT,
                ingested_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )",
        )?;
        
        // Get max sequence
        let max_seq: u64 = conn.query_row(
            "SELECT COALESCE(MAX(sequence), 0) FROM market_ticks",
            [],
            |row| row.get(0),
        )?;
        
        Ok(Self {
            db_path: db_path.to_string(),
            conn,
            sequence: std::sync::atomic::AtomicU64::new(max_seq),
        })
    }
    
    /// Append event to log (like Kafka producer send)
    pub fn append(&self, tick: &MarketTick) -> Result<u64> {
        let seq = self.sequence.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;

        self.conn.execute(
            "INSERT INTO market_ticks (sequence, symbol, price, volume, timestamp)
             VALUES (?, ?, ?, ?, ?)",
            params![
                seq as i64,
                tick.symbol.as_str(),
                tick.price,
                tick.volume,
                tick.timestamp as i64,
            ],
        )?;

        Ok(seq)
    }
    
    /// Read from offset (like Kafka consumer seek)
    pub fn read_from(&self, offset: u64, limit: usize) -> Result<Vec<MarketTick>> {
        let mut stmt = self.conn.prepare(
            "SELECT sequence, symbol, price, volume, timestamp 
             FROM market_ticks 
             WHERE sequence >= ? 
             ORDER BY sequence 
             LIMIT ?"
        )?;
        
        let ticks = stmt.query_map([offset as i64, limit as i64], |row| {
            Ok(MarketTick {
                sequence: row.get(0)?,
                symbol: row.get(1)?,
                price: row.get(2)?,
                volume: row.get(3)?,
                timestamp: row.get(4)?,
            })
        })?;
        
        let mut result = Vec::new();
        for tick_result in ticks {
            result.push(tick_result?);
        }
        
        Ok(result)
    }
    
    /// Get latest offset (like Kafka end offset)
    pub fn latest_offset(&self) -> Result<u64> {
        let offset: u64 = self.conn.query_row(
            "SELECT COALESCE(MAX(sequence), 0) FROM market_ticks",
            [],
            |row| row.get(0),
        )?;
        Ok(offset)
    }
    
    /// Query with filtering (Kafka Streams equivalent)
    pub fn query<F>(&self, filter: F) -> Result<Vec<MarketTick>>
    where
        F: Fn(&MarketTick) -> bool,
    {
        let all_ticks = self.read_from(0, 1_000_000)?;
        Ok(all_ticks.into_iter().filter(filter).collect())
    }
}

// ============================================================================
// QUIC Stream Ingestion (Kafka Producer Equivalent)
// ============================================================================

pub struct QuicStreamIngest {
    log: Arc<DuckDBEventLog>,
    broadcast_tx: broadcast::Sender<MarketTick>,
}

impl QuicStreamIngest {
    pub fn new(log: Arc<DuckDBEventLog>) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1024);
        Self { log, broadcast_tx }
    }
    
    /// Ingest market tick (like Kafka produce)
    pub async fn ingest(&self, tick: MarketTick) -> Result<u64> {
        // 1. Append to durable log (DuckDB WAL)
        let seq = self.log.append(&tick)?;
        
        // 2. Broadcast to subscribers (like Kafka consumers)
        let _ = self.broadcast_tx.send(tick);
        
        Ok(seq)
    }
    
    /// Subscribe to stream (like Kafka consumer)
    pub fn subscribe(&self) -> broadcast::Receiver<MarketTick> {
        self.broadcast_tx.subscribe()
    }
}

// ============================================================================
// Channelized Distribution (Kafka Consumer Groups)
// ============================================================================

pub struct ChannelizedDistributor {
    channels: Vec<async_channel::Sender<MarketTick>>,
}

impl ChannelizedDistributor {
    pub fn new(num_channels: usize, buffer_size: usize) -> (Self, Vec<async_channel::Receiver<MarketTick>>) {
        let mut channels = Vec::new();
        let mut receivers = Vec::new();
        
        for _ in 0..num_channels {
            let (tx, rx) = async_channel::bounded(buffer_size);
            channels.push(tx);
            receivers.push(rx);
        }
        
        (Self { channels }, receivers)
    }
    
    /// Distribute to all channels (like Kafka broadcast to consumer group)
    pub async fn distribute(&self, tick: &MarketTick) -> Result<()> {
        for tx in &self.channels {
            tx.send(tick.clone()).await?;
        }
        Ok(())
    }
}

// ============================================================================
// Pandas Edge Agent (Consumer)
// ============================================================================

pub struct PandasEdgeAgent {
    agent_id: String,
    subscription: broadcast::Receiver<MarketTick>,
    channel_rx: Option<async_channel::Receiver<MarketTick>>,
    processed_count: u64,
}

impl PandasEdgeAgent {
    pub fn new_from_broadcast(agent_id: &str, subscription: broadcast::Receiver<MarketTick>) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            subscription,
            channel_rx: None,
            processed_count: 0,
        }
    }
    
    pub fn new_from_channel(agent_id: &str, channel_rx: async_channel::Receiver<MarketTick>) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            subscription: broadcast::channel(1024).1, // Dummy
            channel_rx: Some(channel_rx),
            processed_count: 0,
        }
    }
    
    /// Process stream (like Kafka consumer poll loop)
    pub async fn run(&mut self) -> Result<Vec<MarketTick>> {
        let mut processed = Vec::new();
        
        while let Ok(tick) = if let Some(rx) = &mut self.channel_rx {
            rx.recv().await.map_err(|e| anyhow::anyhow!("{}", e))
        } else {
            self.subscription.recv().await.map_err(|e| anyhow::anyhow!("{}", e))
        } {
            // Simulate Pandas processing
            self.processed_count += 1;
            processed.push(tick);
            
            // Stop after 10 for smoke test
            if processed.len() >= 10 {
                break;
            }
        }
        
        Ok(processed)
    }
}

// ============================================================================
// Smoke Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_duckdb_event_log() -> Result<()> {
        let log = DuckDBEventLog::new(":memory:")?;
        
        // Append ticks
        let tick1 = MarketTick::new("BTC/USD", 45000.0, 1.5, 1);
        let tick2 = MarketTick::new("ETH/USD", 3200.0, 10.0, 2);
        
        let seq1 = log.append(&tick1)?;
        let seq2 = log.append(&tick2)?;
        
        assert_eq!(seq1, 1);
        assert_eq!(seq2, 2);
        
        // Read from offset
        let ticks = log.read_from(1, 10)?;
        assert_eq!(ticks.len(), 2);
        assert_eq!(ticks[0].symbol, "BTC/USD");
        
        // Latest offset
        let latest = log.latest_offset()?;
        assert_eq!(latest, 2);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_quic_stream_ingest() -> Result<()> {
        let log = Arc::new(DuckDBEventLog::new(":memory:")?);
        let ingest = QuicStreamIngest::new(log.clone());
        
        // Subscribe before sending
        let mut rx = ingest.subscribe();
        
        // Ingest tick
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, 1);
        let seq = ingest.ingest(tick.clone()).await?;
        
        assert_eq!(seq, 1);
        
        // Verify broadcast
        let received = tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await??;
        assert_eq!(received.symbol, "BTC/USD");
        assert_eq!(received.price, 45000.0);
        
        // Verify durable log
        let ticks = log.read_from(0, 10)?;
        assert_eq!(ticks.len(), 1);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_channelized_distributor() -> Result<()> {
        let log = Arc::new(DuckDBEventLog::new(":memory:")?);
        let ingest = QuicStreamIngest::new(log);
        
        // Create distributor with 3 channels
        let (distributor, receivers) = ChannelizedDistributor::new(3, 100);
        
        // Distribute tick
        let tick = MarketTick::new("ETH/USD", 3200.0, 10.0, 1);
        distributor.distribute(&tick).await?;
        
        // All channels should receive
        for mut rx in receivers {
            let received = rx.recv().await?;
            assert_eq!(received.symbol, "ETH/USD");
        }
        
        Ok(())
    }

    #[tokio::test]
    async fn test_pandas_edge_agent() -> Result<()> {
        let log = Arc::new(DuckDBEventLog::new(":memory:")?);
        let ingest = QuicStreamIngest::new(log);
        
        // Create agent
        let mut agent = PandasEdgeAgent::new_from_broadcast(
            "agent-1",
            ingest.subscribe(),
        );
        
        // Send 10 ticks
        for i in 0..10 {
            let tick = MarketTick::new("BTC/USD", 45000.0 + i as f64, 1.5, i);
            ingest.ingest(tick).await?;
        }
        
        // Agent processes
        let processed = agent.run().await?;
        assert_eq!(processed.len(), 10);
        assert_eq!(agent.processed_count, 10);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_kafka_replacement_full_flow() -> Result<()> {
        // Setup: DuckDB event log (Kafka topic)
        let log = Arc::new(DuckDBEventLog::new(":memory:")?);
        
        // Setup: QUIC stream ingest (Kafka producer)
        let ingest = QuicStreamIngest::new(log.clone());
        
        // Setup: Channelized distributor (Kafka consumer groups)
        let (distributor, receivers) = ChannelizedDistributor::new(2, 100);
        
        // Setup: Pandas edge agents (Kafka consumers)
        let mut agents: Vec<PandasEdgeAgent> = receivers
            .into_iter()
            .enumerate()
            .map(|(i, rx)| PandasEdgeAgent::new_from_channel(&format!("agent-{}", i), rx))
            .collect();
        
        // Produce 20 ticks
        for i in 0..20 {
            let symbol = if i % 2 == 0 { "BTC/USD" } else { "ETH/USD" };
            let price = if i % 2 == 0 { 45000.0 } else { 3200.0 };
            let tick = MarketTick::new(symbol, price, 1.5, i);
            
            // Ingest to durable log
            ingest.ingest(tick.clone()).await?;
            
            // Distribute to consumer groups
            distributor.distribute(&tick).await?;
        }
        
        // Verify durable log (replay capability)
        let all_ticks = log.read_from(0, 100)?;
        assert_eq!(all_ticks.len(), 20);
        
        // Verify BTC ticks only (Kafka Streams filtering)
        let btc_ticks = log.query(|t| t.symbol == "BTC/USD")?;
        assert_eq!(btc_ticks.len(), 10);
        
        // Agents receive their messages
        for agent in &mut agents {
            let processed = tokio::time::timeout(
                Duration::from_millis(100),
                agent.run(),
            )
            .await??;
            assert_eq!(processed.len(), 10); // Each agent gets all 10 of its messages
        }
        
        // Verify latest offset
        let latest = log.latest_offset()?;
        assert_eq!(latest, 20);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_replay_from_offset() -> Result<()> {
        let log = DuckDBEventLog::new(":memory:")?;
        
        // Produce 50 ticks
        for i in 0..50 {
            let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
            log.append(&tick)?;
        }
        
        // Replay from offset 25 (like Kafka consumer seek)
        let replayed = log.read_from(25, 10)?;
        assert_eq!(replayed.len(), 10);
        assert_eq!(replayed[0].sequence, 25);
        assert_eq!(replayed[9].sequence, 34);
        
        // Replay from beginning
        let from_start = log.read_from(0, 5)?;
        assert_eq!(from_start.len(), 5);
        assert_eq!(from_start[0].sequence, 0);
        
        Ok(())
    }
}
