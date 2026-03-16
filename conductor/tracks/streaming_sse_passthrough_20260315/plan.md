# Track: Streaming SSE Passthrough

## Objective

Implement hardened server-sent events (SSE) passthrough for ModelMux streaming completions. This enables real-time token streaming from upstream providers (Anthropic, OpenAI, OpenRouter) to clients with proper token tracking, connection management, and error handling.

## Context

The modelmux-desktop-proxy track lists streaming SSE as a remaining item. While basic SSE streaming exists in `src/modelmux/proxy.rs`, it needs:

1. Connection pooling for streaming endpoints
2. Robust error handling for stream interruptions
3. Keep-alive heartbeat handling
4. Proper buffer management for partial SSE frames
5. Integration tests with mock providers

## Technical Design

### SSE Stream Processing

```
Raw Bytes → SSE Frame Parser → Token Extractor → Client Stream
                ↓                      ↓
           Buffer Manager        Token Ledger
```

### Algebraic Properties

The SSE stream transform must satisfy:
- **Identity**: `parse_sse(encode_sse(events)) ≡ events`
- **Associativity**: `parse_sse(a + b) = parse_sse(a) + parse_sse(b)` (for partial frames)
- **Monotonicity**: Token count never decreases: `tokens(t+1) >= tokens(t)`

### Components

1. **`TrackedSseStream`** - Existing wrapper that extracts token usage
2. **`SseFrameParser`** - Robust parser handling partial frames across chunk boundaries
3. **`StreamingConnectionPool`** - Keep connections warm for low-latency streaming
4. **`HeartbeatStream`** - Keep-alive to prevent timeouts during slow generation

## Implementation Checklist

- [ ] Harden `TrackedSseStream` for partial frame handling
- [ ] Add connection pooling for streaming endpoints
- [ ] Implement heartbeat/keep-alive mechanism
- [ ] Add comprehensive error handling for stream failures
- [ ] Write integration tests with mock SSE streams
- [ ] Add metrics for stream duration, tokens/second, error rates
- [ ] Document the streaming API

## Verification

```bash
# Unit tests
cargo test --lib sse_streaming

# Integration test with mock provider
cargo test --test streaming_passthrough

# Manual verification
MODELMUX_ENABLE_STREAMING=1 cargo run --bin modelmux
curl -N -H "Authorization: Bearer $ANTHROPIC_API_KEY" \
  http://localhost:8888/v1/chat/completions \
  -d '{"model": "claude-3-sonnet", "messages": [{"role": "user", "content": "hello"}], "stream": true}'
```

## Dependencies

- `tokio-stream` - Stream utilities
- `futures` - Async stream traits
- `bytes` - Byte buffer management
- `serde_json` - JSON parsing from SSE data

## Status

- Created: 2026-03-15
- Priority: HIGH (blocks modelmux-desktop-proxy completion)
