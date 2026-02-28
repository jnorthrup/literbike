//! Phase 11: Benchmark Tests
//!
//! Performance benchmarks for Kafka replacement stack

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use literbike::kafka_replacement_smoke::*;
use std::sync::Arc;

// ============================================================================
// Benchmark 11.1.1: DuckDB append rate
// ============================================================================

fn bench_duckdb_append(c: &mut Criterion) {
    let log = DuckDBEventLog::new(":memory:").unwrap();

    c.bench_function("duckdb_append", |b| {
        b.iter(|| {
            let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, black_box(1));
            log.append(&tick).unwrap();
        })
    });
}

// ============================================================================
// Benchmark 11.1.2: DuckDB query rate
// ============================================================================

fn bench_duckdb_query(c: &mut Criterion) {
    let log = DuckDBEventLog::new(":memory:").unwrap();

    // Pre-populate
    for i in 0..10000 {
        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
        log.append(&tick).unwrap();
    }

    c.bench_function("duckdb_query_10k", |b| {
        b.iter(|| {
            let _ticks = log.query(|t| t.price > 40000.0).unwrap();
        })
    });
}

// ============================================================================
// Benchmark 11.1.3: QUIC stream throughput
// ============================================================================

fn bench_quic_stream_throughput(c: &mut Criterion) {
    // Note: Full QUIC benchmark requires network setup
    // This is a placeholder for the engine-level benchmark

    c.bench_function("quic_packet_serialize", |b| {
        use literbike::quic::*;

        let packet = QuicPacket {
            header: QuicHeader {
                r#type: QuicPacketType::ShortHeader,
                version: 1,
                destination_connection_id: vec![1, 2, 3, 4],
                source_connection_id: vec![5, 6, 7, 8],
                packet_number: 1,
                token: None,
            },
            frames: vec![QuicFrame::Stream(StreamFrame {
                stream_id: 1,
                offset: 0,
                data: vec![0; 1000],
                fin: false,
            })],
            payload: vec![],
        };

        b.iter(|| {
            let _serialized = bincode::serialize(&packet).unwrap();
        })
    });
}

// ============================================================================
// Benchmark 11.1.4: Broadcast channel latency
// ============================================================================

fn bench_broadcast_latency(c: &mut Criterion) {
    use tokio::sync::broadcast;

    let (tx, _rx) = broadcast::channel::<MarketTick>(1024);

    c.bench_function("broadcast_send", |b| {
        b.iter(|| {
            let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, black_box(1));
            let _ = tx.send(tick);
        })
    });
}

// ============================================================================
// Benchmark 11.1.5: Async-channel distribution
// ============================================================================

fn bench_async_channel_distribution(c: &mut Criterion) {
    use async_channel::bounded;

    let (tx, rx) = bounded::<MarketTick>(100);

    c.bench_function("async_channel_send_recv", |b| {
        b.iter(|| {
            let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, black_box(1));

            // Sync benchmark using block_on
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                tx.send(tick).await.unwrap();
                let _ = rx.recv().await.unwrap();
            });
        })
    });
}

// ============================================================================
// Benchmark 11.1.6: End-to-end latency (ingest → consume)
// ============================================================================

fn bench_end_to_end_latency(c: &mut Criterion) {
    let log = Arc::new(DuckDBEventLog::new(":memory:").unwrap());
    let ingest = QuicStreamIngest::new(log);

    c.bench_function("ingest_to_broadcast", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let mut rx = ingest.subscribe();
                let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, black_box(1));
                ingest.ingest(tick).await.unwrap();
                let _ = rx.recv().await.unwrap();
            });
        })
    });
}

// ============================================================================
// Benchmark 11.1.7: Memory allocation per message
// ============================================================================

fn bench_memory_per_message(c: &mut Criterion) {
    c.bench_function("message_allocation", |b| {
        b.iter(|| {
            let _tick = MarketTick::new(
                black_box("BTC/USD"),
                black_box(45000.0),
                black_box(1.5),
                black_box(1),
            );
        })
    });
}

// ============================================================================
// Benchmark 11.1.8: CCEK overhead
// ============================================================================

fn bench_ccek_overhead(c: &mut Criterion) {
    use literbike::quic::quic_ccek::QuicCcek;

    c.bench_function("ccek_init", |b| {
        b.iter(|| {
            let _ccek = QuicCcek::new_with_key_graph();
        })
    });

    c.bench_function("ccek_transition", |b| {
        b.iter(|| {
            let mut ccek = QuicCcek::new_with_key_graph();
            let _ = ccek.execute_reactor_continuation(0x1001);
        })
    });
}

// ============================================================================
// Parameterized benchmarks
// ============================================================================

fn bench_duckdb_append_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("duckdb_append_batch");

    for batch_size in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(batch_size), batch_size, |b, &size| {
            let log = DuckDBEventLog::new(":memory:").unwrap();

            b.iter(|| {
                for i in 0..size {
                    let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, i);
                    log.append(&tick).unwrap();
                }
            })
        });
    }

    group.finish();
}

fn bench_channel_distribution_scale(c: &mut Criterion) {
    use literbike::kafka_replacement_smoke::ChannelizedDistributor;

    let mut group = c.benchmark_group("channel_distribution_scale");

    for num_channels in [2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_channels),
            num_channels,
            |b, &channels| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let (distributor, _receivers) = ChannelizedDistributor::new(channels, 100);

                b.iter(|| {
                    rt.block_on(async {
                        let tick = MarketTick::new("BTC/USD", 45000.0, 1.5, black_box(1));
                        distributor.distribute(&tick).await.unwrap();
                    });
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// Criterion groups
// ============================================================================

criterion_group!(
    benches,
    bench_duckdb_append,
    bench_duckdb_query,
    bench_quic_stream_throughput,
    bench_broadcast_latency,
    bench_async_channel_distribution,
    bench_end_to_end_latency,
    bench_memory_per_message,
    bench_ccek_overhead,
    bench_duckdb_append_batch,
    bench_channel_distribution_scale,
);

criterion_main!(benches);
