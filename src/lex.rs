// JSON spec: https://www.rfc-editor.org/info/rfc8259/

pub struct Lexer<'data> {
    data: &'data [u8],
    offset: usize,
}

#[derive(Debug)]
pub struct JsonError {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
    pub kind: JsonErrorKind,
}

#[derive(Debug)]
pub enum JsonErrorKind {
    InvalidToken,
    IntegerTooLarge,
    ExpectedValue,
    ExpectedNumber,
    ExpectedString,
    ExpectedFieldName,
    ExpectedColon,
    ExpectedComma,
    ExpectedObject,
    ExpectedArray,
    ExpectedCommaOrEndObject,
    ExpectedCommaOrEndArray,
    UnterminatedString,
    InvalidEscapeSequence,
    InvalidControlCharacterInString,
    InvalidUtf8,
    UnknownField,
}

impl std::error::Error for JsonError {}
use std::{borrow::Cow, slice::SliceIndex};

use JsonErrorKind::*;

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "JSON error at line {}, column {}: {:?}",
            self.line, self.column, self.kind
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonTokenKind {
    // Single-character tokens
    StartArray,
    EndArray,
    StartObject,
    EndObject,
    Colon,
    Comma,
    // Literals
    Null,
    True,
    False,
    // Complex tokens
    PendingNumber,
    PendingString,
}

// Simple utility methods
impl<'data> Lexer<'data> {
    pub fn new(data: &'data [u8]) -> Self {
        Lexer { data, offset: 0 }
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn error_at(&self, offset: usize, kind: JsonErrorKind) -> JsonError {
        let mut line = 1;
        let mut column = 1;
        for &byte in self.get(..offset).unwrap() {
            if byte == b'\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        JsonError {
            offset,
            line,
            column,
            kind,
        }
    }

    pub fn error(&self, kind: JsonErrorKind) -> JsonError {
        self.error_at(self.offset, kind)
    }

    fn get<I: SliceIndex<[u8]>>(&self, index: I) -> Option<&'data I::Output> {
        self.data.get(index)
    }

    fn seek(&mut self, offset: isize) -> Option<usize> {
        let new_offset = (self.offset as isize + offset) as usize;
        if new_offset <= self.data.len() {
            self.offset = new_offset;
            Some(new_offset)
        } else {
            None
        }
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.offset).copied()
    }

    fn next(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.offset += 1;
        Some(byte)
    }

    fn next_if<F>(&mut self, predicate: F) -> Option<u8>
    where
        F: FnOnce(u8) -> bool,
    {
        let byte = self.peek()?;
        if predicate(byte) {
            self.offset += 1;
            Some(byte)
        } else {
            None
        }
    }

    fn next_is(&mut self, expected: u8) -> bool {
        self.next_if(|byte| byte == expected).is_some()
    }

    fn peek_n(&self, n: usize) -> Option<&'data [u8]> {
        self.data.get(self.offset..self.offset + n)
    }

    fn next_n(&mut self, n: usize) -> Option<&'data [u8]> {
        let slice = self.peek_n(n)?;
        self.offset += n;
        Some(slice)
    }

    fn next_n_if<F>(&mut self, n: usize, predicate: F) -> Option<&'data [u8]>
    where
        F: Fn(&[u8]) -> bool,
    {
        let slice = self.peek_n(n)?;
        if predicate(slice) {
            self.offset += n;
            Some(slice)
        } else {
            None
        }
    }

    fn next_n_is(&mut self, expected: &[u8]) -> bool {
        self.next_n_if(expected.len(), |slice| slice == expected)
            .is_some()
    }
}

// Actual JSON parsing
impl<'data> Lexer<'data> {
    pub fn next_token(&mut self) -> Result<JsonTokenKind, JsonError> {
        Ok(loop {
            break match self.next() {
                None => Err(self.error(ExpectedValue))?,
                Some(b' ' | b'\n' | b'\r' | b'\t') => continue,
                Some(b'[') => JsonTokenKind::StartArray,
                Some(b']') => JsonTokenKind::EndArray,
                Some(b'{') => JsonTokenKind::StartObject,
                Some(b'}') => JsonTokenKind::EndObject,
                Some(b':') => JsonTokenKind::Colon,
                Some(b',') => JsonTokenKind::Comma,
                Some(b'n') if self.next_n_is(b"ull") => JsonTokenKind::Null,
                Some(b't') if self.next_n_is(b"rue") => JsonTokenKind::True,
                Some(b'f') if self.next_n_is(b"alse") => JsonTokenKind::False,
                Some(b'-' | b'0'..=b'9') => {
                    self.seek(-1); // put the digit back
                    JsonTokenKind::PendingNumber
                }
                Some(b'"') => {
                    self.seek(-1); // put the quote back
                    JsonTokenKind::PendingString
                }
                _ => Err(self.error(InvalidToken))?,
            };
        })
    }

    fn expect_token_kind(
        &mut self,
        expected: JsonTokenKind,
        error: JsonErrorKind,
    ) -> Result<(), JsonError> {
        (self.next_token()? == expected).ok_or_else(|| self.error(error))
    }

    pub fn expect_array(&mut self) -> Result<(), JsonError> {
        self.expect_token_kind(JsonTokenKind::StartArray, ExpectedArray)
    }

    pub fn expect_object(&mut self) -> Result<(), JsonError> {
        self.expect_token_kind(JsonTokenKind::StartObject, ExpectedValue)
    }

    pub fn expect_colon(&mut self) -> Result<(), JsonError> {
        self.expect_token_kind(JsonTokenKind::Colon, ExpectedColon)
    }

    pub fn expect_string(&mut self) -> Result<(), JsonError> {
        self.expect_token_kind(JsonTokenKind::PendingString, ExpectedString)
    }

    pub fn expect_number(&mut self) -> Result<(), JsonError> {
        self.expect_token_kind(JsonTokenKind::PendingNumber, ExpectedNumber)
    }

    pub fn read_integer(&mut self) -> Result<i128, JsonError> {
        let start = self.offset;
        let negative = self.next_is(b'-');

        let mut n: i128 = (self
            .next_if(|b| b.is_ascii_digit())
            .ok_or_else(|| self.error_at(start, InvalidToken))?
            - b'0') as i128;

        while let Some(b) = self.next_if(|b| b.is_ascii_digit()) {
            n = n
                .checked_mul(10)
                .and_then(|n| n.checked_add((b - b'0') as i128))
                .ok_or_else(|| self.error_at(start, IntegerTooLarge))?;
        }

        if matches!(self.peek(), Some(b'.' | b'e' | b'E')) {
            Err(self.error_at(start, ExpectedNumber))?;
        }

        Ok(if negative { -n } else { n })
    }

    /// Advance the position until we find a *special character*:
    /// - `"` (double quote)
    /// - `\` (backslash)
    /// - illegal characters (control characters 0x00-0x1F)
    ///
    /// Returns the slice of bytes we skipped over, and the special character we found.
    fn skip_to_string_special(&mut self) -> Result<(&'data [u8], u8), JsonError> {
        let start = self.offset;
        // FIXME(optimize): Use SWAR/SIMD to do this multiple bytes at a time
        let special = loop {
            let byte = self.next().ok_or_else(|| self.error(UnterminatedString))?;
            if matches!(byte, b'"' | b'\\' | 0..=0x1F) {
                break byte;
            }
        };
        Ok((self.get(start..self.offset - 1).unwrap(), special))
    }

    pub fn read_string_wtf8(&mut self) -> Result<Cow<'data, [u8]>, JsonError> {
        self.next_is(b'"')
            .ok_or_else(|| self.error(ExpectedString))?;

        let (head, special) = self.skip_to_string_special()?;
        if special == b'"' {
            // The unescaped head covers the entire string, we can borrow it
            return Ok(Cow::Borrowed(head));
        }

        // There was an escape sequence or illegal character

        fn wtf8_encode(surrogate: u16) -> [u8; 3] {
            debug_assert!((0xD800..=0xDFFF).contains(&surrogate));
            [
                0xED,
                0xA0 | ((surrogate >> 6) & 0x1F) as u8,
                0x80 | (surrogate & 0x3F) as u8,
            ]
        }

        let mut buffer = Vec::with_capacity(head.len() + 4);
        buffer.extend_from_slice(head);

        let mut special = special;
        loop {
            if special == b'\\' {
                let esc = self.next().ok_or_else(|| self.error(UnterminatedString))?;
                match esc {
                    b'"' => buffer.push(b'"'),
                    b'\\' => buffer.push(b'\\'),
                    b'/' => buffer.push(b'/'),
                    b'b' => buffer.push(b'\x08'), // backspace
                    b'f' => buffer.push(b'\x0C'), // form feed
                    b'n' => buffer.push(b'\n'),   // newline
                    b'r' => buffer.push(b'\r'),   // carriage return
                    b't' => buffer.push(b'\t'),   // tab
                    b'u' => {
                        let lead_bytes = self
                            .next_n(4)
                            .ok_or_else(|| self.error(UnterminatedString))?;
                        let lead_value = u16::from_ascii_bytes_radix(lead_bytes, 16)
                            .map_err(|_| self.error(InvalidEscapeSequence))?;
                        match lead_value {
                            0xD800..=0xDBFF => {
                                // high surrogate, expect a low surrogate next

                                if !self.next_n_is(b"\\u") {
                                    // Not followed by a valid \u escape sequence
                                    buffer.extend_from_slice(&wtf8_encode(lead_value));
                                    continue;
                                }

                                let trail_bytes = &self
                                    .next_n(4)
                                    .ok_or_else(|| self.error(UnterminatedString))?[2..];
                                let trail_value = u16::from_ascii_bytes_radix(trail_bytes, 16)
                                    .map_err(|_| self.error(InvalidEscapeSequence))?;
                                if !(0xDC00..=0xDFFF).contains(&trail_value) {
                                    // Not followed by a valid low surrogate
                                    buffer.extend_from_slice(&wtf8_encode(lead_value));
                                    continue;
                                }

                                let code_point = 0x10000
                                    + (((lead_value as u32 - 0xD800) << 10)
                                        | (trail_value as u32 - 0xDC00));
                                // Safety: code_point is never a surrogate
                                let char = unsafe { char::from_u32_unchecked(code_point) };

                                // FIXME(optimize): I feel like this might do unnecessary stack
                                // copying, but I haven't checked the assembly output.
                                let mut buf = [0; 4];
                                buffer.extend_from_slice(char.encode_utf8(&mut buf).as_bytes());
                            }
                            0xDC00..=0xDFFF => {
                                // low surrogate without a preceding high surrogate
                                buffer.extend_from_slice(&wtf8_encode(lead_value));
                            }
                            code_point => {
                                // Safety: code_point is never a surrogate
                                let char = unsafe { char::from_u32_unchecked(code_point as u32) };

                                // 3 bytes is enough here because the code point is below 0xFFFF
                                let mut buf = [0; 3];
                                buffer.extend_from_slice(char.encode_utf8(&mut buf).as_bytes());
                            }
                        }
                    }
                    _ => Err(self.error_at(self.offset - 1, InvalidEscapeSequence))?,
                }
            } else if special <= 0x1F {
                Err(self.error_at(self.offset - 1, InvalidControlCharacterInString))?
            }
            let (head, next_special) = self.skip_to_string_special()?;
            buffer.extend_from_slice(head);
            special = next_special;
            if special == b'"' {
                return Ok(Cow::Owned(buffer));
            }
        }
    }

    /// Read a string, assuming the position on the opening quote
    pub fn read_string_utf8(&mut self) -> Result<Cow<'data, str>, JsonError> {
        match self.read_string_wtf8()? {
            Cow::Borrowed(wtf8) => {
                let s = str::from_utf8(wtf8).map_err(|_| self.error(InvalidUtf8))?;
                Ok(Cow::Borrowed(s))
            }
            Cow::Owned(wtf8) => {
                let s = String::from_utf8(wtf8).map_err(|_| self.error(InvalidUtf8))?;
                Ok(Cow::Owned(s))
            }
        }
    }
}
