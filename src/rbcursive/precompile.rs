//! RBCursive Precompile - Sum of All Parsers at Compile Time
//!
//! Pre-baked parser patterns for zero-runtime-overhead protocol dispatch.
//! Used by literbike for port 8888 protocol interception.

/// Pre-baked byte patterns for protocol detection
#[derive(Debug, Clone)]
pub struct PrecompiledPatterns {
    pub ollama_generate: &'static [u8],
    pub ollama_chat: &'static [u8],
    pub ollama_tags: &'static [u8],
    pub ollama_show: &'static [u8],
    pub openai_chat: &'static [u8],
    pub openai_models: &'static [u8],
    pub anthropic_messages: &'static [u8],
    pub health: &'static [u8],
    pub metrics: &'static [u8],
    pub http_get: &'static [u8],
    pub http_post: &'static [u8],
}

impl PrecompiledPatterns {
    pub const fn new() -> Self {
        Self {
            ollama_generate: b"POST /api/generate",
            ollama_chat: b"POST /api/chat",
            ollama_tags: b"GET /api/tags",
            ollama_show: b"POST /api/show",
            openai_chat: b"POST /v1/chat/completions",
            openai_models: b"GET /v1/models",
            anthropic_messages: b"POST /v1/messages",
            health: b"GET /health",
            metrics: b"GET /metrics",
            http_get: b"GET ",
            http_post: b"POST ",
        }
    }
    
    /// Fast prefix match
    #[inline]
    fn starts_with(data: &[u8], pattern: &[u8]) -> bool {
        data.len() >= pattern.len() && &data[..pattern.len()] == pattern
    }
    
    /// Detect protocol from precompiled patterns
    pub fn detect_protocol(&self, data: &[u8]) -> &'static str {
        if Self::starts_with(data, self.ollama_generate) { return "ollama/generate"; }
        if Self::starts_with(data, self.ollama_chat) { return "ollama/chat"; }
        if Self::starts_with(data, self.ollama_tags) { return "ollama/tags"; }
        if Self::starts_with(data, self.ollama_show) { return "ollama/show"; }
        if Self::starts_with(data, self.openai_chat) { return "openai/chat"; }
        if Self::starts_with(data, self.openai_models) { return "openai/models"; }
        if Self::starts_with(data, self.anthropic_messages) { return "anthropic/messages"; }
        if Self::starts_with(data, self.health) { return "health"; }
        if Self::starts_with(data, self.metrics) { return "metrics"; }
        if Self::starts_with(data, self.http_get) { return "http/get"; }
        if Self::starts_with(data, self.http_post) { return "http/post"; }
        "unknown"
    }
}

/// Global precompiled patterns - baked at compile time
pub static PRECOMPILED_PATTERNS: PrecompiledPatterns = PrecompiledPatterns::new();

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_precompiled_ollama_detection() {
        assert_eq!(PRECOMPILED_PATTERNS.detect_protocol(b"POST /api/generate HTTP/1.1"), "ollama/generate");
        assert_eq!(PRECOMPILED_PATTERNS.detect_protocol(b"POST /api/chat HTTP/1.1"), "ollama/chat");
        assert_eq!(PRECOMPILED_PATTERNS.detect_protocol(b"GET /api/tags HTTP/1.1"), "ollama/tags");
        assert_eq!(PRECOMPILED_PATTERNS.detect_protocol(b"POST /api/show HTTP/1.1"), "ollama/show");
    }
    
    #[test]
    fn test_precompiled_openai_detection() {
        assert_eq!(PRECOMPILED_PATTERNS.detect_protocol(b"POST /v1/chat/completions HTTP/1.1"), "openai/chat");
        assert_eq!(PRECOMPILED_PATTERNS.detect_protocol(b"GET /v1/models HTTP/1.1"), "openai/models");
    }
    
    #[test]
    fn test_precompiled_anthropic_detection() {
        assert_eq!(PRECOMPILED_PATTERNS.detect_protocol(b"POST /v1/messages HTTP/1.1"), "anthropic/messages");
    }
    
    #[test]
    fn test_precompiled_health_detection() {
        assert_eq!(PRECOMPILED_PATTERNS.detect_protocol(b"GET /health HTTP/1.1"), "health");
        assert_eq!(PRECOMPILED_PATTERNS.detect_protocol(b"GET /metrics HTTP/1.1"), "metrics");
    }
    
    #[test]
    fn test_precompiled_http_detection() {
        assert_eq!(PRECOMPILED_PATTERNS.detect_protocol(b"GET /api/test HTTP/1.1"), "http/get");
        assert_eq!(PRECOMPILED_PATTERNS.detect_protocol(b"POST /api/test HTTP/1.1"), "http/post");
    }
    
    #[test]
    fn test_precompiled_unknown() {
        assert_eq!(PRECOMPILED_PATTERNS.detect_protocol(b"UNKNOWN"), "unknown");
    }
    
    #[test]
    fn test_all_8888_protocols() {
        // All protocols that dispatch from port 8888
        let tests: Vec<(&[u8], &str)> = vec![
            (b"POST /api/generate HTTP/1.1", "ollama/generate"),
            (b"POST /api/chat HTTP/1.1", "ollama/chat"),
            (b"GET /api/tags HTTP/1.1", "ollama/tags"),
            (b"POST /api/show HTTP/1.1", "ollama/show"),
            (b"POST /v1/chat/completions HTTP/1.1", "openai/chat"),
            (b"GET /v1/models HTTP/1.1", "openai/models"),
            (b"POST /v1/messages HTTP/1.1", "anthropic/messages"),
            (b"GET /health HTTP/1.1", "health"),
            (b"GET /metrics HTTP/1.1", "metrics"),
        ];
        
        for (input, expected) in tests {
            assert_eq!(PRECOMPILED_PATTERNS.detect_protocol(input), expected);
        }
    }
}
