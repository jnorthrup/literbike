use std::io;
use log::{debug, info, warn};
use tokio::io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};

use crate::types::{ProtocolType, ProtocolDetectionResult, BitFlags};
use crate::abstractions::ProtocolDetector;

pub struct WebRtcDetector;

impl ProtocolDetector for WebRtcDetector {
    async fn detect(&self, buffer: &[u8]) -> ProtocolDetectionResult {
        let confidence = if buffer.len() >= 20 {
            // STUN/TURN binding request: 0x0001 (Message Type)
            if buffer.len() >= 20 && 
               ((buffer[0] == 0x00 && buffer[1] == 0x01) || // STUN Binding Request
                (buffer[0] == 0x01 && buffer[1] == 0x01) || // STUN Binding Response
                (buffer[0] == 0x00 && buffer[1] == 0x03)) { // STUN Allocate Request
                let magic_cookie = u32::from_be_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]);
                if magic_cookie == 0x2112A442 { // STUN magic cookie
                    255
                } else {
                    150
                }
            }
            // DTLS handshake detection
            else if buffer[0] == 0x16 && buffer[1] == 0xFE { // DTLS 1.0/1.2
                200
            }
            // ICE candidate format detection
            else if String::from_utf8_lossy(buffer).contains("candidate:") {
                180
            } else {
                0
            }
        } else {
            0
        };

        ProtocolDetectionResult {
            protocol: ProtocolType::WebRtc,
            confidence,
            flags: if confidence > 150 { BitFlags::ENCRYPTED } else { BitFlags::NONE },
            metadata: Some(buffer.to_vec()),
        }
    }

    fn confidence_threshold(&self) -> u8 { 150 }
    fn required_bytes(&self) -> usize { 20 }
}

pub struct QuicDetector;

impl ProtocolDetector for QuicDetector {
    async fn detect(&self, buffer: &[u8]) -> ProtocolDetectionResult {
        let confidence = if buffer.len() >= 16 {
            // QUIC long header: first bit = 1
            if (buffer[0] & 0x80) != 0 {
                let version = u32::from_be_bytes([buffer[1], buffer[2], buffer[3], buffer[4]]);
                match version {
                    0x00000001 => 255, // QUIC v1 (RFC 9000)
                    0x6B3343CF => 240, // QUIC v1 (Google QUIC)
                    0xFF00001D => 220, // Draft versions
                    0x00000000 => 200, // Version negotiation
                    _ if version >= 0xFF000000 => 180, // Draft version pattern
                    _ => 100,
                }
            }
            // QUIC short header: first bit = 0
            else if (buffer[0] & 0x40) != 0 { // Key phase bit
                150
            } else {
                0
            }
        } else {
            0
        };

        ProtocolDetectionResult {
            protocol: ProtocolType::Quic,
            confidence,
            flags: BitFlags::ENCRYPTED,
            metadata: Some(buffer.to_vec()),
        }
    }

    fn confidence_threshold(&self) -> u8 { 150 }
    fn required_bytes(&self) -> usize { 16 }
}

pub struct SshDetector;

impl ProtocolDetector for SshDetector {
    async fn detect(&self, buffer: &[u8]) -> ProtocolDetectionResult {
        let data = String::from_utf8_lossy(buffer);
        let confidence = if data.starts_with("SSH-2.0-") {
            255
        } else if data.starts_with("SSH-1.99-") || data.starts_with("SSH-1.5-") {
            240
        } else if data.starts_with("SSH-") {
            200
        } else if buffer.len() >= 6 && 
                  buffer[0] == 0x00 && buffer[1] == 0x00 && // Packet length
                  buffer[5] >= 1 && buffer[5] <= 99 { // SSH message codes
            150
        } else {
            0
        };

        ProtocolDetectionResult {
            protocol: ProtocolType::Ssh,
            confidence,
            flags: if confidence > 150 { BitFlags::ENCRYPTED } else { BitFlags::NONE },
            metadata: Some(buffer.to_vec()),
        }
    }

    fn confidence_threshold(&self) -> u8 { 180 }
    fn required_bytes(&self) -> usize { 8 }
}

pub struct FtpDetector;

impl ProtocolDetector for FtpDetector {
    async fn detect(&self, buffer: &[u8]) -> ProtocolDetectionResult {
        let data = String::from_utf8_lossy(buffer);
        let confidence = if data.starts_with("220 ") || data.starts_with("220-") {
            255 // FTP server greeting
        } else if data.starts_with("USER ") || data.starts_with("PASS ") ||
                  data.starts_with("QUIT") || data.starts_with("PWD") ||
                  data.starts_with("LIST") || data.starts_with("RETR ") ||
                  data.starts_with("STOR ") {
            240 // FTP commands
        } else if data.contains("530 ") || data.contains("331 ") ||
                  data.contains("226 ") || data.contains("150 ") {
            220 // FTP response codes
        } else {
            0
        };

        ProtocolDetectionResult {
            protocol: ProtocolType::Ftp,
            confidence,
            flags: BitFlags::NONE,
            metadata: Some(buffer.to_vec()),
        }
    }

    fn confidence_threshold(&self) -> u8 { 200 }
    fn required_bytes(&self) -> usize { 4 }
}

pub struct SmtpDetector;

impl ProtocolDetector for SmtpDetector {
    async fn detect(&self, buffer: &[u8]) -> ProtocolDetectionResult {
        let data = String::from_utf8_lossy(buffer);
        let confidence = if data.starts_with("220 ") && data.contains("SMTP") {
            255 // SMTP server greeting
        } else if data.starts_with("HELO ") || data.starts_with("EHLO ") ||
                  data.starts_with("MAIL FROM:") || data.starts_with("RCPT TO:") ||
                  data.starts_with("DATA") || data.starts_with("QUIT") {
            240 // SMTP commands
        } else if data.starts_with("250 ") || data.starts_with("354 ") ||
                  data.starts_with("221 ") {
            220 // SMTP responses
        } else {
            0
        };

        ProtocolDetectionResult {
            protocol: ProtocolType::Smtp,
            confidence,
            flags: BitFlags::NONE,
            metadata: Some(buffer.to_vec()),
        }
    }

    fn confidence_threshold(&self) -> u8 { 200 }
    fn required_bytes(&self) -> usize { 4 }
}

pub struct IrcDetector;

impl ProtocolDetector for IrcDetector {
    async fn detect(&self, buffer: &[u8]) -> ProtocolDetectionResult {
        let data = String::from_utf8_lossy(buffer);
        let confidence = if data.starts_with("NICK ") || data.starts_with("USER ") ||
                           data.starts_with("JOIN #") || data.starts_with("PRIVMSG ") ||
                           data.starts_with("PING ") || data.starts_with("PONG ") {
            255 // IRC commands
        } else if data.starts_with(":") && (data.contains(" 001 ") || 
                                           data.contains(" 002 ") ||
                                           data.contains(" PRIVMSG ")) {
            240 // IRC server responses
        } else if data.contains("MODE ") || data.contains("QUIT ") ||
                  data.contains("PART ") {
            200
        } else {
            0
        };

        ProtocolDetectionResult {
            protocol: ProtocolType::Irc,
            confidence,
            flags: BitFlags::NONE,
            metadata: Some(buffer.to_vec()),
        }
    }

    fn confidence_threshold(&self) -> u8 { 180 }
    fn required_bytes(&self) -> usize { 4 }
}

pub struct WebSocketDetector;

impl ProtocolDetector for WebSocketDetector {
    async fn detect(&self, buffer: &[u8]) -> ProtocolDetectionResult {
        let data = String::from_utf8_lossy(buffer);
        let confidence = if data.contains("Upgrade: websocket") &&
                           data.contains("Connection: Upgrade") &&
                           data.contains("Sec-WebSocket-Key:") {
            255 // WebSocket handshake
        } else if data.contains("Sec-WebSocket-Accept:") {
            240 // WebSocket response
        } else if buffer.len() >= 2 {
            let first_byte = buffer[0];
            let second_byte = buffer[1];
            
            // WebSocket frame detection
            let fin = (first_byte & 0x80) != 0;
            let opcode = first_byte & 0x0F;
            let masked = (second_byte & 0x80) != 0;
            
            if opcode <= 0x0A && (masked || !masked) { // Valid opcode range
                if opcode == 0x1 || opcode == 0x2 || opcode == 0x8 { // Text, binary, close
                    200
                } else {
                    150
                }
            } else {
                0
            }
        } else {
            0
        };

        ProtocolDetectionResult {
            protocol: ProtocolType::Websocket,
            confidence,
            flags: if confidence > 200 { BitFlags::UPGRADE } else { BitFlags::NONE },
            metadata: Some(buffer.to_vec()),
        }
    }

    fn confidence_threshold(&self) -> u8 { 150 }
    fn required_bytes(&self) -> usize { 8 }
}

pub struct MqttDetector;

impl ProtocolDetector for MqttDetector {
    async fn detect(&self, buffer: &[u8]) -> ProtocolDetectionResult {
        let confidence = if buffer.len() >= 2 {
            let msg_type = (buffer[0] & 0xF0) >> 4;
            let flags = buffer[0] & 0x0F;
            
            match msg_type {
                1 => { // CONNECT
                    if buffer.len() >= 10 && 
                       buffer[2] == 0x00 && buffer[3] == 0x04 && // Protocol name length
                       &buffer[4..8] == b"MQTT" { // Protocol name
                        255
                    } else {
                        100
                    }
                }
                2 => 240, // CONNACK
                3 => 220, // PUBLISH
                4 => 200, // PUBACK
                5 => 200, // PUBREC
                6 => 200, // PUBREL
                7 => 200, // PUBCOMP
                8 => 220, // SUBSCRIBE
                9 => 200, // SUBACK
                10 => 220, // UNSUBSCRIBE
                11 => 200, // UNSUBACK
                12 => 180, // PINGREQ
                13 => 180, // PINGRESP
                14 => 200, // DISCONNECT
                _ => 0,
            }
        } else {
            0
        };

        ProtocolDetectionResult {
            protocol: ProtocolType::Mqtt,
            confidence,
            flags: BitFlags::NONE,
            metadata: Some(buffer.to_vec()),
        }
    }

    fn confidence_threshold(&self) -> u8 { 180 }
    fn required_bytes(&self) -> usize { 8 }
}

pub struct SipDetector;

impl ProtocolDetector for SipDetector {
    async fn detect(&self, buffer: &[u8]) -> ProtocolDetectionResult {
        let data = String::from_utf8_lossy(buffer);
        let confidence = if data.starts_with("INVITE ") || data.starts_with("REGISTER ") ||
                           data.starts_with("OPTIONS ") || data.starts_with("BYE ") ||
                           data.starts_with("ACK ") || data.starts_with("CANCEL ") {
            if data.contains("SIP/2.0") {
                255
            } else {
                200
            }
        } else if data.starts_with("SIP/2.0 ") {
            255 // SIP response
        } else if data.contains("Via: SIP/2.0") || data.contains("To: ") ||
                  data.contains("From: ") || data.contains("Call-ID: ") {
            220
        } else {
            0
        };

        ProtocolDetectionResult {
            protocol: ProtocolType::Sip,
            confidence,
            flags: BitFlags::NONE,
            metadata: Some(buffer.to_vec()),
        }
    }

    fn confidence_threshold(&self) -> u8 { 200 }
    fn required_bytes(&self) -> usize { 8 }
}

pub struct RtspDetector;

impl ProtocolDetector for RtspDetector {
    async fn detect(&self, buffer: &[u8]) -> ProtocolDetectionResult {
        let data = String::from_utf8_lossy(buffer);
        let confidence = if data.starts_with("OPTIONS ") || data.starts_with("DESCRIBE ") ||
                           data.starts_with("SETUP ") || data.starts_with("PLAY ") ||
                           data.starts_with("PAUSE ") || data.starts_with("TEARDOWN ") {
            if data.contains("RTSP/1.0") {
                255
            } else {
                200
            }
        } else if data.starts_with("RTSP/1.0 ") {
            255 // RTSP response
        } else if data.contains("CSeq: ") || data.contains("Session: ") ||
                  data.contains("Transport: ") {
            220
        } else {
            0
        };

        ProtocolDetectionResult {
            protocol: ProtocolType::Rtsp,
            confidence,
            flags: BitFlags::NONE,
            metadata: Some(buffer.to_vec()),
        }
    }

    fn confidence_threshold(&self) -> u8 { 200 }
    fn required_bytes(&self) -> usize { 8 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ssh_detection() {
        let detector = SshDetector;
        let ssh_banner = b"SSH-2.0-OpenSSH_8.0\r\n";
        let result = detector.detect(ssh_banner).await;
        assert_eq!(result.protocol, ProtocolType::Ssh);
        assert_eq!(result.confidence, 255);
    }

    #[tokio::test]
    async fn test_websocket_detection() {
        let detector = WebSocketDetector;
        let ws_handshake = b"GET / HTTP/1.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: test\r\n";
        let result = detector.detect(ws_handshake).await;
        assert_eq!(result.protocol, ProtocolType::Websocket);
        assert_eq!(result.confidence, 255);
    }

    #[tokio::test]
    async fn test_quic_detection() {
        let detector = QuicDetector;
        let quic_packet = [0x80, 0x00, 0x00, 0x00, 0x01]; // Long header, QUIC v1
        let result = detector.detect(&quic_packet).await;
        assert_eq!(result.protocol, ProtocolType::Quic);
        assert_eq!(result.confidence, 255);
    }

    #[tokio::test]
    async fn test_mqtt_detection() {
        let detector = MqttDetector;
        let mqtt_connect = [0x10, 0x0A, 0x00, 0x04, b'M', b'Q', b'T', b'T']; // CONNECT packet
        let result = detector.detect(&mqtt_connect).await;
        assert_eq!(result.protocol, ProtocolType::Mqtt);
        assert_eq!(result.confidence, 255);
    }
}