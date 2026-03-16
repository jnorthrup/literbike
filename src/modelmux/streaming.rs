//! Hardened SSE (Server-Sent Events) streaming for ModelMux
//!
//! Provides robust SSE frame parsing, connection pooling, heartbeat keep-alive,
//! and token tracking for streaming LLM completions.

use bytes::Bytes;
use futures::stream::{Stream, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::interval;
use log::{debug, trace, warn};

/// An SSE frame representing a single event
#[derive(Debug, Clone, PartialEq)]
pub struct SseFrame {
    /// The event type (e.g., "message", "error")
    pub event: Option<String>,
    /// The data payload
    pub data: String,
    /// Optional event ID for replay/resume
    pub id: Option<String>,
    /// Optional retry hint in milliseconds
    pub retry: Option<u64>,
}

impl SseFrame {
    /// Parse an SSE frame from a raw string
    /// Format: field: value\n (fields: event, data, id, retry)
    /// Frames are delimited by double newline
    pub fn parse(raw: &str) -> Option<Self> {
        let mut event = None;
        let mut data = String::new();
        let mut id = None;
        let mut retry = None;

        for line in raw.lines() {
            if line.starts_with("event:") {
                event = Some(line[6..].trim_start().to_string());
            } else if line.starts_with("data:") {
                let data_line = line[5..].trim_start();
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(data_line);
            } else if line.starts_with("id:") {
                id = Some(line[3..].trim_start().to_string());
            } else if line.starts_with("retry:") {
                retry = line[6..].trim_start().parse().ok();
            }
            // Lines starting with ":" are comments, ignored
        }

        if data.is_empty() && event.is_none() && id.is_none() {
            return None;
        }

        Some(SseFrame { event, data, id, retry })
    }

    /// Serialize frame to SSE format
    pub fn to_sse_string(&self) -> String {
        let mut output = String::new();
        
        if let Some(ref event) = self.event {
            output.push_str(&format!("event: {}\n", event));
        }
        if let Some(ref id) = self.id {
            output.push_str(&format!("id: {}\n", id));
        }
        if let Some(retry) = self.retry {
            output.push_str(&format!("retry: {}\n", retry));
        }
        
        // Data may contain newlines - each line gets "data: " prefix
        for line in self.data.lines() {
            output.push_str(&format!("data: {}\n", line));
        }
        
        output.push('\n');
        output
    }

    /// Extract token usage from JSON data if present
    pub fn extract_token_usage(&self) -> Option<u64> {
        if let Ok(json) = serde_json::from_str::<Value>(&self.data) {
            // Look for usage field which may contain total_tokens
            if let Some(usage) = json.get("usage") {
                if let Some(total) = usage.get("total_tokens").and_then(|v| v.as_u64()) {
                    return Some(total);
                }
                // Sum prompt_tokens + completion_tokens
                let prompt = usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                let completion = usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                if prompt > 0 || completion > 0 {
                    return Some(prompt + completion);
                }
            }
        }
        None
    }

    /// Check if this is a "[DONE]" marker frame
    pub fn is_done_marker(&self) -> bool {
        self.data.trim() == "[DONE]"
    }
}

/// Robust SSE frame parser handling partial frames across chunk boundaries
pub struct SseFrameParser {
    /// Buffer for incomplete frame data
    buffer: String,
    /// Maximum buffer size before discarding (prevent memory exhaustion)
    max_buffer_size: usize,
}

impl SseFrameParser {
    /// Create a new parser with default 1MB buffer limit
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            max_buffer_size: 1024 * 1024, // 1MB
        }
    }

    /// Create a parser with custom buffer limit
    pub fn with_max_buffer(max_size: usize) -> Self {
        Self {
            buffer: String::new(),
            max_buffer_size: max_size,
        }
    }

    /// Parse new chunk data, returning complete frames
    /// Handles partial frames that span multiple chunks
    pub fn parse_chunk(&mut self, chunk: &str) -> Vec<SseFrame> {
        self.buffer.push_str(chunk);
        
        // Check buffer size limit
        if self.buffer.len() > self.max_buffer_size {
            warn!("SSE buffer exceeded max size, discarding partial data");
            self.buffer.clear();
            return Vec::new();
        }

        let mut frames = Vec::new();
        let mut last_end = 0;

        // Find all complete frames (delimited by \n\n)
        while let Some(pos) = self.buffer[last_end..].find("\n\n") {
            let frame_end = last_end + pos;
            let frame_data = &self.buffer[last_end..frame_end];
            
            if let Some(frame) = SseFrame::parse(frame_data) {
                frames.push(frame);
            }
            
            last_end = frame_end + 2; // Skip past \n\n
        }

        // Keep only incomplete data in buffer
        if last_end > 0 {
            self.buffer = self.buffer[last_end..].to_string();
        }

        frames
    }

    /// Get any remaining data in buffer (incomplete frame)
    pub fn flush_buffer(&mut self) -> Option<SseFrame> {
        if self.buffer.is_empty() {
            return None;
        }
        
        let frame = SseFrame::parse(&self.buffer);
        self.buffer.clear();
        frame
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

impl Default for SseFrameParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics for streaming connections
#[derive(Debug, Clone, Default)]
pub struct StreamingMetrics {
    /// Total bytes streamed
    pub bytes_streamed: u64,
    /// Total frames parsed
    pub frames_parsed: u64,
    /// Total tokens tracked
    pub tokens_tracked: u64,
    /// Stream start time
    pub start_time: Option<Instant>,
    /// Last activity time
    pub last_activity: Option<Instant>,
    /// Error count
    pub error_count: u64,
}

impl StreamingMetrics {
    /// Create new metrics
    pub fn new() -> Self {
        Self {
            start_time: Some(Instant::now()),
            last_activity: Some(Instant::now()),
            ..Default::default()
        }
    }

    /// Record bytes streamed
    pub fn record_bytes(&mut self, bytes: u64) {
        self.bytes_streamed += bytes;
        self.last_activity = Some(Instant::now());
    }

    /// Record frame parsed
    pub fn record_frame(&mut self) {
        self.frames_parsed += 1;
        self.last_activity = Some(Instant::now());
    }

    /// Record tokens tracked
    pub fn record_tokens(&mut self, tokens: u64) {
        self.tokens_tracked += tokens;
    }

    /// Record error
    pub fn record_error(&mut self) {
        self.error_count += 1;
    }

    /// Get stream duration
    pub fn duration(&self) -> Option<Duration> {
        self.start_time.map(|start| start.elapsed())
    }

    /// Get tokens per second
    pub fn tokens_per_second(&self) -> f64 {
        match self.duration() {
            Some(d) if d.as_secs_f64() > 0.0 => {
                self.tokens_tracked as f64 / d.as_secs_f64()
            }
            _ => 0.0
        }
    }

    /// Get idle time since last activity
    pub fn idle_time(&self) -> Option<Duration> {
        self.last_activity.map(|last| last.elapsed())
    }
}

/// Hardened SSE stream with robust parsing and metrics
pub struct TrackedSseStream {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    provider: String,
    parser: SseFrameParser,
    metrics: StreamingMetrics,
    tokens_accumulated: u64,
}

impl TrackedSseStream {
    /// Create a new tracked SSE stream
    pub fn new(
        stream: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
        provider: String,
    ) -> Self {
        Self {
            inner: stream,
            provider,
            parser: SseFrameParser::new(),
            metrics: StreamingMetrics::new(),
            tokens_accumulated: 0,
        }
    }

    /// Get current metrics
    pub fn metrics(&self) -> &StreamingMetrics {
        &self.metrics
    }

    /// Track tokens for the provider
    fn track_tokens(&mut self, tokens: u64) {
        if tokens > 0 {
            let new_tokens = tokens.saturating_sub(self.tokens_accumulated);
            if new_tokens > 0 {
                self.tokens_accumulated = tokens;
                self.metrics.record_tokens(new_tokens);
                let _ = crate::keymux::dsel::track_tokens(&self.provider, new_tokens);
                trace!("Tracked {} tokens for provider {}", new_tokens, self.provider);
            }
        }
    }
}

impl Stream for TrackedSseStream {
    type Item = Result<Bytes, reqwest::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                self.metrics.record_bytes(bytes.len() as u64);
                
                // Parse UTF-8 and extract frames
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    let frames = self.parser.parse_chunk(text);
                    
                    for frame in &frames {
                        self.metrics.record_frame();
                        
                        // Extract and track token usage
                        if let Some(usage) = frame.extract_token_usage() {
                            self.track_tokens(usage);
                        }
                        
                        // Log completion
                        if frame.is_done_marker() {
                            debug!("SSE stream completed for provider {}", self.provider);
                        }
                    }
                }

                Poll::Ready(Some(Ok(bytes)))
            }
            Poll::Ready(Some(Err(e))) => {
                self.metrics.record_error();
                warn!("SSE stream error for provider {}: {}", self.provider, e);
                Poll::Ready(Some(Err(e)))
            }
            Poll::Ready(None) => {
                // Flush any remaining partial frame
                if let Some(frame) = self.parser.flush_buffer() {
                    self.metrics.record_frame();
                    if let Some(usage) = frame.extract_token_usage() {
                        self.track_tokens(usage);
                    }
                }
                debug!("SSE stream ended for provider {}: {:?}", self.provider, self.metrics);
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Heartbeat stream wrapper that injects keep-alive comments
pub struct HeartbeatStream<S> {
    inner: Pin<Box<S>>,
    heartbeat_interval: Duration,
    last_heartbeat: Instant,
    heartbeat_bytes: Bytes,
}

impl<S> HeartbeatStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
{
    /// Create a new heartbeat stream with 30-second keep-alive
    pub fn new(stream: S) -> Self {
        Self::with_interval(stream, Duration::from_secs(30))
    }

    /// Create with custom interval
    pub fn with_interval(stream: S, interval: Duration) -> Self {
        Self {
            inner: Box::pin(stream),
            heartbeat_interval: interval,
            last_heartbeat: Instant::now(),
            heartbeat_bytes: Bytes::from_static(b":keep-alive\n\n"),
        }
    }
}

impl<S> Stream for HeartbeatStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Send,
{
    type Item = Result<Bytes, reqwest::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Check if we need to send a heartbeat
        if self.last_heartbeat.elapsed() >= self.heartbeat_interval {
            self.last_heartbeat = Instant::now();
            return Poll::Ready(Some(Ok(self.heartbeat_bytes.clone())));
        }

        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(item)) => {
                self.last_heartbeat = Instant::now();
                Poll::Ready(Some(item))
            }
            other => other,
        }
    }
}

/// Pooled connection for streaming endpoints
#[derive(Clone)]
pub struct StreamingConnection {
    pub provider: String,
    pub base_url: String,
    pub created_at: Instant,
    pub last_used: Instant,
}

impl StreamingConnection {
    /// Check if connection is stale (unused for > 5 minutes)
    pub fn is_stale(&self) -> bool {
        self.last_used.elapsed() > Duration::from_secs(300)
    }
}

/// Connection pool for streaming endpoints
pub struct StreamingConnectionPool {
    connections: Arc<RwLock<HashMap<String, Vec<StreamingConnection>>>>,
    max_connections_per_provider: usize,
}

impl StreamingConnectionPool {
    /// Create a new connection pool
    pub fn new() -> Self {
        Self::with_capacity(10)
    }

    /// Create with custom capacity per provider
    pub fn with_capacity(max_per_provider: usize) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            max_connections_per_provider: max_per_provider,
        }
    }

    /// Get a connection for a provider
    pub async fn get_connection(&self, provider: &str) -> Option<StreamingConnection> {
        let mut conns = self.connections.write().await;
        if let Some(pool) = conns.get_mut(provider) {
            // Remove stale connections
            pool.retain(|c| !c.is_stale());
            
            // Return most recently used connection
            if let Some(mut conn) = pool.pop() {
                conn.last_used = Instant::now();
                return Some(conn);
            }
        }
        None
    }

    /// Return a connection to the pool
    pub async fn return_connection(&self, conn: StreamingConnection) {
        let mut conns = self.connections.write().await;
        let pool = conns.entry(conn.provider.clone()).or_insert_with(Vec::new);
        
        if pool.len() < self.max_connections_per_provider {
            pool.push(conn);
        }
        // Otherwise, drop the connection (pool is full)
    }

    /// Create a new connection entry
    pub fn create_connection(provider: String, base_url: String) -> StreamingConnection {
        let now = Instant::now();
        StreamingConnection {
            provider,
            base_url,
            created_at: now,
            last_used: now,
        }
    }

    /// Clear all connections
    pub async fn clear(&self) {
        let mut conns = self.connections.write().await;
        conns.clear();
    }

    /// Get pool statistics
    pub async fn stats(&self) -> HashMap<String, usize> {
        let conns = self.connections.read().await;
        conns.iter().map(|(k, v)| (k.clone(), v.len())).collect()
    }
}

impl Default for StreamingConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Factory function to create a tracked SSE stream
pub fn create_tracked_sse_stream(
    stream: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    provider: String,
) -> Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>> {
    let tracked = TrackedSseStream::new(stream, provider);
    Box::pin(tracked)
}

/// Factory function to add heartbeat to a stream
pub fn add_heartbeat<S>(stream: S) -> HeartbeatStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
{
    HeartbeatStream::new(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;

    #[test]
    fn test_sse_frame_parse_simple() {
        let raw = "data: hello world\n\n";
        let frame = SseFrame::parse(raw).unwrap();
        assert_eq!(frame.data, "hello world");
        assert_eq!(frame.event, None);
    }

    #[test]
    fn test_sse_frame_parse_full() {
        let raw = "event: message\nid: 123\nretry: 5000\ndata: hello\ndata: world\n\n";
        let frame = SseFrame::parse(raw).unwrap();
        assert_eq!(frame.event, Some("message".to_string()));
        assert_eq!(frame.id, Some("123".to_string()));
        assert_eq!(frame.retry, Some(5000));
        assert_eq!(frame.data, "hello\nworld");
    }

    #[test]
    fn test_sse_frame_serialization() {
        let frame = SseFrame {
            event: Some("message".to_string()),
            data: "hello\nworld".to_string(),
            id: Some("123".to_string()),
            retry: Some(5000),
        };
        let serialized = frame.to_sse_string();
        assert!(serialized.contains("event: message"));
        assert!(serialized.contains("id: 123"));
        assert!(serialized.contains("retry: 5000"));
        assert!(serialized.contains("data: hello"));
        assert!(serialized.contains("data: world"));
    }

    #[test]
    fn test_sse_frame_token_extraction() {
        let frame = SseFrame {
            event: None,
            data: r#"{"usage":{"total_tokens":150}}"#.to_string(),
            id: None,
            retry: None,
        };
        assert_eq!(frame.extract_token_usage(), Some(150));

        let frame2 = SseFrame {
            event: None,
            data: r#"{"usage":{"prompt_tokens":50,"completion_tokens":100}}"#.to_string(),
            id: None,
            retry: None,
        };
        assert_eq!(frame2.extract_token_usage(), Some(150));
    }

    #[test]
    fn test_sse_frame_done_marker() {
        let frame = SseFrame {
            event: None,
            data: "[DONE]".to_string(),
            id: None,
            retry: None,
        };
        assert!(frame.is_done_marker());
    }

    #[test]
    fn test_frame_parser_complete_frames() {
        let mut parser = SseFrameParser::new();
        let frames = parser.parse_chunk("data: frame1\n\ndata: frame2\n\n");
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].data, "frame1");
        assert_eq!(frames[1].data, "frame2");
    }

    #[test]
    fn test_frame_parser_partial_frame() {
        let mut parser = SseFrameParser::new();
        
        // First chunk: incomplete
        let frames1 = parser.parse_chunk("data: partial");
        assert_eq!(frames1.len(), 0);
        
        // Second chunk: completes the frame
        let frames2 = parser.parse_chunk(" data\n\ndata: new\n\n");
        assert_eq!(frames2.len(), 2);
        assert_eq!(frames2[0].data, "partial data");
        assert_eq!(frames2[1].data, "new");
    }

    #[test]
    fn test_frame_parser_flush() {
        let mut parser = SseFrameParser::new();
        parser.parse_chunk("data: incomplete");
        
        let flushed = parser.flush_buffer();
        assert!(flushed.is_some());
        assert_eq!(flushed.unwrap().data, "incomplete");
        
        // Buffer should be empty now
        let empty = parser.flush_buffer();
        assert!(empty.is_none());
    }

    #[test]
    fn test_frame_parser_buffer_limit() {
        let mut parser = SseFrameParser::with_max_buffer(100);
        
        // Add data that exceeds buffer limit
        let large_data = "data: ".to_string() + &"x".repeat(200);
        let frames = parser.parse_chunk(&large_data);
        
        // Should return empty and clear buffer
        assert_eq!(frames.len(), 0);
        assert!(parser.flush_buffer().is_none());
    }

    #[test]
    fn test_streaming_metrics() {
        let mut metrics = StreamingMetrics::new();
        
        metrics.record_bytes(100);
        metrics.record_frame();
        metrics.record_frame();
        metrics.record_tokens(50);
        
        assert_eq!(metrics.bytes_streamed, 100);
        assert_eq!(metrics.frames_parsed, 2);
        assert_eq!(metrics.tokens_tracked, 50);
        assert!(metrics.duration().is_some());
        
        std::thread::sleep(Duration::from_millis(10));
        assert!(metrics.tokens_per_second() > 0.0);
    }

    #[tokio::test]
    async fn test_tracked_sse_stream() {
        let data = vec![
            Ok(Bytes::from("data: ")),
            Ok(Bytes::from(r#"{"usage":{"total_tokens":100}}"#)),
            Ok(Bytes::from("\n\n")),
        ];
        let stream = stream::iter(data);
        let tracked = TrackedSseStream::new(Box::pin(stream), "test".to_string());
        
        let results: Vec<_> = tracked.collect().await;
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_connection_pool() {
        let pool = StreamingConnectionPool::new();
        
        // Create and return a connection
        let conn = StreamingConnectionPool::create_connection(
            "test".to_string(),
            "https://api.test.com".to_string(),
        );
        pool.return_connection(conn).await;
        
        // Get it back
        let retrieved = pool.get_connection("test").await;
        assert!(retrieved.is_some());
        
        // Pool should be empty now
        let empty = pool.get_connection("test").await;
        assert!(empty.is_none());
        
        // Stats should show 0
        let stats = pool.stats().await;
        assert_eq!(stats.get("test"), Some(&0));
    }

    #[tokio::test]
    async fn test_connection_pool_stale_removal() {
        let pool = StreamingConnectionPool::new();
        
        // Create an old connection
        let mut old_conn = StreamingConnectionPool::create_connection(
            "test".to_string(),
            "https://api.test.com".to_string(),
        );
        old_conn.last_used = Instant::now() - Duration::from_secs(400);
        
        pool.return_connection(old_conn).await;
        
        // Should be removed as stale
        let retrieved = pool.get_connection("test").await;
        assert!(retrieved.is_none());
    }
}
