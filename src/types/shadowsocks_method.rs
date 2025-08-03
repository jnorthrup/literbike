/// Shadowsocks encryption methods
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowsocksMethod {
    Aes128Gcm,
    Aes256Gcm,
    Chacha20Poly1305,
    None,
}

impl ShadowsocksMethod {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Aes128Gcm => "aes-128-gcm",
            Self::Aes256Gcm => "aes-256-gcm",
            Self::Chacha20Poly1305 => "chacha20-poly1305",
            Self::None => "",
        }
    }
}

impl From<ShadowsocksMethod> for u8 {
    fn from(method: ShadowsocksMethod) -> Self {
        match method {
            ShadowsocksMethod::Aes128Gcm => 0x01,
            ShadowsocksMethod::Aes256Gcm => 0x02,
            ShadowsocksMethod::Chacha20Poly1305 => 0x03,
            ShadowsocksMethod::None => 0x00,
        }
    }
}