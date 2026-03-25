//! File I/O utilities for aifed.

use std::path::Path;

use crate::error::{Error, Result};

// =============================================================================
// CRLF / Line Ending Handling
// =============================================================================
//
// ## Background
//
// We use `split('\n')` instead of `lines()` to preserve `\r` characters at line
// endings. This ensures CRLF files (`\r\n`) are correctly round-tripped:
//
// ```
// Original:  "line1\r\nline2\n"
// split('\n'): ["line1\r", "line2", ""]
// join("\n"): "line1\r\nline2\n"  ✓ preserved
// ```
//
// ## Why not use `lines()`?
//
// `str::lines()` treats both `\n` and `\r\n` as line terminators, discarding
// the `\r`. After an edit cycle, CRLF files would become LF files.
//
// ## Trailing Empty String
//
// We preserve the trailing empty string from `split('\n')` to maintain trailing
// newline information:
//
// ```
// "a\nb"   → ["a", "b"]       → join → "a\nb"     (no trailing newline)
// "a\nb\n" → ["a", "b", ""]   → join → "a\nb\n"   (trailing newline)
// ```
//
// For CRLF files, the trailing empty string is still "" (not "\r"), because
// the `\r` belongs to the last content line:
//
// ```
// "a\r\nb\r\n" → ["a\r", "b\r", ""] → join → "a\r\nb\r\n" ✓
// ```
//
// ## Impact on Human Readers (Terminal Output)
//
// When outputting text format to a terminal, `\r` characters may cause display
// issues because `\r` (carriage return) moves the cursor to the beginning of
// the line, potentially overwriting previous characters:
//
// ```
// Actual content:  "hello\rworld"
// Terminal shows:  "world" (overwrites "hello")
// ```
//
// This primarily affects human users viewing CRLF files in terminals.
// For AI agents reading stdout raw bytes, `\r` is correctly captured.
//
// ## Recommendations
//
// - For AI agents: Text format works correctly; raw `\r` bytes are preserved.
// - For human users: Use JSON format (`--json`) to view CRLF files, which
//   properly escapes `\r` as the string `\r`.
//
// ## Future Improvements
//
// - Detect TTY output and escape control characters for terminal display
// - Add `--escape` flag for explicit control character escaping

/// Split content by `\n`, preserving `\r` at line endings and trailing empty string.
///
/// # Examples
///
/// ```
/// let lines = split_lines("a\r\nb\n");
/// assert_eq!(lines, vec!["a\r", "b", ""]);
///
/// let lines = split_lines("a\nb");
/// assert_eq!(lines, vec!["a", "b"]);
/// ```
pub fn split_lines(content: &str) -> Vec<&str> {
    content.split('\n').collect()
}

/// Owned version of `split_lines`, returns `Vec<String>`.
pub fn split_lines_owned(content: &str) -> Vec<String> {
    content.split('\n').map(|s| s.to_string()).collect()
}

/// Write lines to a file.
///
/// Lines are joined with `\n`. Trailing newline is preserved via the trailing
/// empty string convention (see module docs). CRLF line endings are preserved
/// because `join("\n")` produces `\r\n` from lines ending with `\r`.
pub fn write_file(path: &Path, lines: &[String]) -> Result<()> {
    let content = lines.join("\n");
    std::fs::write(path, content)
        .map_err(|e| Error::InvalidIo { path: path.to_path_buf(), source: e })?;
    Ok(())
}
