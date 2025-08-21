// Port target for QuicProtocolAdapter.kt - parsing/serialization stubs

pub fn quic_adapter_name() -> &'static str {
    "quic::QuicProtocolAdapter"
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quic_name() {
        assert_eq!(quic_adapter_name(), "quic::QuicProtocolAdapter");
    }
}
