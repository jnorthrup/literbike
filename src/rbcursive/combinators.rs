// Parser combinator framework with continuations for network protocols
// Zero-allocation, high-performance parsing with SIMD integration

use crate::rbcursive::scanner::SimdScanner;
use std::marker::PhantomData;

/// Core parser trait for network protocol parsing
pub trait Parser<'a, T> {
    type Error;
    /// Parse input, returning result and consumed bytes
    fn parse(&self, input: &'a [u8]) -> ParseResult<T, Self::Error>;
}

// Allow passing references to parsers without moving them
impl<'a, T, P> Parser<'a, T> for &P
where
    P: Parser<'a, T>,
{
    type Error = P::Error;
    #[inline(always)]
    fn parse(&self, input: &'a [u8]) -> ParseResult<T, Self::Error> {
        (*self).parse(input)
    }
}

/// Parse result with continuation support for streaming
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseResult<T, E> {
    /// Parsing completed successfully
    Complete(T, usize),
    /// Need more data to continue parsing
    Incomplete(usize),
    /// Parsing failed with error
    Error(E, usize),
}

impl<T, E> ParseResult<T, E> {
    /// Extract the value if complete
    pub fn into_complete(self) -> Option<(T, usize)> {
        match self {
            Self::Complete(value, consumed) => Some((value, consumed)),
            _ => None,
        }
    }

    /// Check if result is complete
    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Complete(_, _))
    }

    /// Get consumed bytes count
    pub fn consumed(&self) -> usize {
        match self {
            Self::Complete(_, consumed)
            | Self::Incomplete(consumed)
            | Self::Error(_, consumed) => *consumed,
        }
    }

    /// Functor map over the successful value, preserving signaling and consumed count
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> ParseResult<U, E> {
        match self {
            ParseResult::Complete(v, c) => ParseResult::Complete(f(v), c),
            ParseResult::Incomplete(c) => ParseResult::Incomplete(c),
            ParseResult::Error(e, c) => ParseResult::Error(e, c),
        }
    }

    /// Map the error type while preserving signaling and consumed count
    pub fn map_err<F>(self, f: impl FnOnce(E) -> F) -> ParseResult<T, F> {
        match self {
            ParseResult::Complete(v, c) => ParseResult::Complete(v, c),
            ParseResult::Incomplete(c) => ParseResult::Incomplete(c),
            ParseResult::Error(e, c) => ParseResult::Error(f(e), c),
        }
    }

    /// Collapse to a coarse signal for parser outcomes
    pub fn signal(&self) -> Signal {
        match self {
            ParseResult::Complete(_, _) => Signal::Accept,
            ParseResult::Incomplete(_) => Signal::NeedMore,
            ParseResult::Error(_, _) => Signal::Reject,
        }
    }
}

/// Coarse-grained signal for parser outcomes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Accept,
    NeedMore,
    Reject,
}

/// Parse error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    InvalidInput,
    UnexpectedEnd,
    InvalidProtocol,
    InvalidMethod,
    InvalidHeader,
    InvalidLength,
}

/// Basic byte parser
pub struct ByteParser {
    target: u8,
}

impl ByteParser {
    pub fn new(target: u8) -> Self {
        Self { target }
    }
}

impl<'a> Parser<'a, u8> for ByteParser {
    type Error = ParseError;

    fn parse(&self, input: &'a [u8]) -> ParseResult<u8, Self::Error> {
        if input.is_empty() {
            return ParseResult::Incomplete(0);
        }
        if input[0] == self.target {
            ParseResult::Complete(input[0], 1)
        } else {
            ParseResult::Error(ParseError::InvalidInput, 0)
        }
    }
}

/// Take exact number of bytes
pub struct TakeParser {
    count: usize,
}

impl TakeParser {
    pub fn new(count: usize) -> Self {
        Self { count }
    }
}

impl<'a> Parser<'a, &'a [u8]> for TakeParser {
    type Error = ParseError;

    fn parse(&self, input: &'a [u8]) -> ParseResult<&'a [u8], Self::Error> {
        if input.len() < self.count {
            ParseResult::Incomplete(input.len())
        } else {
            ParseResult::Complete(&input[..self.count], self.count)
        }
    }
}

/// Take until delimiter (SIMD-accelerated)
pub struct TakeUntilParser<'s> {
    delimiter: u8,
    scanner: &'s dyn SimdScanner,
}

impl<'s> TakeUntilParser<'s> {
    pub fn new(delimiter: u8, scanner: &'s dyn SimdScanner) -> Self {
        Self { delimiter, scanner }
    }
}

impl<'a, 's> Parser<'a, &'a [u8]> for TakeUntilParser<'s> {
    type Error = ParseError;

    fn parse(&self, input: &'a [u8]) -> ParseResult<&'a [u8], Self::Error> {
        if input.is_empty() {
            return ParseResult::Incomplete(0);
        }
        // Use SIMD to find delimiter
        let positions = self.scanner.scan_bytes(input, &[self.delimiter]);
        if let Some(&pos) = positions.first() {
            ParseResult::Complete(&input[..pos], pos)
        } else {
            // Delimiter not found - need more data
            ParseResult::Incomplete(input.len())
        }
    }
}

/// Take while predicate is true
pub struct TakeWhileParser<F> {
    predicate: F,
}

impl<F> TakeWhileParser<F>
where
    F: Fn(u8) -> bool,
{
    pub fn new(predicate: F) -> Self {
        Self { predicate }
    }
}

impl<'a, F> Parser<'a, &'a [u8]> for TakeWhileParser<F>
where
    F: Fn(u8) -> bool,
{
    type Error = ParseError;

    fn parse(&self, input: &'a [u8]) -> ParseResult<&'a [u8], Self::Error> {
        let mut count = 0;
        for &byte in input {
            if (self.predicate)(byte) {
                count += 1;
            } else {
                break;
            }
        }
        ParseResult::Complete(&input[..count], count)
    }
}

/// Tag parser - match exact byte sequence
pub struct TagParser {
    tag: Vec<u8>,
}

impl TagParser {
    pub fn new(tag: &[u8]) -> Self {
        Self { tag: tag.to_vec() }
    }
}

impl<'a> Parser<'a, &'a [u8]> for TagParser {
    type Error = ParseError;

    fn parse(&self, input: &'a [u8]) -> ParseResult<&'a [u8], Self::Error> {
        if input.len() < self.tag.len() {
            return ParseResult::Incomplete(input.len());
        }
        if input.starts_with(&self.tag) {
            ParseResult::Complete(&input[..self.tag.len()], self.tag.len())
        } else {
            ParseResult::Error(ParseError::InvalidInput, 0)
        }
    }
}

/// Sequence combinator - parse A then B
pub struct SequenceParser<A, B> {
    first: A,
    second: B,
}

impl<A, B> SequenceParser<A, B> {
    pub fn new(first: A, second: B) -> Self {
        Self { first, second }
    }
}

impl<'a, A, B, T1, T2> Parser<'a, (T1, T2)> for SequenceParser<A, B>
where
    A: Parser<'a, T1, Error = ParseError>,
    B: Parser<'a, T2, Error = ParseError>,
{
    type Error = ParseError;

    fn parse(&self, input: &'a [u8]) -> ParseResult<(T1, T2), Self::Error> {
        match self.first.parse(input) {
            ParseResult::Complete(first_result, first_consumed) => {
                match self.second.parse(&input[first_consumed..]) {
                    ParseResult::Complete(second_result, second_consumed) => {
                        ParseResult::Complete(
                            (first_result, second_result),
                            first_consumed + second_consumed,
                        )
                    }
                    ParseResult::Incomplete(consumed) => {
                        ParseResult::Incomplete(first_consumed + consumed)
                    }
                    ParseResult::Error(err, consumed) => {
                        ParseResult::Error(err, first_consumed + consumed)
                    }
                }
            }
            ParseResult::Incomplete(consumed) => ParseResult::Incomplete(consumed),
            ParseResult::Error(err, consumed) => ParseResult::Error(err, consumed),
        }
    }
}

/// Alternative combinator - try A, if fails try B
pub struct AlternativeParser<A, B> {
    first: A,
    second: B,
}

impl<A, B> AlternativeParser<A, B> {
    pub fn new(first: A, second: B) -> Self {
        Self { first, second }
    }
}

impl<'a, A, B, T> Parser<'a, T> for AlternativeParser<A, B>
where
    A: Parser<'a, T, Error = ParseError>,
    B: Parser<'a, T, Error = ParseError>,
{
    type Error = ParseError;

    fn parse(&self, input: &'a [u8]) -> ParseResult<T, Self::Error> {
        match self.first.parse(input) {
            ParseResult::Complete(result, consumed) => ParseResult::Complete(result, consumed),
            ParseResult::Incomplete(consumed) => ParseResult::Incomplete(consumed),
            ParseResult::Error(_, _) => self.second.parse(input),
        }
    }
}

/// Map combinator - transform parser result
pub struct MapParser<P, F, T> {
    parser: P,
    mapper: F,
    _phantom: PhantomData<T>,
}

impl<P, F, T> MapParser<P, F, T> {
    pub fn new(parser: P, mapper: F) -> Self {
        Self { parser, mapper, _phantom: PhantomData }
    }
}

impl<'a, P, F, T, U> Parser<'a, U> for MapParser<P, F, T>
where
    P: Parser<'a, T, Error = ParseError>,
    F: Fn(T) -> U,
{
    type Error = ParseError;

    fn parse(&self, input: &'a [u8]) -> ParseResult<U, Self::Error> {
        match self.parser.parse(input) {
            ParseResult::Complete(result, consumed) => {
                ParseResult::Complete((self.mapper)(result), consumed)
            }
            ParseResult::Incomplete(consumed) => ParseResult::Incomplete(consumed),
            ParseResult::Error(err, consumed) => ParseResult::Error(err, consumed),
        }
    }
}

/// Convenience functions for common parsers
pub fn byte(target: u8) -> ByteParser { ByteParser::new(target) }
/// Alias for byte() to emphasize character literal intent in protocol grammars
pub fn chlit(target: u8) -> ByteParser { byte(target) }
pub fn take(count: usize) -> TakeParser { TakeParser::new(count) }
pub fn take_until<'s>(delimiter: u8, scanner: &'s dyn SimdScanner) -> TakeUntilParser<'s> { TakeUntilParser::new(delimiter, scanner) }
pub fn take_while<F>(predicate: F) -> TakeWhileParser<F> where F: Fn(u8) -> bool { TakeWhileParser::new(predicate) }
pub fn tag(tag: &[u8]) -> TagParser { TagParser::new(tag) }
pub fn sequence<A, B>(first: A, second: B) -> SequenceParser<A, B> { SequenceParser::new(first, second) }
pub fn alternative<A, B>(first: A, second: B) -> AlternativeParser<A, B> { AlternativeParser::new(first, second) }

// ----------------- RangeWhile and Confix combinators -----------------

/// Parse a run of bytes within [start..=end]. Enforces min length and optional max.
pub struct ByteRangeWhileParser {
    start: u8,
    end: u8,
    min: usize,
    max: Option<usize>,
}

impl ByteRangeWhileParser {
    pub fn new(start: u8, end: u8, min: usize, max: Option<usize>) -> Self {
        Self { start, end, min, max }
    }
}

impl<'a> Parser<'a, &'a [u8]> for ByteRangeWhileParser {
    type Error = ParseError;

    fn parse(&self, input: &'a [u8]) -> ParseResult<&'a [u8], Self::Error> {
        if input.is_empty() { return ParseResult::Incomplete(0); }
        let mut len = 0usize;
        let bound = self.max.map(|m| m.min(input.len())).unwrap_or(input.len());
        while len < bound {
            let b = input[len];
            if b < self.start || b > self.end { break; }
            len += 1;
        }
        if len < self.min {
            if input.len() < self.min { ParseResult::Incomplete(input.len()) } else { ParseResult::Error(ParseError::InvalidInput, len) }
        } else {
            ParseResult::Complete(&input[..len], len)
        }
    }
}

/// Convenience constructor for ByteRangeWhileParser
pub fn range_while(start: u8, end: u8, min: usize, max: Option<usize>) -> ByteRangeWhileParser { ByteRangeWhileParser::new(start, end, min, max) }

/// Parse content enclosed by open/close bytes. If allow_nested, balances pairs.
pub struct ConfixParser {
    open: u8,
    close: u8,
    allow_nested: bool,
}

impl ConfixParser {
    pub fn new(open: u8, close: u8, allow_nested: bool) -> Self { Self { open, close, allow_nested } }
}

impl<'a> Parser<'a, &'a [u8]> for ConfixParser {
    type Error = ParseError;

    fn parse(&self, input: &'a [u8]) -> ParseResult<&'a [u8], Self::Error> {
        if input.first() != Some(&self.open) {
            return if input.is_empty() { ParseResult::Incomplete(0) } else { ParseResult::Error(ParseError::InvalidInput, 0) };
        }
        if !self.allow_nested {
            // Simple scan to next close
            for (i, b) in input.iter().enumerate().skip(1) {
                if *b == self.close { return ParseResult::Complete(&input[..=i], i + 1); }
            }
            return ParseResult::Incomplete(input.len());
        }
        // Nested matching
        let mut depth = 1usize;
        let mut i = 1usize;
        while i < input.len() {
            let b = input[i];
            if b == self.open { depth += 1; }
            else if b == self.close {
                depth -= 1;
                if depth == 0 { return ParseResult::Complete(&input[..=i], i + 1); }
            }
            i += 1;
        }
        ParseResult::Incomplete(input.len())
    }
}

/// Convenience constructor for ConfixParser
pub fn confix(open: u8, close: u8, allow_nested: bool) -> ConfixParser { ConfixParser::new(open, close, allow_nested) }

pub fn map<P, F, T>(parser: P, mapper: F) -> MapParser<P, F, T> { MapParser::new(parser, mapper) }

/// Common character classes for network protocols
pub fn is_space(byte: u8) -> bool { matches!(byte, b' ' | b'\t') }
pub fn is_crlf(byte: u8) -> bool { matches!(byte, b'\r' | b'\n') }
pub fn is_alpha(byte: u8) -> bool { matches!(byte, b'A'..=b'Z' | b'a'..=b'z') }
pub fn is_digit(byte: u8) -> bool { matches!(byte, b'0'..=b'9') }
pub fn is_token_char(byte: u8) -> bool {
    // HTTP token characters (RFC 7230)
    matches!(byte,
        b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.' |
        b'^' | b'_' | b'`' | b'|' | b'~' | b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rbcursive::scanner::ScalarScanner;

    #[test]
    fn test_byte_parser() {
        let parser = byte(b'G');
        let input = b"GET";
        match parser.parse(input) {
            ParseResult::Complete(result, consumed) => {
                assert_eq!(result, b'G');
                assert_eq!(consumed, 1);
            }
            _ => panic!("Expected complete parse"),
        }
    }

    #[test]
    fn test_take_parser() {
        let parser = take(3);
        let input = b"GET /test";
        match parser.parse(input) {
            ParseResult::Complete(result, consumed) => {
                assert_eq!(result, b"GET");
                assert_eq!(consumed, 3);
            }
            _ => panic!("Expected complete parse"),
        }
    }

    #[test]
    fn test_tag_parser() {
        let parser = tag(b"GET");
        let input = b"GET /path";
        match parser.parse(input) {
            ParseResult::Complete(result, consumed) => {
                assert_eq!(result, b"GET");
                assert_eq!(consumed, 3);
            }
            _ => panic!("Expected complete parse"),
        }
    }

    #[test]
    fn test_take_until_simd() {
        let scanner = ScalarScanner::new();
        let parser = take_until(b' ', &scanner);
        let input = b"GET /path";
        match parser.parse(input) {
            ParseResult::Complete(result, consumed) => {
                assert_eq!(result, b"GET");
                assert_eq!(consumed, 3);
            }
            _ => panic!("Expected complete parse"),
        }
    }

    #[test]
    fn test_sequence_combinator() {
        let parser = sequence(tag(b"GET"), sequence(byte(b' '), tag(b"/path")));
        let input = b"GET /path";
        match parser.parse(input) {
            ParseResult::Complete((method, (space, path)), consumed) => {
                assert_eq!(method, b"GET");
                assert_eq!(space, b' ');
                assert_eq!(path, b"/path");
                assert_eq!(consumed, 9);
            }
            _ => panic!("Expected complete parse"),
        }
    }

    #[test]
    fn test_alternative_combinator() {
        let parser = alternative(tag(b"GET"), tag(b"POST"));
        let input1 = b"GET /";
        match parser.parse(input1) {
            ParseResult::Complete(result, consumed) => {
                assert_eq!(result, b"GET");
                assert_eq!(consumed, 3);
            }
            _ => panic!("Expected complete parse for GET"),
        }
        let input2 = b"POST /";
        match parser.parse(input2) {
            ParseResult::Complete(result, consumed) => {
                assert_eq!(result, b"POST");
                assert_eq!(consumed, 4);
            }
            _ => panic!("Expected complete parse for POST"),
        }
    }

    #[test]
    fn test_incomplete_parsing() {
        let parser = tag(b"GET");
        let input = b"GE"; // Incomplete input
        match parser.parse(input) {
            ParseResult::Incomplete(consumed) => {
                assert_eq!(consumed, 2);
            }
            _ => panic!("Expected incomplete parse"),
        }
    }
}