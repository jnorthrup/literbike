use literbike::quic::quic_protocol::{
    deserialize_packet, serialize_packet, ConnectionId, QuicFrame, QuicHeader, QuicPacket,
    QuicPacketType, StreamFrame,
};
use std::net::UdpSocket;
use std::time::Duration;

async fn fetch_resource(
    socket: &UdpSocket,
    path: &str,
    stream_id: u64,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let header = QuicHeader {
        r#type: QuicPacketType::ShortHeader,
        version: 1,
        destination_connection_id: ConnectionId {
            bytes: vec![1, 2, 3, 4, 5, 6, 7, 8],
        },
        source_connection_id: ConnectionId {
            bytes: vec![2, 2, 2, 2, 2, 2, 2, 2],
        },
        packet_number: stream_id * 100, // Just a unique PN
        token: None,
    };

    let frames = vec![QuicFrame::Stream(StreamFrame {
        stream_id,
        offset: 0,
        fin: false,
        data: format!("GET {} HTTP/3\r\nHost: localhost\r\n\r\n", path).into_bytes(),
    })];

    let packet = QuicPacket {
        header,
        frames,
        payload: Vec::new(),
    };
    let serialized = serialize_packet(&packet)?;
    socket.send(&serialized)?;

    let mut buf = [0u8; 65536];
    let mut data = Vec::new();

    // For large files (like the PNG), we need to read many packets
    // Our VQA server currently sends the whole file in one stream frame,
    // which might be split into multiple UDP packets by the network stack or QUIC layer.
    // However, our FOUNDATIONAL codec currently tries to fit it in one if possible?
    // Actually, std::fs::read returns 554kb, which is way too big for one UDP packet.
    // Our QuicEngine::send_stream_data MUST be splitting it.

    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        match socket.recv(&mut buf) {
            Ok(len) => {
                if let Ok(p) = deserialize_packet(&buf[..len]) {
                    for frame in p.frames {
                        if let QuicFrame::Stream(s) = frame {
                            if s.stream_id == stream_id {
                                data.extend_from_slice(&s.data);
                            }
                        }
                    }
                } else {
                    // Raw fallback
                }
            }
            Err(_) => break,
        }
    }
    Ok(data)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("127.0.0.1:4433")?;
    socket.set_read_timeout(Some(Duration::from_millis(500)))?;

    println!("🚀 Starting QUIC TDD Full Stack Verification (Robust)");

    // 1. Fetch index.html
    let html = fetch_resource(&socket, "/", 1).await?;
    println!("✅ index.html: {} bytes", html.len());

    // 2. Fetch index.css
    let css = fetch_resource(&socket, "/index.css", 3).await?;
    println!("✅ index.css: {} bytes", css.len());

    // 3. Fetch bw_test_pattern.png
    println!("📥 Requesting PNG (this might take a second)...");
    let png = fetch_resource(&socket, "/bw_test_pattern.png", 5).await?;
    println!("✅ bw_test_pattern.png: {} bytes", png.len());

    if html.len() > 100 && css.len() > 100 && png.len() > 10000 {
        println!("\n✨✨ ALL ASSETS VERIFIED OVER QUIC ✨✨");
        println!("The server is READY for Chrome QA.");
    } else {
        println!("\n❌ Verification failed. Missing or small assets.");
        if png.len() == 0 {
            println!("   TIP: Check if bw_test_pattern.png is splitting across many packets that the codec fails to handle.");
        }
    }

    Ok(())
}
