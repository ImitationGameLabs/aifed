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

    // Build insert operations (one per line)
    let mut ops_input = String::new();
    for line in clipboard_content.split('\n') {
        ops_input.push_str(&format!(
            "+ {}:{} \"{}\"\n",
            anchor_line,
            anchor_hash,
            escape_content(line)
        ));
    }

    let operations = batch::parse_batch_operations(&ops_input)?;
    batch::execute_batch(path, operations, false, format, Some(daemon_client)).await
}

/// Escape content for batch operation string (JSON-style)
fn escape_content(content: &str) -> String {
    content
        .replace('\\', "\\\\")
        .replace('\"', "\\\"")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
}
