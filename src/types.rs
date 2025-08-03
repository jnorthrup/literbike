use std::fmt::{Display, Formatter};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProtocolType {
    Http = 0x01,
    Https = 0x02,
    Socks5 = 0x03,
    Connect = 0x04,
    Doh = 0x05,
    Upnp = 0x06,
    Bonjour = 0x07,
    Shadowsocks = 0x08,
    Tls = 0x09,
    Udp = 0x0A,
    Tcp = 0x0B,
    Pac = 0x0C,
    WebRtc = 0x0D,
    Quic = 0x0E,
    Ssh = 0x0F,
    Ftp = 0x10,
    Smtp = 0x11,
    Pop3 = 0x12,
    Imap = 0x13,
    Irc = 0x14,
    Xmpp = 0x15,
    Mqtt = 0x16,
    Websocket = 0x17,
    H2c = 0x18,
    Rtsp = 0x19,
    Sip = 0x1A,
    Dns = 0x1B,
    Dhcp = 0x1C,
    Snmp = 0x1D,
    Ntp = 0x1E,
    Ldap = 0x1F,
    Kerberos = 0x20,
    Radius = 0x21,
    Syslog = 0x22,
    Telnet = 0x23,
    Rlogin = 0x24,
    Vnc = 0x25,
    Rdp = 0x26,
    X11 = 0x27,
    Smb = 0x28,
    Nfs = 0x29,
    Tftp = 0x2A,
    BitTorrent = 0x2B,
    Gnutella = 0x2C,
    Kazaa = 0x2D,
    Skype = 0x2E,
    TeamViewer = 0x2F,
    Tor = 0x30,
    I2p = 0x31,
    Onion = 0x32,
    Freenet = 0x33,
    Raw = 0xFF,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressType {
    Ipv4 = 0x01,
    DomainName = 0x03,
    Ipv6 = 0x04,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Socks5Command {
    Connect = 0x01,
    Bind = 0x02,
    UdpAssociate = 0x03,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Socks5Reply {
    Succeeded = 0x00,
    GeneralFailure = 0x01,
    ConnectionNotAllowed = 0x02,
    NetworkUnreachable = 0x03,
    HostUnreachable = 0x04,
    ConnectionRefused = 0x05,
    TtlExpired = 0x06,
    CommandNotSupported = 0x07,
    AddressTypeNotSupported = 0x08,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get = 0x01,
    Post = 0x02,
    Put = 0x03,
    Delete = 0x04,
    Head = 0x05,
    Options = 0x06,
    Connect = 0x07,
    Trace = 0x08,
    Patch = 0x09,
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardPort {
    Http = 80,
    Https = 443,
    Dns = 53,
    Socks5 = 1080,
    HttpProxy = 8080,
    HttpsProxy = 8443,
    SquidProxy = 3128,
    Upnp = 1900,
    Mdns = 5353,
    PacServer = 8888,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowsocksMethod {
    Aes256Gcm,
    Chacha20IetfPoly1305,
    Aes128Gcm,
    Aes192Gcm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsVersion {
    Tls10 = 0x0301,
    Tls11 = 0x0302,
    Tls12 = 0x0303,
    Tls13 = 0x0304,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpnpAction {
    Search,
    Notify,
    Subscribe,
    Unsubscribe,
    AddPortMapping,
    DeletePortMapping,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetAddress {
    Ipv4 { addr: Ipv4Addr, port: u16 },
    Ipv6 { addr: Ipv6Addr, port: u16 },
    Domain { host: String, port: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Idle = 0x00,
    Handshaking = 0x01,
    Authenticating = 0x02,
    ProtocolDetection = 0x03,
    Connected = 0x04,
    Relaying = 0x05,
    Closing = 0x06,
    Closed = 0x07,
    Error = 0xFF,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    NoAuth = 0x00,
    Gssapi = 0x01,
    UsernamePassword = 0x02,
    NoAcceptable = 0xFF,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitFlags(pub u8);

impl BitFlags {
    pub const NONE: BitFlags = BitFlags(0x00);
    pub const KEEP_ALIVE: BitFlags = BitFlags(0x01);  
    pub const CLOSE: BitFlags = BitFlags(0x02);
    pub const UPGRADE: BitFlags = BitFlags(0x04);
    pub const CHUNKED: BitFlags = BitFlags(0x08);
    pub const GZIP: BitFlags = BitFlags(0x10);
    pub const DEFLATE: BitFlags = BitFlags(0x20);
    pub const ENCRYPTED: BitFlags = BitFlags(0x40);
    pub const AUTHENTICATED: BitFlags = BitFlags(0x80);

    pub fn has_flag(self, flag: BitFlags) -> bool {
        (self.0 & flag.0) != 0
    }

    pub fn set_flag(&mut self, flag: BitFlags) {
        self.0 |= flag.0;
    }

    pub fn clear_flag(&mut self, flag: BitFlags) {
        self.0 &= !flag.0;
    }

    pub fn toggle_flag(&mut self, flag: BitFlags) {
        self.0 ^= flag.0;
    }
}

#[derive(Debug, Clone)]
pub struct ProtocolDetectionResult {
    pub protocol: ProtocolType,
    pub confidence: u8,
    pub flags: BitFlags,
    pub metadata: Option<Vec<u8>>,
}

impl Display for ProtocolType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolType::Http => write!(f, "HTTP"),
            ProtocolType::Https => write!(f, "HTTPS"),
            ProtocolType::Socks5 => write!(f, "SOCKS5"),
            ProtocolType::Connect => write!(f, "CONNECT"),
            ProtocolType::Doh => write!(f, "DoH"),
            ProtocolType::Upnp => write!(f, "UPnP"),
            ProtocolType::Bonjour => write!(f, "Bonjour"),
            ProtocolType::Shadowsocks => write!(f, "Shadowsocks"),
            ProtocolType::Tls => write!(f, "TLS"),
            ProtocolType::Udp => write!(f, "UDP"),
            ProtocolType::Tcp => write!(f, "TCP"),
            ProtocolType::Pac => write!(f, "PAC"),
            ProtocolType::WebRtc => write!(f, "WebRTC"),
            ProtocolType::Quic => write!(f, "QUIC"),
            ProtocolType::Ssh => write!(f, "SSH"),
            ProtocolType::Ftp => write!(f, "FTP"),
            ProtocolType::Smtp => write!(f, "SMTP"),
            ProtocolType::Pop3 => write!(f, "POP3"),
            ProtocolType::Imap => write!(f, "IMAP"),
            ProtocolType::Irc => write!(f, "IRC"),
            ProtocolType::Xmpp => write!(f, "XMPP"),
            ProtocolType::Mqtt => write!(f, "MQTT"),
            ProtocolType::Websocket => write!(f, "WebSocket"),
            ProtocolType::H2c => write!(f, "HTTP/2"),
            ProtocolType::Rtsp => write!(f, "RTSP"),
            ProtocolType::Sip => write!(f, "SIP"),
            ProtocolType::Dns => write!(f, "DNS"),
            ProtocolType::Dhcp => write!(f, "DHCP"),
            ProtocolType::Snmp => write!(f, "SNMP"),
            ProtocolType::Ntp => write!(f, "NTP"),
            ProtocolType::Ldap => write!(f, "LDAP"),
            ProtocolType::Kerberos => write!(f, "Kerberos"),
            ProtocolType::Radius => write!(f, "RADIUS"),
            ProtocolType::Syslog => write!(f, "Syslog"),
            ProtocolType::Telnet => write!(f, "Telnet"),
            ProtocolType::Rlogin => write!(f, "Rlogin"),
            ProtocolType::Vnc => write!(f, "VNC"),
            ProtocolType::Rdp => write!(f, "RDP"),
            ProtocolType::X11 => write!(f, "X11"),
            ProtocolType::Smb => write!(f, "SMB"),
            ProtocolType::Nfs => write!(f, "NFS"),
            ProtocolType::Tftp => write!(f, "TFTP"),
            ProtocolType::BitTorrent => write!(f, "BitTorrent"),
            ProtocolType::Gnutella => write!(f, "Gnutella"),
            ProtocolType::Kazaa => write!(f, "Kazaa"),
            ProtocolType::Skype => write!(f, "Skype"),
            ProtocolType::TeamViewer => write!(f, "TeamViewer"),
            ProtocolType::Tor => write!(f, "Tor"),
            ProtocolType::I2p => write!(f, "I2P"),
            ProtocolType::Onion => write!(f, "Onion"),
            ProtocolType::Freenet => write!(f, "Freenet"),
            ProtocolType::Raw => write!(f, "RAW"),
        }
    }
}

impl From<u16> for StandardPort {
    fn from(port: u16) -> Self {
        match port {
            53 => StandardPort::Dns,
            80 => StandardPort::Http,
            443 => StandardPort::Https,
            1080 => StandardPort::Socks5,
            1900 => StandardPort::Upnp,
            3128 => StandardPort::SquidProxy,
            5353 => StandardPort::Mdns,
            8080 => StandardPort::HttpProxy,
            8443 => StandardPort::HttpsProxy,
            8888 => StandardPort::PacServer,
            _ => StandardPort::Http,
        }
    }
}

impl From<StandardPort> for u16 {
    fn from(port: StandardPort) -> Self {
        port as u16
    }
}

impl TargetAddress {
    pub fn new(host: &str, port: u16) -> Self {
        if let Ok(ipv4) = host.parse::<Ipv4Addr>() {
            Self::Ipv4 { addr: ipv4, port }
        } else if let Ok(ipv6) = host.parse::<Ipv6Addr>() {
            Self::Ipv6 { addr: ipv6, port }
        } else {
            Self::Domain { host: host.to_string(), port }
        }
    }

    pub fn to_socket_addr(&self, resolved_ip: Option<IpAddr>) -> Option<SocketAddr> {
        match self {
            Self::Ipv4 { addr, port } => Some(SocketAddr::new(IpAddr::V4(*addr), *port)),
            Self::Ipv6 { addr, port } => Some(SocketAddr::new(IpAddr::V6(*addr), *port)),
            Self::Domain { port, .. } => resolved_ip.map(|ip| SocketAddr::new(ip, *port)),
        }
    }

    pub fn port(&self) -> u16 {
        match self {
            Self::Ipv4 { port, .. } | Self::Ipv6 { port, .. } | Self::Domain { port, .. } => *port,
        }
    }

    pub fn host(&self) -> String {
        match self {
            Self::Ipv4 { addr, .. } => addr.to_string(),
            Self::Ipv6 { addr, .. } => format!("[{}]", addr),
            Self::Domain { host, .. } => host.clone(),
        }
    }

    pub fn is_local_domain(&self) -> bool {
        match self {
            Self::Domain { host, .. } => host.ends_with(".local"),
            _ => false,
        }
    }
}

impl Display for TargetAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ipv4 { addr, port } => write!(f, "{}:{}", addr, port),
            Self::Ipv6 { addr, port } => write!(f, "[{}]:{}", addr, port),
            Self::Domain { host, port } => write!(f, "{}:{}", host, port),
        }
    }
}

impl ShadowsocksMethod {
    pub fn key_length(&self) -> usize {
        match self {
            ShadowsocksMethod::Aes128Gcm => 16,
            ShadowsocksMethod::Aes192Gcm => 24, 
            ShadowsocksMethod::Aes256Gcm => 32,
            ShadowsocksMethod::Chacha20IetfPoly1305 => 32,
        }
    }

    pub fn nonce_length(&self) -> usize {
        match self {
            ShadowsocksMethod::Aes128Gcm | 
            ShadowsocksMethod::Aes192Gcm | 
            ShadowsocksMethod::Aes256Gcm => 12,
            ShadowsocksMethod::Chacha20IetfPoly1305 => 12,
        }
    }
}

impl From<&str> for ShadowsocksMethod {
    fn from(method: &str) -> Self {
        match method.to_lowercase().as_str() {
            "aes-128-gcm" => ShadowsocksMethod::Aes128Gcm,
            "aes-192-gcm" => ShadowsocksMethod::Aes192Gcm,
            "aes-256-gcm" => ShadowsocksMethod::Aes256Gcm,
            "chacha20-ietf-poly1305" => ShadowsocksMethod::Chacha20IetfPoly1305,
            _ => ShadowsocksMethod::Aes256Gcm,
        }
    }
}

pub fn bitbang_u16(value: u16) -> [u8; 2] {
    value.to_be_bytes()
}

pub fn bitbang_u32(value: u32) -> [u8; 4] {
    value.to_be_bytes()
}

pub fn unbang_u16(bytes: &[u8]) -> u16 {
    u16::from_be_bytes([bytes[0], bytes[1]])
}

pub fn unbang_u32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

pub fn extract_bits(value: u8, start: u8, length: u8) -> u8 {
    let mask = (1u8 << length) - 1;
    (value >> start) & mask
}

pub fn set_bits(value: u8, start: u8, length: u8, bits: u8) -> u8 {
    let mask = ((1u8 << length) - 1) << start;
    (value & !mask) | ((bits << start) & mask)
}