use literbike::quic::quic_protocol::{
    deserialize_packet, serialize_packet, ConnectionId, QuicFrame, QuicHeader, QuicPacket,
    QuicPacketType, StreamFrame,
};
use std::env;
use std::net::{ToSocketAddrs, UdpSocket};
use std::time::Duration;

async fn fetch_resource(
    socket: &UdpSocket,
    host: &str,
    path: &str,
    stream_id: u64,
    verbose: bool,
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

    let request_str = format!("GET {} HTTP/3\r\nHost: {}\r\n\r\n", path, host);
    if verbose {
        println!("> {}", request_str.replace("\r\n", "\n> "));
    }

    let frames = vec![QuicFrame::Stream(StreamFrame {
        stream_id,
        offset: 0,
        fin: false,
        data: request_str.into_bytes(),
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
            Err(_) => break, // Timeout or error
        }
    }
    Ok(data)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    let mut url_str = "http://localhost:4433/";
    let mut verbose = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-v" | "--verbose" => verbose = true,
            arg if !arg.starts_with("-") => url_str = arg,
            _ => {
                eprintln!("Usage: quic_curl [-v|--verbose] [URL]");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let url = if url_str.starts_with("http://") || url_str.starts_with("https://") {
        url_str.to_string()
    } else {
        format!("https://{}", url_str)
    };

    let url_parts: Vec<&str> = url.splitn(4, '/').collect();
    let host_port = url_parts.get(2).unwrap_or(&"localhost:4433");
    let host = host_port.split(':').next().unwrap_or("localhost");
    let port = host_port.split(':').nth(1).unwrap_or("4433");
    let addr_str = format!("{}:{}", host, port);

    let path = if url_parts.len() > 3 {
        format!("/{}", url_parts[3])
    } else {
        "/".to_string()
    };

    println!("* Connecting to {}...", addr_str);

    let addrs: Vec<_> = addr_str.to_socket_addrs()?.collect();
    if addrs.is_empty() {
        return Err(format!("Could not resolve {}", addr_str).into());
    }

    let target_addr = addrs[0];
    let bind_addr = if target_addr.is_ipv6() {
        "[::]:0"
    } else {
        "0.0.0.0:0"
    };

    let socket = UdpSocket::bind(bind_addr)?;
    socket.connect(target_addr)?;
    socket.set_read_timeout(Some(Duration::from_millis(500)))?;

    println!("* Connected via QUIC/UDP to {}", target_addr);

    let response = fetch_resource(&socket, host, &path, 1, verbose).await?;

    if verbose {
        println!("< [Received {} bytes]", response.len());
        println!("<");
    }

    // Try to print as string, otherwise just binary indication
    match String::from_utf8(response.clone()) {
        Ok(s) => print!("{}", s),
        Err(_) => println!("[Binary data: {} bytes]", response.len()),
    }

    Ok(())
}
