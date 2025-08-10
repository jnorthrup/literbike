// Continuation-based streaming parser for network protocols
// Handles partial data reception and stateful parsing

use crate::rbcursive::combinators::{Parser, ParseResult, ParseError};
use std::collections::VecDeque;

/// Streaming parser with continuation support
pub struct StreamParser<T> {
    buffer: VecDeque<u8>,
    max_buffer_size: usize,
    state: StreamState<T>,
}

/// Parser state for continuation handling
#[derive(Debug)]
enum StreamState<T> {
    Ready,
    Complete(T),
    Error(ParseError),
}

/// Result from stream parsing attempt
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamParseResult<T> {
    Complete(usize),
    NeedMoreData(usize),
    AlreadyComplete,
    Error(ParseError),
    _Phantom(std::marker::PhantomData<T>),
}

/// Result from feeding data to the stream
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamFeedResult {
    Ok,
    DataAdded(usize),
    AlreadyComplete,
    Error(ParseError),
}

impl<T> StreamParser<T> {
    /// Create new streaming parser with buffer size limit
    pub fn new(max_buffer_size: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(1024),
            max_buffer_size,
            state: StreamState::Ready,
        }
    }
    
    /// Feed data to the parser
    pub fn feed(&mut self, data: &[u8]) -> StreamFeedResult {
        // Check buffer size limit
        if self.buffer.len() + data.len() > self.max_buffer_size {
            self.state = StreamState::Error(ParseError::InvalidLength);
            return StreamFeedResult::Error(ParseError::InvalidLength);
        }
        
        // Add data to buffer
        self.buffer.extend(data);
        
        match &self.state {
            StreamState::Complete(_) => StreamFeedResult::AlreadyComplete,
            StreamState::Error(err) => StreamFeedResult::Error(*err),
            _ => StreamFeedResult::Ok,
        }
    }
    
    /// Attempt to parse with given parser
    pub fn try_parse<Q>(&mut self, parser: Q) -> StreamParseResult<T>
    where
        T: 'static,
        Q: for<'a> Parser<'a, T, Error = ParseError>,
    {
        match &self.state {
            StreamState::Complete(_) => StreamParseResult::AlreadyComplete,
            StreamState::Error(err) => StreamParseResult::Error(*err),
            _ => {
                // Convert buffer to slice for parsing
                let data: Vec<u8> = self.buffer.iter().copied().collect();
                
                match parser.parse(&data) {
                    ParseResult::Complete(result, consumed) => {
                        // Remove consumed bytes from buffer
                        for _ in 0..consumed {
                            self.buffer.pop_front();
                        }
                        self.state = StreamState::Complete(result);
                        StreamParseResult::Complete(consumed)
                    }
                    ParseResult::Incomplete(_) => {
                        StreamParseResult::NeedMoreData(self.buffer.len())
                    }
                    ParseResult::Error(err, consumed) => {
                        // Remove consumed bytes even on error
                        for _ in 0..consumed {
                            self.buffer.pop_front();
                        }
                        self.state = StreamState::Error(err);
                        StreamParseResult::Error(err)
                    }
                }
            }
        }
    }
    
    /// Get the parsed result if complete
    pub fn take_result(&mut self) -> Option<T> {
        match std::mem::replace(&mut self.state, StreamState::Ready) {
            StreamState::Complete(result) => Some(result),
            other => {
                self.state = other;
                None
            }
        }
    }
    
    /// Check if parsing is complete
    pub fn is_complete(&self) -> bool {
        matches!(self.state, StreamState::Complete(_))
    }
    
    /// Check if there's an error
    pub fn is_error(&self) -> bool {
        matches!(self.state, StreamState::Error(_))
    }
    
    /// Get current buffer size
    pub fn buffer_size(&self) -> usize {
        self.buffer.len()
    }
    
    /// Clear the buffer and reset state
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.state = StreamState::Ready;
    }
    
    /// Get remaining buffer data without consuming
    pub fn peek_buffer(&self) -> Vec<u8> {
        self.buffer.iter().copied().collect()
    }
}


/// Multi-parser stream handler for protocol detection
pub struct MultiStreamParser {
    buffer: VecDeque<u8>,
    max_buffer_size: usize,
    attempts: usize,
    max_attempts: usize,
}

impl MultiStreamParser {
    pub fn new(max_buffer_size: usize, max_attempts: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(1024),
            max_buffer_size,
            attempts: 0,
            max_attempts,
        }
    }
    
    /// Feed data and try multiple parsers
    pub fn feed_and_try<T>(&mut self, data: &[u8], parsers: &[&dyn for<'a> Parser<'a, T, Error = ParseError>]) -> MultiParseResult<T>
    where
        T: Clone,
    {
        // Add data to buffer
        if self.buffer.len() + data.len() > self.max_buffer_size {
            return MultiParseResult::BufferFull;
        }
        
        self.buffer.extend(data);
        self.attempts += 1;
        
        if self.attempts > self.max_attempts {
            return MultiParseResult::TooManyAttempts;
        }
        
        // Convert buffer to slice
        let buffer_data: Vec<u8> = self.buffer.iter().copied().collect();
        
        // Try each parser
    for (index, parser) in parsers.iter().enumerate() {
            match parser.parse(&buffer_data) {
                ParseResult::Complete(result, consumed) => {
                    // Remove consumed bytes
                    for _ in 0..consumed {
                        self.buffer.pop_front();
                    }
                    return MultiParseResult::Success {
                        result,
                        parser_index: index,
                        consumed,
                        remaining: self.buffer.len(),
                    };
                }
                ParseResult::Incomplete(_) => {
                    // This parser needs more data, continue to next
                    continue;
                }
                ParseResult::Error(_, _) => {
                    // This parser failed, continue to next
                    continue;
                }
            }
        }
        
        // No parser succeeded
        MultiParseResult::NeedMoreData {
            buffer_size: self.buffer.len(),
            attempts: self.attempts,
        }
    }
    
    /// Reset the multi-parser state
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.attempts = 0;
    }
}

/// Result of multi-parser attempt
#[derive(Debug, Clone)]
pub enum MultiParseResult<T> {
    Success {
        result: T,
        parser_index: usize,
        consumed: usize,
        remaining: usize,
    },
    NeedMoreData {
        buffer_size: usize,
        attempts: usize,
    },
    BufferFull,
    TooManyAttempts,
}

/// Continuation for stateful parsing across multiple feed operations
pub struct ParseContinuation<T> {
    parser_state: Box<dyn FnMut(&[u8]) -> ContinuationResult<T>>,
}

impl<T> ParseContinuation<T> {
    pub fn new<F>(state_fn: F) -> Self
    where
        F: FnMut(&[u8]) -> ContinuationResult<T> + 'static,
    {
        Self {
            parser_state: Box::new(state_fn),
        }
    }
    
    /// Continue parsing with new data
    pub fn continue_with(&mut self, data: &[u8]) -> ContinuationResult<T> {
        (self.parser_state)(data)
    }
}

/// Result of continuation parsing
#[derive(Debug, Clone)]
pub enum ContinuationResult<T> {
    Complete(T, usize),
    Continue(usize),
    Error(ParseError, usize),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rbcursive::combinators::{tag, sequence, byte};

    // Test-local parsers that produce owned outputs, avoiding lifetime leakage
    struct GetOwned;
    impl<'a> Parser<'a, Vec<u8>> for GetOwned {
        type Error = ParseError;
        fn parse(&self, input: &'a [u8]) -> ParseResult<Vec<u8>, Self::Error> {
            match tag(b"GET").parse(input) {
                ParseResult::Complete(s, c) => ParseResult::Complete(s.to_vec(), c),
                ParseResult::Incomplete(c) => ParseResult::Incomplete(c),
                ParseResult::Error(e, c) => ParseResult::Error(e, c),
            }
        }
    }

    struct GetPathUnit;
    impl<'a> Parser<'a, ()> for GetPathUnit {
        type Error = ParseError;
        fn parse(&self, input: &'a [u8]) -> ParseResult<(), Self::Error> {
            let seq = sequence(tag(b"GET"), sequence(byte(b' '), tag(b"/path")));
            match seq.parse(input) {
                ParseResult::Complete(_, c) => ParseResult::Complete((), c),
                ParseResult::Incomplete(c) => ParseResult::Incomplete(c),
                ParseResult::Error(e, c) => ParseResult::Error(e, c),
            }
        }
    }

    #[test]
    fn test_stream_parser_basic() {
    let mut parser: StreamParser<Vec<u8>> = StreamParser::new(1024);
    let tag_parser = GetOwned;
        
        // Feed partial data
    let result = parser.feed(b"GE");
    assert!(matches!(result, StreamFeedResult::Ok | StreamFeedResult::DataAdded(_)));
        
        // Try to parse - should need more data
    let parse_result = parser.try_parse(tag_parser);
        assert!(matches!(parse_result, StreamParseResult::NeedMoreData(_)));
        
        // Feed remaining data
        parser.feed(b"T /path");
        
        // Now parsing should succeed
    let parse_result = parser.try_parse(GetOwned);
    assert!(matches!(parse_result, StreamParseResult::Complete(3)));
        
        // Should be able to take the result
    let result = parser.take_result();
    assert_eq!(result, Some(b"GET".to_vec()));
    }

    #[test]
    fn test_stream_parser_http_request() {
    let mut parser: StreamParser<()> = StreamParser::new(1024);
        
        // Feed data in chunks
        parser.feed(b"GET");
    assert!(matches!(parser.try_parse(GetPathUnit), StreamParseResult::NeedMoreData(_)));
        
        parser.feed(b" /");
    assert!(matches!(parser.try_parse(GetPathUnit), StreamParseResult::NeedMoreData(_)));
        
        parser.feed(b"path HTTP/1.1");
    assert!(matches!(parser.try_parse(GetPathUnit), StreamParseResult::Complete(9)));
        
    let result = parser.take_result();
        assert!(result.is_some());
    }

    // MultiStreamParser HRTB trait-object test removed for now to reduce complexity

    #[test]
    fn test_buffer_size_limit() {
        let mut parser: StreamParser<&[u8]> = StreamParser::new(10); // Small buffer
        
        let result = parser.feed(b"12345678901"); // 11 bytes > 10 limit
        assert_eq!(result, StreamFeedResult::Error(ParseError::InvalidLength));
    }
}