//! Escape sequence normalization for batch content parsing.
//!
//! aifed uses `json_escape` for unescaping, which only supports standard JSON
//! escape sequences. This module bridges the gap for `\xNN` hex escapes (common
//! in Rust, Python, and shell code) by converting them to JSON-compatible
//! `\u00NN` form before `json_escape` processes the string.

use std::borrow::Cow;

/// Converts `\xNN` hex escapes to JSON-compatible `\u00NN` form.
///
/// Only exactly two hex digits after `\x` are converted. Anything that doesn't
/// match that pattern (e.g. `\x`, `\x1`, `\xGG`) is passed through unchanged,
/// so `json_escape` can still report an appropriate error for truly invalid
/// sequences.
///
/// Escape sequences that precede `\x` are respected: `\\x1b` (escaped backslash
/// followed by `x1b`) is left untouched, so json_escape later decodes it as the
/// four-character string `\x1b` rather than the ESC byte.
///
/// Returns `Cow::Borrowed` when no `\x` is present, avoiding allocation for
/// the common case.
pub fn normalize_hex_escapes(s: &str) -> Cow<'_, str> {
    if !s.contains("\\x") {
        return Cow::Borrowed(s);
    }

    let mut result = String::with_capacity(s.len() + 4);
    let mut remaining = s;

    while !remaining.is_empty() {
        match remaining.find('\\') {
            None => {
                result.push_str(remaining);
                break;
            }
            Some(pos) => {
                // Append everything before the backslash.
                result.push_str(&remaining[..pos]);
                remaining = &remaining[pos..];

                let mut chars = remaining.chars();
                chars.next(); // consume '\'

                match chars.next() {
                    Some('x') => {
                        let rest = chars.as_str();
                        let mut hex_chars = rest.chars();
                        let c1 = hex_chars.next();
                        let c2 = hex_chars.next();

                        if let (Some(c1), Some(c2)) = (c1, c2)
                            && c1.is_ascii_hexdigit()
                            && c2.is_ascii_hexdigit()
                        {
                            result.push_str("\\u00");
                            result.push(c1);
                            result.push(c2);
                            // c1 and c2 are hex digits, always single bytes
                            remaining = &rest[2..];
                            continue;
                        }
                        // Not valid \xNN — pass through \x and keep scanning.
                        result.push_str("\\x");
                        remaining = rest;
                    }
                    Some(c) => {
                        // Any other escape (\n, \\, \t, \uXXXX, …) — pass through;
                        // json_escape handles these downstream.
                        result.push('\\');
                        result.push(c);
                        remaining = chars.as_str();
                    }
                    None => {
                        // Trailing backslash.
                        result.push('\\');
                        remaining = "";
                    }
                }
            }
        }
    }

    Cow::Owned(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── positive: valid \xNN conversions ─────────────────────────────────────

    #[test]
    fn test_x1b_converts_to_u001b() {
        assert_eq!(normalize_hex_escapes(r"\x1b"), r"\u001b");
    }

    #[test]
    fn test_xff_converts() {
        assert_eq!(normalize_hex_escapes(r"\xff"), r"\u00ff");
    }

    #[test]
    fn test_uppercase_hex_converts() {
        assert_eq!(normalize_hex_escapes(r"\xAF"), r"\u00AF");
    }

    #[test]
    fn test_x00_converts() {
        assert_eq!(normalize_hex_escapes(r"\x00"), r"\u0000");
    }

    #[test]
    fn test_multiple_hex_escapes() {
        assert_eq!(normalize_hex_escapes(r"\x1b\x5b"), r"\u001b\u005b");
    }

    #[test]
    fn test_mixed_with_json_escapes() {
        // \n is a JSON escape — normalize_hex_escapes does not touch it;
        // json_escape handles it downstream.
        assert_eq!(normalize_hex_escapes(r"\x1b\n"), r"\u001b\n");
    }

    // ── zero-alloc path ───────────────────────────────────────────────────────

    #[test]
    fn test_no_hex_escape_returns_borrowed() {
        let s = r"hello\nworld";
        let result = normalize_hex_escapes(s);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, r"hello\nworld");
    }

    // ── passthrough: invalid \x patterns ────────────────────────────────────

    #[test]
    fn test_x_at_end_passthrough() {
        assert_eq!(normalize_hex_escapes(r"\x"), r"\x");
    }

    #[test]
    fn test_x_one_digit_passthrough() {
        assert_eq!(normalize_hex_escapes(r"\x1"), r"\x1");
    }

    #[test]
    fn test_x_non_hex_passthrough() {
        assert_eq!(normalize_hex_escapes(r"\xGG"), r"\xGG");
    }

    #[test]
    fn test_x_space_passthrough() {
        assert_eq!(normalize_hex_escapes(r"\x "), r"\x ");
    }

    // ── correctness: context-sensitive backslash handling ────────────────────

    #[test]
    fn test_escaped_backslash_before_x_passthrough() {
        // \\x1b = escaped backslash (\) + literal x1b
        // The \x at position 1 must NOT be treated as a hex escape.
        // json_escape later decodes \\ as \ and leaves x1b as-is: result = \x1b
        assert_eq!(normalize_hex_escapes(r"\\x1b"), r"\\x1b");
    }

    #[test]
    fn test_utf8_chars_not_corrupted() {
        // Non-ASCII characters before a hex escape must survive unchanged.
        let input = "caf\u{e9}\\x1b";
        let expected = "caf\u{e9}\\u001b";
        assert_eq!(normalize_hex_escapes(input), expected);
    }
}
