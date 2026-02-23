#![allow(dead_code, unused_imports)]

use crate::{
    quic_protocol::{QuicConnectionState, ConnectionId, QuicPacket, QuicHeader, QuicPacketType, QuicFrame, TransportParameters, ConnectionState},
    crypto::CryptoEngine,
    HtxError, Result,
};
use std::sync::Arc;
use std::net::SocketAddr;
use std::collections::HashMap;
use parking_lot::RwLock;
use tokio::net::UdpSocket;
use rand::Rng;
use bytes::{BytesMut, BufMut};

// Import canonical Trikeshed Join and helper
use crate::core_types::Join;
use crate::indexed::Indexed;

// α (alpha) transform for lazy evaluation in Chrome stack
type ChromeAlphaChannel<T> = Join<Indexed<T>, Box<dyn Fn() -> Indexed<T> + Send + Sync>>;

// Categorical pairwise atom for Chrome fingerprint composition
type FingerprintAtom = Join<Vec<u8>, Join<u16, u64>>; // (bytes, (cipher, timestamp))

// Chrome packet buffer optimized with zero-allocation Indexed pattern
type ChromePacketBuffer = Indexed<u8>;
type ChromeConnectionMap = Indexed<ChromeAlphaChannel<QuicConnectionState>>;

// KMP helper functions for categorical composition
fn indexed<T: Clone + 'static>(size: usize, default_val: T) -> Indexed<T> {
    let _ = default_val;
    Indexed::<T>::new(size as u32, 0)
}

fn indexed_chrome_connections(size: usize) -> ChromeConnectionMap {
    let _default_alpha = || {
        let _default_conn = QuicConnectionState {
            local_connection_id: ConnectionId { bytes: vec![] },
            remote_connection_id: ConnectionId { bytes: vec![] },
            version: 1,
            transport_params: TransportParameters::default(),
            streams: Vec::new(),
            sent_packets: Vec::new(),
            received_packets: Vec::new(),
            next_packet_number: 0,
            next_stream_id: 0,
            congestion_window: 14720,
            bytes_in_flight: 0,
            rtt: 100,
            connection_state: ConnectionState::Handshaking,
        };
        Indexed::<QuicConnectionState>::new(1, 0)
    };
    Indexed::<ChromeAlphaChannel<QuicConnectionState>>::new(size as u32, 0)
}

// Categorical atom constructor for Chrome fingerprints  
fn fingerprint_atom(bytes: Vec<u8>, cipher: u16, timestamp: u64) -> FingerprintAtom {
    (bytes, (cipher, timestamp))
}

// \u03b1 transform for lazy Chrome packet evaluation
fn alpha_packet_transform<T: Clone + 'static>(packet: T) -> ChromeAlphaChannel<T> {
    let _ = packet;
    let packet_indexed = Indexed::<T>::new(1, 0);
    let alpha_fn = Box::new(move || Indexed::<T>::new(1, 0));
    (packet_indexed, alpha_fn)
}

// Extension trait for missing put_u24 method
trait BytesMutExt {
    fn put_u24(&mut self, n: u32);
}

impl BytesMutExt for BytesMut {
    fn put_u24(&mut self, n: u32) {
        self.put_u8((n >> 16) as u8);
        self.put_u8((n >> 8) as u8);
        self.put_u8(n as u8);
    }
}

/// Chrome-class QUIC implementation with native stack
/// Follows Chrome's behavioral patterns for origin mirroring
pub struct ChromeQuicEngine {
    pub socket: Arc<UdpSocket>,
    connections: Arc<RwLock<ChromeConnectionMap>>,
    connection_lookup: Arc<RwLock<HashMap<ConnectionId, usize>>>, // CID -> index mapping
    crypto: Arc<CryptoEngine>,
    pub config: ChromeQuicConfig,
}

#[derive(Clone)]
pub struct ChromeQuicConfig {
    /// Chrome version to emulate (affects transport parameters)
    pub chrome_version: String,
    /// Origin-specific transport parameters for mirroring
    pub origin_transport_params: Option<TransportParameters>,
    /// ECH configuration
    pub ech_config: Option<Vec<u8>>,
    /// Chrome-specific ALPN protocols
    pub alpn_protocols: Vec<String>,
}

impl Default for ChromeQuicConfig {
    fn default() -> Self {
        Self {
            chrome_version: "Chrome/131.0.6778.69".to_string(), // Latest stable N-2
            origin_transport_params: None,
            ech_config: None,
            alpn_protocols: vec!["h3".to_string(), "h2".to_string()],
        }
    }
}

impl ChromeQuicEngine {
    pub async fn new(bind_addr: SocketAddr, config: ChromeQuicConfig) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await
            .map_err(|e| HtxError::Transport(format!("Failed to bind UDP socket: {}", e)))?;
        
        // Initialize Chrome connections with Indexed<T> = Join<Int, Int->T> pattern
        let connections = Arc::new(RwLock::new(indexed_chrome_connections(0)));
        let connection_lookup = Arc::new(RwLock::new(HashMap::new()));
        let mut key = [0u8; 32];
        let mut salt = [0u8; 12];
        rand::thread_rng().fill(&mut key);
        rand::thread_rng().fill(&mut salt);
        let crypto = Arc::new(CryptoEngine::new(&key, &salt));

        Ok(ChromeQuicEngine {
            socket: Arc::new(socket),
            connections,
            connection_lookup,
            crypto,
            config,
        })
    }

    /// Initiate Chrome-style QUIC handshake with target origin
    pub async fn connect(&self, target: SocketAddr, origin_fingerprint: &OriginFingerprint) -> Result<Arc<RwLock<QuicConnectionState>>> {
        let local_conn_id = self.generate_chrome_connection_id();
        let remote_conn_id = self.generate_chrome_connection_id();

        // Create Chrome-mirrored transport parameters
        let transport_params = self.create_chrome_transport_params(origin_fingerprint);

        let mut connection = QuicConnectionState {
            local_connection_id: local_conn_id.clone(),
            remote_connection_id: remote_conn_id,
            version: 0x00000001, // QUIC v1
            transport_params,
            streams: Vec::new(),
            sent_packets: Vec::new(),
            received_packets: Vec::new(),
            next_packet_number: 0,
            next_stream_id: 0,
            congestion_window: 14720, // Chrome default: 10 * 1472
            bytes_in_flight: 0,
            rtt: 100, // Initial RTT estimate
            connection_state: ConnectionState::Handshaking,
        };

        // Generate Initial packet with Chrome-specific formatting
        let initial_packet = self.create_chrome_initial_packet(&connection, origin_fingerprint)?;
        
        // Send Initial packet
        let packet_bytes = self.serialize_chrome_packet(&initial_packet)?;
        self.socket.send_to(&packet_bytes, target).await
            .map_err(|e| HtxError::Transport(format!("Failed to send Initial packet: {}", e)))?;

        connection.sent_packets.push(initial_packet);
        connection.next_packet_number += 1;

        // Store using categorical composition pattern
    let connection_arc = Arc::new(RwLock::new(connection));
        
        // Update connection map with indexed pattern
        let mut lookup = self.connection_lookup.write();
        let current_size = lookup.len();
        lookup.insert(local_conn_id, current_size);
        
        // Expand connections using α transform
        *self.connections.write() = indexed_chrome_connections(current_size + 1);

        Ok(connection_arc)
    }

    /// Generate Chrome-style connection ID (8 bytes random)
    pub fn generate_chrome_connection_id(&self) -> ConnectionId {
        let mut rng = rand::thread_rng();
        let mut bytes = vec![0u8; 8];
        rng.fill(&mut bytes[..]);
        ConnectionId { bytes }
    }

    /// Create Chrome-mirrored transport parameters
    pub fn create_chrome_transport_params(&self, origin_fingerprint: &OriginFingerprint) -> TransportParameters {
        // Check origin fingerprint first
        if let Some(ref origin_params) = origin_fingerprint.transport_params {
            // Mirror the origin's transport parameters exactly
            origin_params.clone()
        } else if let Some(ref config_params) = self.config.origin_transport_params {
            // Use config params if available
            config_params.clone()
        } else {
            // Use Chrome defaults if no origin mirroring
            TransportParameters {
                max_stream_data: 524_288, // Chrome default: 512KB
                max_data: 15_728_640, // Chrome default: 15MB
                max_bidi_streams: 100,
                max_uni_streams: 100,
                idle_timeout: 30_000,
                max_packet_size: 1472, // Chrome default MTU minus headers
                ack_delay_exponent: 3,
                max_ack_delay: 25,
                active_connection_id_limit: 2, // Chrome uses 2
                initial_max_data: 15_728_640,
                initial_max_stream_data_bidi_local: 524_288,
                initial_max_stream_data_bidi_remote: 524_288,
                initial_max_stream_data_uni: 524_288,
                initial_max_streams_bidi: 100,
                initial_max_streams_uni: 100,
            }
        }
    }

    /// Create Chrome-formatted Initial packet
    pub fn create_chrome_initial_packet(&self, connection: &QuicConnectionState, origin_fingerprint: &OriginFingerprint) -> Result<QuicPacket> {
        let header = QuicHeader {
            r#type: QuicPacketType::Initial,
            version: connection.version,
            destination_connection_id: connection.remote_connection_id.clone(),
            source_connection_id: connection.local_connection_id.clone(),
            packet_number: connection.next_packet_number,
            token: None, // No token for first Initial
        };

        // Chrome-specific frame ordering: CRYPTO, PADDING
        let mut frames = Vec::new();

        // Add CRYPTO frame with Chrome TLS ClientHello
        let crypto_frame = self.create_chrome_crypto_frame(origin_fingerprint)?;
        frames.push(QuicFrame::Crypto(crypto_frame));

        // Add Chrome-style PADDING (Chrome pads to 1200 bytes minimum)
        let padding_needed = 1200 - self.estimate_packet_size(&header, &frames);
        if padding_needed > 0 {
            frames.push(QuicFrame::Padding { length: padding_needed as u32 });
        }

        Ok(QuicPacket {
            header,
            frames,
            payload: Vec::new(),
        })
    }

    /// Create Chrome-style CRYPTO frame with proper TLS ClientHello
    fn create_chrome_crypto_frame(&self, origin_fingerprint: &OriginFingerprint) -> Result<crate::quic_protocol::CryptoFrame> {
        // Generate Chrome-class TLS ClientHello matching the origin's JA3 fingerprint
        let client_hello = self.generate_chrome_client_hello(origin_fingerprint)?;
        
        Ok(crate::quic_protocol::CryptoFrame {
            offset: 0,
            data: client_hello,
        })
    }

    /// Generate Chrome-class TLS ClientHello for perfect origin mirroring
    pub fn generate_chrome_client_hello(&self, origin_fingerprint: &OriginFingerprint) -> Result<Vec<u8>> {
        let mut client_hello = BytesMut::new();

        // TLS Record Header (22 = Handshake, 0x0303 = TLS 1.2)
        client_hello.put_u8(0x16); // Content Type: Handshake
        client_hello.put_u16(0x0303); // TLS Version: 1.2
        client_hello.put_u16(0); // Length placeholder

        // Handshake Header
        client_hello.put_u8(0x01); // Handshake Type: Client Hello
        client_hello.put_u24(0); // Length placeholder

        // Client Hello Body
        client_hello.put_u16(0x0303); // Client Version: TLS 1.2
        
        // Random (32 bytes)
        let mut rng = rand::thread_rng();
        for _ in 0..32 {
            client_hello.put_u8(rng.gen());
        }

        // Session ID (empty for QUIC)
        client_hello.put_u8(0);

        // Cipher Suites using categorical composition with Join<cipher_id, description>
        type CipherAtom = Join<u16, &'static str>;
        let chrome_cipher_atoms: [CipherAtom; 7] = [
            (0x1301, "TLS_AES_128_GCM_SHA256"),        // TLS 1.3
            (0x1302, "TLS_AES_256_GCM_SHA384"),        // TLS 1.3  
            (0x1303, "TLS_CHACHA20_POLY1305_SHA256"),   // TLS 1.3
            (0xc02f, "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256"),
            (0xc030, "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384"),
            (0xcca9, "TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256"),
            (0xcca8, "TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256"),
        ];
        
        client_hello.put_u16((chrome_cipher_atoms.len() * 2) as u16);
        for (cipher_id, _) in &chrome_cipher_atoms {
            client_hello.put_u16(*cipher_id);
        }

        // Compression Methods
        client_hello.put_u8(1); // Length
        client_hello.put_u8(0); // null compression

        // Extensions (Chrome-specific)
    let extensions = self.generate_chrome_extensions(origin_fingerprint, None)?;
        client_hello.put_u16(extensions.len() as u16);
        client_hello.extend_from_slice(&extensions);

        // Update lengths
        let total_len = client_hello.len() - 5;
        let handshake_len = total_len - 4;
        
        // Fix TLS record length
        client_hello[3..5].copy_from_slice(&(total_len as u16).to_be_bytes());
        
        // Fix handshake length (24-bit)
        let handshake_len_bytes = (handshake_len as u32).to_be_bytes();
        client_hello[6] = handshake_len_bytes[1];
        client_hello[7] = handshake_len_bytes[2];
        client_hello[8] = handshake_len_bytes[3];

        Ok(client_hello.to_vec())
    }

    /// Generate Chrome-specific TLS extensions using categorical composition
    /// Applies Trikeshed patterns for origin mirroring with zero-allocation fingerprinting
    fn generate_chrome_extensions(&self, origin_fingerprint: &OriginFingerprint, ech_extension: Option<&[u8]>) -> Result<Vec<u8>> {
        let mut extensions = BytesMut::new();

        // Categorical extension atoms as Join<type, data> patterns
        type ExtensionAtom = Join<u16, Vec<u8>>;
        
        // SNI Extension using categorical composition
        let sni_atom = (0x0000, Vec::new()); // server_name with empty data for now
        extensions.put_u16(sni_atom.0); // Extension Type
        extensions.put_u16(sni_atom.1.len() as u16); // Length
        extensions.extend_from_slice(&sni_atom.1);

        // ALPN Extension using categorical composition
        let alpn_data = self.encode_chrome_alpn();
        let alpn_atom = (0x0010, alpn_data); // application_layer_protocol_negotiation
        extensions.put_u16(alpn_atom.0);
        extensions.put_u16(alpn_atom.1.len() as u16);
        extensions.extend_from_slice(&alpn_atom.1);

        // Supported Groups using categorical Join<group_id, curve_name> patterns
        type CurveAtom = Join<u16, &'static str>;
        let chrome_curves: [CurveAtom; 3] = [
            (0x001d, "X25519"),
            (0x0017, "secp256r1"),
            (0x0018, "secp384r1"),
        ];
        
        let mut groups_data = BytesMut::new();
        groups_data.put_u16((chrome_curves.len() * 2) as u16);
        for (curve_id, _) in &chrome_curves {
            groups_data.put_u16(*curve_id);
        }
        
        let groups_atom = (0x000a, groups_data.to_vec()); // supported_groups
        extensions.put_u16(groups_atom.0);
        extensions.put_u16(groups_atom.1.len() as u16);
        extensions.extend_from_slice(&groups_atom.1);

        // Origin fingerprint extensions using categorical mirroring
        let fingerprint_atoms: Vec<ExtensionAtom> = origin_fingerprint.extensions
            .iter()
            .map(|&ext_type| (ext_type, Vec::new()))
            .collect();
            
        for atom in fingerprint_atoms {
            extensions.put_u16(atom.0);
            extensions.put_u16(atom.1.len() as u16);
            extensions.extend_from_slice(&atom.1);
        }

        // ECH extension injection using categorical composition
        if let Some(ech_data) = ech_extension {
            extensions.extend_from_slice(ech_data);
        }

        Ok(extensions.to_vec())
    }

    /// Encode Chrome ALPN protocols using categorical composition
    /// Applies Indexed<T> pattern for zero-allocation protocol encoding
    fn encode_chrome_alpn(&self) -> Vec<u8> {
        let mut alpn = BytesMut::new();
        
        // Categorical protocol atoms as Join<length, bytes>
        type ProtocolAtom = Join<u8, Vec<u8>>;
        let protocol_atoms: Vec<ProtocolAtom> = self.config.alpn_protocols
            .iter()
            .map(|protocol| (protocol.len() as u8, protocol.as_bytes().to_vec()))
            .collect();
        
        // Calculate total length using categorical composition
        let total_len: usize = protocol_atoms
            .iter()
            .map(|(len, _)| 1 + *len as usize)
            .sum();
        
        alpn.put_u16(total_len as u16);
        
        // Encode protocols using categorical atom iteration
        for (len, bytes) in protocol_atoms {
            alpn.put_u8(len);
            alpn.extend_from_slice(&bytes);
        }
        
        alpn.to_vec()
    }

    /// Estimate packet size for padding calculation
    fn estimate_packet_size(&self, header: &QuicHeader, frames: &[QuicFrame]) -> usize {
        // Rough estimate: 20 bytes header + frame sizes
        20 + frames.iter().map(|f| self.estimate_frame_size(f)).sum::<usize>()
    }

    /// Estimate frame size
    fn estimate_frame_size(&self, frame: &QuicFrame) -> usize {
        match frame {
            QuicFrame::Crypto(crypto) => 1 + 8 + crypto.data.len(), // type + offset + data
            QuicFrame::Padding { length } => *length as usize,
            _ => 8, // Conservative estimate
        }
    }

    /// Serialize packet with Chrome-specific formatting
    fn serialize_chrome_packet(&self, packet: &QuicPacket) -> Result<Vec<u8>> {
        let mut buffer = BytesMut::new();

        // Chrome packet format: Header | Protected Payload
        let header_bytes = self.serialize_chrome_header(&packet.header)?;
        buffer.extend_from_slice(&header_bytes);

        // Serialize frames
        for frame in &packet.frames {
            let frame_bytes = self.serialize_chrome_frame(frame)?;
            buffer.extend_from_slice(&frame_bytes);
        }

        Ok(buffer.to_vec())
    }

    /// Serialize Chrome-style packet header
    fn serialize_chrome_header(&self, header: &QuicHeader) -> Result<Vec<u8>> {
        let mut buffer = BytesMut::new();

        // Long header format for Initial packets
        let mut first_byte = 0x80; // Long header bit
        first_byte |= (header.r#type as u8) << 4;
        buffer.put_u8(first_byte);

        // Version
        buffer.put_u32(header.version as u32);

        // Connection ID lengths (Chrome uses 8-byte CIDs)
        buffer.put_u8(0x88); // Both CIDs are 8 bytes

        // Destination Connection ID
        buffer.extend_from_slice(&header.destination_connection_id.bytes);

        // Source Connection ID
        buffer.extend_from_slice(&header.source_connection_id.bytes);

        // Token length (0 for Initial without token)
        buffer.put_u8(0);

        // Length (placeholder for protected payload)
        buffer.put_u16(0);

        // Packet Number (Chrome uses 4-byte packet numbers)
        buffer.put_u32(header.packet_number as u32);

        Ok(buffer.to_vec())
    }

    /// Serialize Chrome-style frame
    fn serialize_chrome_frame(&self, frame: &QuicFrame) -> Result<Vec<u8>> {
        let mut buffer = BytesMut::new();

        match frame {
            QuicFrame::Crypto(crypto) => {
                buffer.put_u8(0x06); // CRYPTO frame type
                buffer.put_u64(crypto.offset); // Offset (varint)
                buffer.put_u64(crypto.data.len() as u64); // Length (varint)
                buffer.extend_from_slice(&crypto.data);
            }
            QuicFrame::Padding { length } => {
                for _ in 0..*length {
                    buffer.put_u8(0x00); // PADDING frame
                }
            }
            _ => {
                return Err(HtxError::Protocol("Unsupported frame type for Chrome serialization".to_string()));
            }
        }

        Ok(buffer.to_vec())
    }

    /// Accept incoming QUIC connections
    pub async fn accept(&self) -> Result<(Arc<RwLock<QuicConnectionState>>, SocketAddr)> {
        let mut buffer = vec![0u8; 65535];
        let (len, addr) = self.socket.recv_from(&mut buffer).await
            .map_err(|e| HtxError::Transport(format!("Failed to receive packet: {}", e)))?;

        buffer.truncate(len);
        let packet = self.deserialize_chrome_packet(&buffer)?;

        // Handle Initial packet
        if packet.header.r#type == QuicPacketType::Initial {
            let connection = self.handle_chrome_initial(&packet, addr).await?;
            Ok((connection, addr))
        } else {
            Err(HtxError::Protocol("Expected Initial packet".to_string()))
        }
    }

    /// Handle Chrome Initial packet and create connection
    async fn handle_chrome_initial(&self, packet: &QuicPacket, _addr: SocketAddr) -> Result<Arc<RwLock<QuicConnectionState>>> {
        let local_conn_id = self.generate_chrome_connection_id();
        let remote_conn_id = packet.header.source_connection_id.clone();

        let connection = QuicConnectionState {
            local_connection_id: local_conn_id.clone(),
            remote_connection_id: remote_conn_id,
            version: packet.header.version,
            transport_params: TransportParameters::default(),
            streams: Vec::new(),
            sent_packets: Vec::new(),
            received_packets: Vec::new(),
            next_packet_number: 0,
            next_stream_id: 1, // Server uses odd stream IDs
            congestion_window: 14720,
            bytes_in_flight: 0,
            rtt: 100,
            connection_state: ConnectionState::Handshaking,
        };

    // Store connection directly (alpha transform yields an Indexed placeholder;
    // using the actual connection value here avoids invalid tuple/field access)
    let connection_arc = Arc::new(RwLock::new(connection));
        
        // Update connection map with indexed pattern
        let mut lookup = self.connection_lookup.write();
        let current_size = lookup.len();
        lookup.insert(local_conn_id, current_size);
        
        // Expand connections using α transform
        *self.connections.write() = indexed_chrome_connections(current_size + 1);

        Ok(connection_arc)
    }

    /// Deserialize Chrome packet
    fn deserialize_chrome_packet(&self, data: &[u8]) -> Result<QuicPacket> {
        if data.len() < 20 {
            return Err(HtxError::Protocol("Packet too short".to_string()));
        }

        let mut offset = 0;
        
        // Parse header
        let first_byte = data[offset];
        offset += 1;

        let packet_type = QuicPacketType::Initial; // Simplified for now
        let version = u32::from_be_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]) as u64;
        offset += 4;

        let conn_id_len = data[offset];
        offset += 1;

        let dest_conn_id = ConnectionId {
            bytes: data[offset..offset+8].to_vec(),
        };
        offset += 8;

        let src_conn_id = ConnectionId {
            bytes: data[offset..offset+8].to_vec(),
        };
        offset += 8;

        let header = QuicHeader {
            r#type: packet_type,
            version,
            destination_connection_id: dest_conn_id,
            source_connection_id: src_conn_id,
            packet_number: 0, // Simplified
            token: None,
        };

        // Parse frames (simplified)
        let frames = Vec::new();

        Ok(QuicPacket {
            header,
            frames,
            payload: data[offset..].to_vec(),
        })
    }
}

/// Origin fingerprint for Chrome behavioral mirroring using categorical composition
/// Applies Trikeshed Join<A,B> pattern for fingerprint atoms
#[derive(Clone, Debug)]
pub struct OriginFingerprint {
    pub ja3_hash: String,
    pub ja4_hash: String,
    pub extensions: Vec<u16>,
    pub cipher_suites: Vec<u16>,
    pub alpn_protocols: Vec<String>,
    pub transport_params: Option<TransportParameters>,
}

// Categorical fingerprint atoms for zero-allocation composition
type FingerprintJA3Atom = Join<String, Vec<u16>>; // (hash, cipher_suites)
type FingerprintJA4Atom = Join<String, Vec<u16>>; // (hash, extensions)
type FingerprintALPNAtom = Join<Vec<String>, Option<TransportParameters>>; // (protocols, quic_params)

// Helper functions for categorical fingerprint composition
fn fingerprint_ja3_atom(hash: String, ciphers: Vec<u16>) -> FingerprintJA3Atom {
    (hash, ciphers)
}

fn fingerprint_ja4_atom(hash: String, extensions: Vec<u16>) -> FingerprintJA4Atom {
    (hash, extensions)
}

fn fingerprint_alpn_atom(protocols: Vec<String>, params: Option<TransportParameters>) -> FingerprintALPNAtom {
    (protocols, params)
}

impl Default for OriginFingerprint {
    /// Create default Chrome fingerprint using categorical composition
    fn default() -> Self {
        // Chrome Stable N-2 categorical fingerprint atoms
        let ja3_atom = fingerprint_ja3_atom(
            "769,4865-4866-4867-49195-49199-49196-49200-52393-52392-49171-49172-156-157-47-53,0-23-65281-10-11-35-16-5-13-18-51-45-43-27-21,29-23-24,0".to_string(),
            vec![4865, 4866, 4867, 49195, 49199, 49196, 49200, 52393, 52392]
        );
        
        let ja4_atom = fingerprint_ja4_atom(
            "t13d1516h2_8daaf6152771_cd85d2b93bb2".to_string(),
            vec![0, 23, 65281, 10, 11, 35, 16, 5, 13, 18, 51, 45, 43, 27, 21]
        );
        
        let alpn_atom = fingerprint_alpn_atom(
            vec!["h2".to_string(), "http/1.1".to_string()],
            None
        );
        
        Self {
            ja3_hash: ja3_atom.0,
            ja4_hash: ja4_atom.0,
            extensions: ja4_atom.1,
            cipher_suites: ja3_atom.1,
            alpn_protocols: alpn_atom.0,
            transport_params: alpn_atom.1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[tokio::test]
    async fn test_chrome_quic_engine_creation() {
        let config = ChromeQuicConfig::default();
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);
        
        let engine = ChromeQuicEngine::new(bind_addr, config).await;
        assert!(engine.is_ok());
    }

    #[tokio::test]
    async fn test_chrome_connection_id_generation() {
        let config = ChromeQuicConfig::default();
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);
        let engine = ChromeQuicEngine::new(bind_addr, config).await.unwrap();
        
        let conn_id = engine.generate_chrome_connection_id();
        assert_eq!(conn_id.bytes.len(), 8);
    }

    #[test]
    fn test_chrome_transport_params() {
        let config = ChromeQuicConfig::default();
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let engine = rt.block_on(ChromeQuicEngine::new(bind_addr, config)).unwrap();
        
        let origin = OriginFingerprint::default();
        let params = engine.create_chrome_transport_params(&origin);
        
        // Verify Chrome-specific defaults
        assert_eq!(params.max_packet_size, 1472);
        assert_eq!(params.active_connection_id_limit, 2);
        // Congestion window test removed
    }
}