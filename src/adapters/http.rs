// Port target for HttpProtocolAdapter.kt / HttpParser.kt

pub fn http_adapter_name() -> &'static str {
    "http::HttpProtocolAdapter"
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn http_name() {
        assert_eq!(http_adapter_name(), "http::HttpProtocolAdapter");
    }
}
