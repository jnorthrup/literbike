use literbike::quic::quic_protocol::{serialize_packet, deserialize_packet, QuicPacket, QuicHeader, QuicFrame, StreamFrame, ConnectionId, QuicPacketType};
use std::net::UdpSocket;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("127.0.0.1:4433")?;
    socket.set_read_timeout(Some(Duration::from_millis(2000)))?;

    println!("🚀 Testing simple QUIC stream request to stream 0");

    // Send a simple HTTP/3 request on stream 0
    let header = QuicHeader {
        r#type: QuicPacketType::ShortHeader,
        version: 1,
        destination_connection_id: ConnectionId { bytes: vec![1, 2, 3, 4, 5, 6, 7, 8] },
        source_connection_id: ConnectionId { bytes: vec![2, 2, 2, 2, 2, 2, 2, 2] },
        packet_number: 1,
        token: None,
    };

    // HTTP/3 request format (simplified)
    let http_request = b"GET / HTTP/3\r\nHost: localhost\r\n\r\n".to_vec();

    let frames = vec![QuicFrame::Stream(StreamFrame {
        stream_id: 0,  // Use stream 0 as expected by server
        offset: 0,
        fin: true,
        data: http_request,
    })];

    let packet = QuicPacket { header, frames, payload: Vec::new() };
    let serialized = serialize_packet(&packet)?;
    println!("📤 Sending {} bytes to server", serialized.len());
    socket.send(&serialized)?;

    let mut buf = [0u8; 65536];
    let mut received_data = Vec::new();

    println!("📥 Waiting for response...");
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        match socket.recv(&mut buf) {
            Ok(len) => {
                println!("📡 Received {} bytes", len);
                if let Ok(p) = deserialize_packet(&buf[..len]) {
                    println!("📦 Got packet with {} frames", p.frames.len());
                    for frame in p.frames {
                        match frame {
                            QuicFrame::Stream(s) => {
                                println!("📄 Stream frame: stream_id={}, offset={}, fin={}, data_len={}",
                                    s.stream_id, s.offset, s.fin, s.data.len());
                                received_data.extend_from_slice(&s.data);
                            }
                            QuicFrame::ConnectionClose(cc) => {
                                println!("⚠️  Connection close: error_code={}", cc.error_code);
                            }
                            other => {
                                println!("❓ Other frame: {:?}", other);
                            }
                        }
                    }
                } else {
                    println!("❌ Failed to deserialize packet");
                }
            }
            Err(_) => {
                println!("⏰ Timeout waiting for response");
                break;
            }
        }
    }

    if !received_data.is_empty() {
        println!("\n✅ Received {} bytes of response data", received_data.len());
        if let Ok(text) = String::from_utf8(received_data.clone()) {
            println!("Response (UTF-8):\n{}", text);
        } else {
            println!("Response (hex): {:02x?}", received_data);
        }
    } else {
        println!("\n❌ No response data received");
    }

    Ok(())
}
