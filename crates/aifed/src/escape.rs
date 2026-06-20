//! Escape handling for the two directions of aifed's content round-trip.
//!
//! - **Write path** ([`normalize_hex_escapes`]): the edit decoder uses
//!   `json_escape`, which only understands standard JSON escapes, so this
//!   converts `\xNN` hex escapes (common in Rust, Python, and shell code) to the
//!   JSON-compatible `\u00NN` form before `json_escape` runs. Agents can then
//!   write raw bytes like ANSI ESC as `\x1b`.
//! - **Read path** ([`escape_for_display`]): the display counterpart. Control
//!   bytes stored raw in a file are rendered back as `\xNN`/`\r` so `aifed read`
//!   and friends show a faithful, copyable line an agent can paste back verbatim.
//!
//! The line hash is always over the RAW content; read-path escaping is purely a
//! display transform applied after hashing.
//!
//! ## Display sites that must stay in sync
//!
//! `escape_for_display` must be applied at every site that prints raw file
//! content as text — missing one leaves that view with invisible control bytes
//! and an inconsistent display contract. Current sites:
//!
//! - `output::format_lines` (read, incl. `--no-hashes`)
//! - `commands::copy` text branch
//! - `commands::clipboard` text branch
//! - `commands::history::print_diff_hunk`
//! - `diff::print_diffs` (undo/redo)
//! - `edit_view::render::render_row` (edit/rename diffs)
//!
//! Deliberately not escaped: `error::HashMismatch` emits the raw
//! `actual_content` in its message — error-message escaping is a separate
//! concern. The batch-source counterpart is `commands::paste::escape_content`,
//! which escapes `\`/`"`/`\t`/`\r`/`\xNN` so clipboard content survives
//! `parse_batch_operations`.

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

/// Encode non-printable control bytes for display, so `aifed read`/`copy`/
/// `clipboard`/diff output is visible and round-trips through the edit decoder.
///
/// The read-path counterpart of [`normalize_hex_escapes`]: the write path accepts
/// `\xNN`/`\r` and stores the raw byte; the read path renders that byte back as
/// `\xNN`/`\r` so an agent can copy a line verbatim into the next edit.
///
/// Byte policy:
/// - Tab (`0x09`) is left raw — visible, copyable, accepted verbatim by the
///   decoder; escaping it would harm readability of tab-indented files.
/// - LF (`0x0a`) is left raw — per-line views never contain it, but clipboard
///   content is multi-line and its separators must stay real newlines.
/// - Carriage return (`0x0d`) becomes `\r` — a literal CR breaks terminal display
///   and is invalid in a JSON string, but `\r` round-trips.
/// - All other C0 controls (`0x00`–`0x08`, `0x0b`, `0x0c`, `0x0e`–`0x1f`) and
///   DEL (`0x7f`) become `\xNN`.
/// - C1 controls (`U+0080`-`U+009F`) are also left raw; they are rare in source files.
/// - Printable bytes and valid UTF-8 are unchanged.
///
/// The line hash is always over the RAW content (`hash_line`); this escaping is a
/// display-layer transform applied AFTER hashing, so a displayed
/// `LINE:HASH|\xNN...` keeps a copyable, hash-correct anchor.
///
/// Returns `Cow::Borrowed` when no escapable byte is present, avoiding
/// allocation for the common case.
pub fn escape_for_display(s: &str) -> Cow<'_, str> {
    // Fast path: no escapable control byte. Tab (0x09) and LF (0x0a) are left
    // raw — LF can occur in multi-line clipboard content.
    if !s.as_bytes().iter().any(|&b| needs_escape(b)) {
        return Cow::Borrowed(s);
    }

    let mut out = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        match c {
            '\t' | '\n' => out.push(c),
            '\r' => out.push_str("\\r"),
            // Other C0 controls and DEL render as \xNN via the shared
            // control_needs_hex set; is_ascii() keeps `c as u8` sound (multibyte >= 0x80).
            c if c.is_ascii() && control_needs_hex(c as u8) => {
                out.push('\\');
                out.push('x');
                let n = c as u32;
                out.push(hex_digit(n >> 4));
                out.push(hex_digit(n & 0xf));
            }
            _ => out.push(c),
        }
    }
    Cow::Owned(out)
}

/// The bytes rendered as `\xNN` by [`escape_for_display`] and `paste::escape_content`
/// — C0 controls and DEL, excluding tab/LF (raw) and CR (rendered as `\r` via a
/// dedicated arm). Single source of truth for the hex byte set across both sites.
pub(crate) fn control_needs_hex(b: u8) -> bool {
    matches!(b, 0x00..=0x08 | 0x0b | 0x0c | 0x0e..=0x1f | 0x7f)
}

/// Whether [`escape_for_display`] must transform `b`: CR (-> `\r`) or any
/// [`control_needs_hex`] byte. Tab and LF are left raw, so they are excluded.
fn needs_escape(b: u8) -> bool {
    b == b'\r' || control_needs_hex(b)
}

/// Lowercase hex digit for a nibble in `0..=15`.
fn hex_digit(nibble: u32) -> char {
    match nibble {
        0..=9 => (b'0' + nibble as u8) as char,
        _ => (b'a' + (nibble - 10) as u8) as char,
    }
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

    // ── escape_for_display: read-path control-byte encoding ───────────────────

    #[test]
    fn escape_for_display_fast_path_borrowed() {
        // No control bytes → Borrowed, unchanged. Tab is left raw.
        for s in ["", "plain ascii", "\t\ttab-indented", "fn main() {}", "café résumé"] {
            match escape_for_display(s) {
                Cow::Borrowed(got) => assert_eq!(got, s),
                Cow::Owned(got) => panic!("expected Borrowed for {s:?}, got Owned {got:?}"),
            }
        }
    }

    #[test]
    fn escape_for_display_escapes_control_bytes() {
        assert_eq!(escape_for_display("\r"), r"\r");
        assert_eq!(escape_for_display("\0"), r"\x00");
        assert_eq!(escape_for_display("\x1b"), r"\x1b");
        assert_eq!(escape_for_display("\u{c}"), r"\x0c"); // form feed
        assert_eq!(escape_for_display("\u{7f}"), r"\x7f"); // DEL
        // Printable neighbour preserved; boundary at 0x7e/0x7f.
        assert_eq!(escape_for_display("~\u{7f}"), r"~\x7f");
        // A line composed solely of control bytes.
        assert_eq!(escape_for_display("\0\x1b\u{7f}"), r"\x00\x1b\x7f");
    }

    #[test]
    fn escape_for_display_preserves_tab_and_utf8() {
        // Tab is left raw (literal), not \t.
        assert_eq!(escape_for_display("a\tb"), "a\tb");
        // Multibyte UTF-8 survives; only the ESC byte is escaped.
        assert_eq!(escape_for_display("café\x1b"), r"café\x1b");
        // Mixed: literal tab + escaped CR + escaped ESC.
        assert_eq!(escape_for_display("\t\r\x1b"), "\t\\r\\x1b");
        // LF is left raw (multi-line clipboard content keeps real newlines).
        assert_eq!(escape_for_display("a\nb\x1b"), "a\nb\\x1b");
    }
}
