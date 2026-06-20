//! Paste command - paste clipboard content into a file

use std::path::Path;

use crate::batch;
use crate::error::{Error, Result};
use crate::hash::hash_line;
use crate::locator::Locator;
use crate::output::OutputFormat;
use aifed_daemon_client::DaemonClient;

/// Execute the paste command
pub async fn execute(
    path: &Path,
    position_str: &str,
    daemon_client: &DaemonClient,
    format: OutputFormat,
) -> Result<()> {
    if !path.exists() {
        return Err(Error::FileNotFound {
            path: crate::file::to_absolute(path),
            cwd: std::env::current_dir().unwrap_or_default(),
        });
    }

    // Get clipboard content
    let clipboard_content = daemon_client
        .get_clipboard()
        .await
        .map_err(Error::ClientError)?
        .ok_or_else(|| {
            Error::ClientError(aifed_common::ClientError::ApiError {
                code: "CLIPBOARD_EMPTY".to_string(),
                message: "Clipboard is empty".to_string(),
            })
        })?;

    // Parse position (e.g., "10:AB")
    let locator = Locator::parse(position_str)?;
    let (anchor_line, anchor_hash) = match locator {
        Locator::Hashline { line, hash } => (line, hash),
        _ => {
            return Err(Error::InvalidLocator {
                input: position_str.to_string(),
                reason: "Paste requires a hashline (e.g., \"10:AB\")".to_string(),
            });
        }
    };

    // Verify hash for non-virtual lines
    if anchor_line > 0 {
        let file_content = crate::file::read_text_file(path)?;
        let lines = crate::file::split_lines(&file_content);
        if anchor_line > lines.len() {
            return Err(Error::InvalidLocator {
                input: position_str.to_string(),
                reason: format!("Line {} out of range (1-{})", anchor_line, lines.len()),
            });
        }
        let actual_hash = hash_line(lines[anchor_line - 1]);
        if actual_hash != anchor_hash && !crate::hash::is_virtual_hash(&anchor_hash) {
            return Err(Error::HashMismatch {
                path: path.to_path_buf(),
                line: anchor_line,
                expected: anchor_hash,
                actual: actual_hash,
                actual_content: lines[anchor_line - 1].to_string(),
            });
        }
    }

    // Build one insert operation carrying all clipboard lines in order.
    let mut ops_input = format!("+ {}:{}", anchor_line, anchor_hash);
    for line in clipboard_content.split('\n') {
        ops_input.push_str(&format!(" \"{}\"", escape_content(line)));
    }
    ops_input.push('\n');

    let operations = batch::parse_batch_operations(&ops_input)?;
    batch::execute_batch(
        path,
        operations,
        false,
        format,
        Some(daemon_client),
        &crate::indent::IndentSettings::detecting(),
    )
    .await
}

/// Escape content for a batch operation string so clipboard lines survive
/// `parse_batch_operations` byte-for-byte: `\`/`"`, named `\t`/`\r`, and every
/// remaining control byte as `\xNN` (the batch-source counterpart of
/// `escape::escape_for_display`).
/// Expects a single line (no `\n`); callers split multi-line clipboard content first.
fn escape_content(content: &str) -> String {
    let mut out = String::with_capacity(content.len() + 8);
    for c in content.chars() {
        match c {
            '\\' | '"' => {
                out.push('\\');
                out.push(c);
            }
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            // Other C0 controls and DEL -> \xNN (tab/CR/quote/backslash handled above).
            c if c.is_ascii() && crate::escape::control_needs_hex(c as u8) => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_content_escapes_control_bytes() {
        // Raw control bytes become \xNN so they survive parse_batch_operations,
        // closing the copy -> paste round-trip hole.
        assert_eq!(escape_content("a\x1bb"), r"a\x1bb");
        assert_eq!(escape_content("\u{0}"), r"\x00");
        assert_eq!(escape_content("\u{c}"), r"\x0c");
        assert_eq!(escape_content("\u{7f}"), r"\x7f");
    }

    #[test]
    fn escape_content_keeps_named_escapes() {
        assert_eq!(escape_content("a\tb"), r"a\tb");
        assert_eq!(escape_content("a\rb"), r"a\rb");
        assert_eq!(escape_content(r"a\b"), r"a\\b");
        assert_eq!(escape_content("a\"b"), r#"a\"b"#);
    }
}
