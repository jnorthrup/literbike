//! Test 1.1.3: Frame Types - Additional coverage for all QuicFrame variants

use literbike::quic::*;
use anyhow::Result;

// ============================================================================
// Test Stream Frame variants
// ============================================================================

#[test]
fn test_stream_frame_variants() -> Result<()> {
    // Test stream frame with offset 0
    let frame1 = QuicFrame::Stream(StreamFrame {
        stream_id: 0,
        offset: 0,
        data: vec![1, 2, 3],
        fin: false,
    });

    let serialized = bincode::serialize(&frame1)?;
    let deserialized: QuicFrame = bincode::deserialize(&serialized)?;
    
    if let QuicFrame::Stream(sf) = deserialized {
        assert_eq!(sf.stream_id, 0);
        assert_eq!(sf.offset, 0);
    }

    // Test stream frame with FIN
    let frame2 = QuicFrame::Stream(StreamFrame {
        stream_id: 1,
        offset: 100,
        data: vec![],
        fin: true,
    });

    let serialized = bincode::serialize(&frame2)?;
    let deserialized: QuicFrame = bincode::deserialize(&serialized)?;
    
    if let QuicFrame::Stream(sf) = deserialized {
        assert!(sf.fin);
        assert_eq!(sf.data.len(), 0);
    }

    // Test stream frame with large offset
    let frame3 = QuicFrame::Stream(StreamFrame {
        stream_id: 2,
        offset: u64::MAX,
        data: vec![0xFF],
        fin: false,
    });

    let serialized = bincode::serialize(&frame3)?;
    let deserialized: QuicFrame = bincode::deserialize(&serialized)?;
    
    if let QuicFrame::Stream(sf) = deserialized {
        assert_eq!(sf.offset, u64::MAX);
    }

    Ok(())
}

// ============================================================================
// Test Ack Frame variants
// ============================================================================

#[test]
fn test_ack_frame_variants() -> Result<()> {
    // Test ack with single range
    let frame1 = QuicFrame::Ack(AckFrame {
        largest_acknowledged: 10,
        ack_delay: 0,
        ack_ranges: vec![(5, 10)],
    });

    let serialized = bincode::serialize(&frame1)?;
    let deserialized: QuicFrame = bincode::deserialize(&serialized)?;
    
    if let QuicFrame::Ack(af) = deserialized {
        assert_eq!(af.largest_acknowledged, 10);
        assert_eq!(af.ack_ranges.len(), 1);
    }

    // Test ack with multiple ranges (gaps)
    let frame2 = QuicFrame::Ack(AckFrame {
        largest_acknowledged: 20,
        ack_delay: 1000,
        ack_ranges: vec![(15, 20), (10, 12), (5, 8)],
    });

    let serialized = bincode::serialize(&frame2)?;
    let deserialized: QuicFrame = bincode::deserialize(&serialized)?;
    
    if let QuicFrame::Ack(af) = deserialized {
        assert_eq!(af.ack_ranges.len(), 3);
        assert_eq!(af.ack_delay, 1000);
    }

    // Test ack with zero delay
    let frame3 = QuicFrame::Ack(AckFrame {
        largest_acknowledged: 5,
        ack_delay: 0,
        ack_ranges: vec![(0, 5)],
    });

    let serialized = bincode::serialize(&frame3)?;
    let deserialized: QuicFrame = bincode::deserialize(&serialized)?;
    
    if let QuicFrame::Ack(af) = deserialized {
        assert_eq!(af.ack_delay, 0);
    }

    Ok(())
}

// ============================================================================
// Test Crypto Frame variants
// ============================================================================

#[test]
fn test_crypto_frame_variants() -> Result<()> {
    // Test crypto frame with offset 0
    let frame1 = QuicFrame::Crypto(CryptoFrame {
        offset: 0,
        data: vec![0x01, 0x02, 0x03],
    });

    let serialized = bincode::serialize(&frame1)?;
    let deserialized: QuicFrame = bincode::deserialize(&serialized)?;
    
    if let QuicFrame::Crypto(cf) = deserialized {
        assert_eq!(cf.offset, 0);
        assert_eq!(cf.data.len(), 3);
    }

    // Test crypto frame with large offset
    let frame2 = QuicFrame::Crypto(CryptoFrame {
        offset: 1000000,
        data: vec![0xAA; 1000],
    });

    let serialized = bincode::serialize(&frame2)?;
    let deserialized: QuicFrame = bincode::deserialize(&serialized)?;
    
    if let QuicFrame::Crypto(cf) = deserialized {
        assert_eq!(cf.offset, 1000000);
        assert_eq!(cf.data.len(), 1000);
    }

    // Test crypto frame with empty data
    let frame3 = QuicFrame::Crypto(CryptoFrame {
        offset: 500,
        data: vec![],
    });

    let serialized = bincode::serialize(&frame3)?;
    let deserialized: QuicFrame = bincode::deserialize(&serialized)?;
    
    if let QuicFrame::Crypto(cf) = deserialized {
        assert_eq!(cf.data.len(), 0);
    }

    Ok(())
}

// ============================================================================
// Test Padding Frame variants
// ============================================================================

#[test]
fn test_padding_frame_variants() -> Result<()> {
    // Test padding with 0 bytes
    let frame1 = QuicFrame::Padding(0);
    let serialized = bincode::serialize(&frame1)?;
    let deserialized: QuicFrame = bincode::deserialize(&serialized)?;
    
    if let QuicFrame::Padding(len) = deserialized {
        assert_eq!(len, 0);
    }

    // Test padding with max reasonable value
    let frame2 = QuicFrame::Padding(1500);
    let serialized = bincode::serialize(&frame2)?;
    let deserialized: QuicFrame = bincode::deserialize(&serialized)?;
    
    if let QuicFrame::Padding(len) = deserialized {
        assert_eq!(len, 1500);
    }

    Ok(())
}

// ============================================================================
// Test mixed frame sequences
// ============================================================================

#[test]
fn test_mixed_frame_sequence() -> Result<()> {
    let frames = vec![
        QuicFrame::Crypto(CryptoFrame {
            offset: 0,
            data: vec![0x01, 0x02],
        }),
        QuicFrame::Padding(10),
        QuicFrame::Stream(StreamFrame {
            stream_id: 1,
            offset: 0,
            data: vec![0xDE, 0xAD],
            fin: false,
        }),
        QuicFrame::Ack(AckFrame {
            largest_acknowledged: 5,
            ack_delay: 100,
            ack_ranges: vec![(0, 5)],
        }),
        QuicFrame::Padding(5),
    ];

    // Serialize all frames
    let serialized = bincode::serialize(&frames)?;
    
    // Deserialize
    let deserialized: Vec<QuicFrame> = bincode::deserialize(&serialized)?;
    
    // Verify count
    assert_eq!(deserialized.len(), 5);

    // Verify each frame type
    assert!(matches!(deserialized[0], QuicFrame::Crypto(_)));
    assert!(matches!(deserialized[1], QuicFrame::Padding(_)));
    assert!(matches!(deserialized[2], QuicFrame::Stream(_)));
    assert!(matches!(deserialized[3], QuicFrame::Ack(_)));
    assert!(matches!(deserialized[4], QuicFrame::Padding(_)));

    Ok(())
}
