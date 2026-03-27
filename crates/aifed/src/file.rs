//! File I/O utilities for aifed.

use std::path::Path;

use crate::error::{Error, Result};

/// Read a text file, rejecting binary files.
pub fn read_text_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)
        .map_err(|e| Error::InvalidIo { path: path.to_path_buf(), source: e })?;

    if content_inspector::inspect(&bytes).is_binary() {
        let abs_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        return Err(Error::BinaryFile { path: abs_path });
    }

    String::from_utf8(bytes)
        .map_err(|e| Error::InvalidEncoding { path: path.to_path_buf(), source: e })
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_text_file_plain_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        std::fs::write(&path, "hello\nworld\n").unwrap();

        let content = read_text_file(&path).unwrap();
        assert_eq!(content, "hello\nworld\n");
    }

    #[test]
    fn test_read_text_file_binary_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("binary.bin");
        // Write null bytes — content_inspector detects this as binary
        std::fs::write(&path, b"\x00\x01\x02\x03").unwrap();

        let result = read_text_file(&path);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("editing non-text files is not supported"));
    }

    #[test]
    fn test_read_text_file_error_contains_absolute_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("binary.bin");
        std::fs::write(&path, b"\x00\x00").unwrap();

        let result = read_text_file(&path);
        assert!(result.is_err());
        if let Error::BinaryFile { path: abs_path } = result.unwrap_err() {
            assert!(abs_path.is_absolute());
        } else {
            panic!("Expected BinaryFile error variant");
        }
    }

    #[test]
    fn test_read_text_file_empty_file_is_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.txt");
        std::fs::write(&path, "").unwrap();

        let content = read_text_file(&path).unwrap();
        assert_eq!(content, "");
    }

    #[test]
    fn test_read_text_file_nonexistent() {
        let path = Path::new("/tmp/this_file_absolutely_does_not_exist_aifed_test.txt");
        let result = read_text_file(path);
        assert!(result.is_err());
        // Should be IO error, not BinaryFile
        let err = result.unwrap_err().to_string();
        assert!(err.contains("IO error"));
    }

    #[test]
    fn test_read_text_file_invalid_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("invalid_utf8.txt");
        // Write bytes that are valid text according to content_inspector but invalid UTF-8
        // 0xC3 0x28 is an invalid UTF-8 sequence (lone high byte followed by '(')
        std::fs::write(&path, b"hello \xc3\x28 world").unwrap();

        let result = read_text_file(&path);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid UTF-8"));
    }
}
