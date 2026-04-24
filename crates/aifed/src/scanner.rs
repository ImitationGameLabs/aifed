//! Character-level scanner for batch edit input.
//!
//! Walks input character-by-character, treating newlines as ordinary
//! whitespace. Produces a token stream consumed by the parser in [`batch`].

use crate::error::{Error, Result};

/// Scanner tokens. Only distinguishes syntax structure — the parser decides
/// whether an Unquoted token is a hashline, content, or something else.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Token<'a> {
    Plus,
    Minus,
    Equals,
    RangeStart,
    RangeEnd,
    Comma,
    Quoted(&'a str),
    Unquoted(&'a str),
}

/// Owned peek result that does not borrow the [`Scanner`].
#[derive(Debug, Clone, PartialEq)]
pub enum PeekResult<'a> {
    Token(Token<'a>),
    Err(String),
}

/// Character-level scanner that iterates over input producing [`Token`]s.
///
/// Whitespace (including newlines) is treated as a token separator, so
/// operations can span multiple lines.
pub struct Scanner<'a> {
    input: &'a str,
    pos: usize,
    line: usize,
    column: usize,
    peeked: Option<Result<Token<'a>>>,
}

impl<'a> Scanner<'a> {
    pub fn new(input: &'a str) -> Self {
        Scanner { input, pos: 0, line: 1, column: 1, peeked: None }
    }

    /// Consume and return the next token.
    pub fn next_token(&mut self) -> Option<Result<Token<'a>>> {
        self.peeked.take().or_else(|| self.scan_token())
    }

    /// Peek at the next token without consuming it.
    ///
    /// Returns an owned [`PeekResult`] that does not borrow `self`, allowing
    /// subsequent calls to `next_token` after inspecting the result.
    pub fn peek_token(&mut self) -> Option<PeekResult<'a>> {
        if self.peeked.is_none() {
            self.peeked = self.scan_token();
        }
        self.peeked.as_ref().map(|r| match r {
            Ok(tok) => PeekResult::Token(*tok),
            Err(e) => PeekResult::Err(e.to_string()),
        })
    }

    // ── internal scanning ────────────────────────────────────────────

    /// Current line number (1-based).
    pub fn line(&self) -> usize {
        self.line
    }

    /// Current column number (1-based).
    #[allow(dead_code)]
    pub fn column(&self) -> usize {
        self.column
    }

    fn scan_token(&mut self) -> Option<Result<Token<'a>>> {
        self.skip_whitespace();

        let ch = self.input[self.pos..].chars().next()?;
        match ch {
            '#' => {
                self.skip_to_eol();
                self.scan_token() // comment → skip, try next
            }
            '+' => {
                self.advance(ch);
                Some(Ok(Token::Plus))
            }
            '-' => {
                self.advance(ch);
                Some(Ok(Token::Minus))
            }
            '=' => {
                self.advance(ch);
                Some(Ok(Token::Equals))
            }
            '[' => {
                self.advance(ch);
                Some(Ok(Token::RangeStart))
            }
            ']' => {
                self.advance(ch);
                Some(Ok(Token::RangeEnd))
            }
            ',' => {
                self.advance(ch);
                Some(Ok(Token::Comma))
            }
            '"' => self.scan_quoted(),
            _ => self.scan_unquoted(),
        }
    }

    /// Advance position tracking by one character.
    fn advance(&mut self, ch: char) {
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        self.pos += ch.len_utf8();
    }

    /// Skip over whitespace (including newlines).
    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() {
            let ch = self.input[self.pos..].chars().next().unwrap();
            if !ch.is_ascii_whitespace() {
                break;
            }
            self.advance(ch);
        }
    }

    /// Skip from `#` to end of line (not consuming the newline).
    fn skip_to_eol(&mut self) {
        while self.pos < self.input.len() {
            let ch = self.input[self.pos..].chars().next().unwrap();
            if ch == '\n' {
                break;
            }
            // update column for non-newline chars
            self.column += 1;
            self.pos += ch.len_utf8();
        }
    }

    /// Scan a quoted string `"..."` with backslash escape support.
    ///
    /// Returns the raw content between quotes (escape sequences intact).
    /// The caller (`decode_content`) processes escapes.
    fn scan_quoted(&mut self) -> Option<Result<Token<'a>>> {
        // Skip opening '"'
        self.advance('"');
        let start = self.pos;

        while self.pos < self.input.len() {
            let ch = self.input[self.pos..].chars().next().unwrap();
            if ch == '\\' {
                self.pos += ch.len_utf8();
                self.column += 1;
                if self.pos < self.input.len() {
                    let next = self.input[self.pos..].chars().next().unwrap();
                    self.pos += next.len_utf8();
                    self.column += 1;
                }
            } else if ch == '"' {
                let raw = &self.input[start..self.pos];
                self.advance(ch);
                return Some(Ok(Token::Quoted(raw)));
            } else if ch == '\n' {
                return Some(Err(Error::Syntax {
                    line: self.line,
                    column: self.column,
                    reason: "Newline inside quoted string".to_string(),
                }));
            } else {
                self.advance(ch);
            }
        }

        Some(Err(Error::Syntax {
            line: self.line,
            column: self.column,
            reason: "Unterminated string".to_string(),
        }))
    }

    /// Scan an unquoted token (continuous non-whitespace, non-structural
    /// characters).
    fn scan_unquoted(&mut self) -> Option<Result<Token<'a>>> {
        let start = self.pos;
        while self.pos < self.input.len() {
            let ch = self.input[self.pos..].chars().next().unwrap();
            // Structural chars and whitespace end an unquoted token.
            if ch.is_ascii_whitespace() || matches!(ch, '[' | ']' | ',' | '"') {
                break;
            }
            self.pos += ch.len_utf8();
            if ch == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }

        let raw = &self.input[start..self.pos];
        if raw.is_empty() {
            self.scan_token() // degenerate — retry
        } else {
            Some(Ok(Token::Unquoted(raw)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Collect all tokens from input, flattening errors into panics.
    fn collect(input: &str) -> Vec<Token<'_>> {
        let mut scanner = Scanner::new(input);
        let mut tokens = Vec::new();
        while let Some(tok) = scanner.next_token() {
            tokens.push(tok.unwrap());
        }
        tokens
    }

    // ── basic tokens ────────────────────────────────────────────────

    #[test]
    fn test_basic_op() {
        let t = collect("+ 42:AB \"x\"");
        assert_eq!(
            t,
            vec![Token::Plus, Token::Unquoted("42:AB"), Token::Quoted("x")]
        );
    }

    #[test]
    fn test_delete() {
        let t = collect("- 15:7M");
        assert_eq!(t, vec![Token::Minus, Token::Unquoted("15:7M")]);
    }

    #[test]
    fn test_replace() {
        let t = collect("= 10:3K \"replacement\"");
        assert_eq!(
            t,
            vec![Token::Equals, Token::Unquoted("10:3K"), Token::Quoted("replacement")]
        );
    }

    #[test]
    fn test_multiple_quoted() {
        let t = collect("+ 1:AA \"a\" \"b\"");
        assert_eq!(
            t,
            vec![Token::Plus, Token::Unquoted("1:AA"), Token::Quoted("a"), Token::Quoted("b"),]
        );
    }

    #[test]
    fn test_unquoted_content() {
        let t = collect("+ 42:AB simple_text");
        assert_eq!(
            t,
            vec![Token::Plus, Token::Unquoted("42:AB"), Token::Unquoted("simple_text")]
        );
    }

    #[test]
    fn test_range_delete() {
        let t = collect("- [2:AA,5:BB]");
        assert_eq!(
            t,
            vec![
                Token::Minus,
                Token::RangeStart,
                Token::Unquoted("2:AA"),
                Token::Comma,
                Token::Unquoted("5:BB"),
                Token::RangeEnd,
            ]
        );
    }

    #[test]
    fn test_range_delete_single_line() {
        let t = collect("- [3:AB,3:AB]");
        assert_eq!(
            t,
            vec![
                Token::Minus,
                Token::RangeStart,
                Token::Unquoted("3:AB"),
                Token::Comma,
                Token::Unquoted("3:AB"),
                Token::RangeEnd,
            ]
        );
    }

    // ── comments and blanks ─────────────────────────────────────────

    #[test]
    fn test_comment_skipped() {
        let t = collect("# comment\n+ 1:AA \"x\"");
        assert_eq!(
            t,
            vec![Token::Plus, Token::Unquoted("1:AA"), Token::Quoted("x")]
        );
    }

    #[test]
    fn test_blank_lines() {
        let t = collect("\n\n+ 1:AA \"x\"\n\n");
        assert_eq!(
            t,
            vec![Token::Plus, Token::Unquoted("1:AA"), Token::Quoted("x")]
        );
    }

    #[test]
    fn test_empty_input() {
        let t = collect("");
        assert!(t.is_empty());
    }

    #[test]
    fn test_only_comments() {
        let t = collect("# comment\n# another\n");
        assert!(t.is_empty());
    }

    #[test]
    fn test_mixed_comments_and_ops() {
        let t = collect("# header\n+ 1:AA \"a\"\n# separator\n- 2:BB\n# footer");
        assert_eq!(t.len(), 5);
    }

    // ── newlines as separators ──────────────────────────────────────

    #[test]
    fn test_newline_as_separator() {
        let t = collect("+\n42:AB\n\"x\"");
        assert_eq!(
            t,
            vec![Token::Plus, Token::Unquoted("42:AB"), Token::Quoted("x")]
        );
    }

    #[test]
    fn test_multiline_range() {
        let t = collect("-\n[2:AA\n,\n5:BB]");
        assert_eq!(
            t,
            vec![
                Token::Minus,
                Token::RangeStart,
                Token::Unquoted("2:AA"),
                Token::Comma,
                Token::Unquoted("5:BB"),
                Token::RangeEnd,
            ]
        );
    }

    #[test]
    fn test_comments_between_tokens() {
        // `#` at line-start is a comment even when between tokens
        let t = collect("+\n# note: line 42\n42:AB\n\"x\"");
        assert_eq!(
            t,
            vec![Token::Plus, Token::Unquoted("42:AB"), Token::Quoted("x")]
        );
    }

    #[test]
    fn test_crlf() {
        let t = collect("+\r\n42:AB\r\n\"x\"");
        assert_eq!(
            t,
            vec![Token::Plus, Token::Unquoted("42:AB"), Token::Quoted("x")]
        );
    }

    // ── quoted string edge cases ───────────────────────────────────

    #[test]
    fn test_quoted_empty() {
        let t = collect("+ 1:AA \"\"");
        assert_eq!(
            t,
            vec![Token::Plus, Token::Unquoted("1:AA"), Token::Quoted("")]
        );
    }

    #[test]
    fn test_quoted_with_spaces() {
        let t = collect("+ 1:AA \"content with spaces\"");
        assert_eq!(
            t,
            vec![Token::Plus, Token::Unquoted("1:AA"), Token::Quoted("content with spaces")]
        );
    }

    #[test]
    fn test_quoted_escapes_raw() {
        // Escapes are passed through raw — decode_content handles them.
        let t = collect("+ 10:AB \"say \\\"hello\\\"\"");
        assert_eq!(
            t,
            vec![Token::Plus, Token::Unquoted("10:AB"), Token::Quoted("say \\\"hello\\\"")]
        );
    }

    #[test]
    fn test_quoted_backslash() {
        let t = collect("+ 10:AB \"path\\\\to\\\\file\"");
        assert_eq!(
            t,
            vec![Token::Plus, Token::Unquoted("10:AB"), Token::Quoted("path\\\\to\\\\file")]
        );
    }

    // ── errors ──────────────────────────────────────────────────────

    #[test]
    fn test_unterminated_string() {
        let mut scanner = Scanner::new("+ 1:AA \"unterminated");
        // +, 1:AA
        assert!(scanner.next_token().unwrap().is_ok());
        assert!(scanner.next_token().unwrap().is_ok());
        // unterminated string should error
        let err = scanner.next_token().unwrap();
        assert!(err.is_err());
        let msg = err.unwrap_err().to_string();
        assert!(msg.contains("Unterminated"), "error: {msg}");
    }

    #[test]
    fn test_newline_in_quoted_string() {
        let mut scanner = Scanner::new("+ 1:AA \"hello\nworld\"");
        assert!(scanner.next_token().unwrap().is_ok());
        assert!(scanner.next_token().unwrap().is_ok());
        let err = scanner.next_token().unwrap();
        assert!(err.is_err());
        let msg = err.unwrap_err().to_string();
        assert!(msg.contains("Newline inside"), "error: {msg}");
    }

    // ── peek ────────────────────────────────────────────────────────

    #[test]
    fn test_peek_does_not_consume() {
        let mut scanner = Scanner::new("+ 42:AB \"x\"");
        // Peek at Plus
        assert!(matches!(
            scanner.peek_token(),
            Some(PeekResult::Token(Token::Plus))
        ));
        // Still available via next_token
        assert!(matches!(scanner.next_token(), Some(Ok(Token::Plus))));
        // And peeking again after consuming is the next token
        assert!(matches!(
            scanner.peek_token(),
            Some(PeekResult::Token(Token::Unquoted("42:AB")))
        ));
    }

    #[test]
    fn test_peek_multiple_times() {
        let mut scanner = Scanner::new("+ 1:AA");
        assert!(matches!(
            scanner.peek_token(),
            Some(PeekResult::Token(Token::Plus))
        ));
        assert!(matches!(
            scanner.peek_token(),
            Some(PeekResult::Token(Token::Plus))
        ));
        assert!(matches!(
            scanner.peek_token(),
            Some(PeekResult::Token(Token::Plus))
        ));
        assert!(matches!(scanner.next_token(), Some(Ok(Token::Plus))));
    }

    // ── position tracking ──────────────────────────────────────────

    #[test]
    fn test_position_tracking() {
        let mut scanner = Scanner::new("+\n42:AB\n\"x\"");
        // After '+' (line 1, col 1)
        assert!(matches!(scanner.next_token(), Some(Ok(Token::Plus))));
        assert_eq!(scanner.line, 1);
        // After '\n' 42:AB (line 2, col 5)
        assert!(matches!(
            scanner.next_token(),
            Some(Ok(Token::Unquoted("42:AB")))
        ));
        assert_eq!(scanner.line, 2);
        // After '\n' "x" (line 3)
        assert!(matches!(scanner.next_token(), Some(Ok(Token::Quoted("x")))));
        assert_eq!(scanner.line, 3);
    }

    #[test]
    fn test_crlf_position() {
        let mut scanner = Scanner::new("+\r\n42:AB");
        // `+` then `\r` (whitespace), then `\n` (whitespace that bumps line)
        assert!(matches!(scanner.next_token(), Some(Ok(Token::Plus))));
        assert!(matches!(
            scanner.next_token(),
            Some(Ok(Token::Unquoted("42:AB")))
        ));
        assert_eq!(scanner.line, 2);
    }

    // ── virtual line ────────────────────────────────────────────────

    #[test]
    fn test_virtual_line() {
        let t = collect("+ 0:00 \"first\"");
        assert_eq!(
            t,
            vec![Token::Plus, Token::Unquoted("0:00"), Token::Quoted("first")]
        );
    }

    // ── `#` in content is not a comment ────────────────────────────

    #[test]
    fn test_hash_in_unquoted_content() {
        let t = collect("+ 1:AA foo#bar");
        assert_eq!(
            t,
            vec![Token::Plus, Token::Unquoted("1:AA"), Token::Unquoted("foo#bar")]
        );
    }
}
