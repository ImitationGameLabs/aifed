//! Copy command - copy lines from a file to clipboard

use std::path::Path;

use crate::error::{Error, Result};
use crate::hash::hash_line;
use crate::locator::Locator;
use crate::output::OutputFormat;
use aifed_daemon_client::DaemonClient;

/// Execute the copy command
pub async fn execute(
    path: &Path,
    range_str: &str,
    daemon_client: &DaemonClient,
    format: OutputFormat,
) -> Result<()> {
    if !path.exists() {
        return Err(Error::FileNotFound {
            path: crate::file::to_absolute(path),
            cwd: std::env::current_dir().unwrap_or_default(),
        });
    }

    // Parse range (e.g., "[1:AB,5:CD]")
    let locator = Locator::parse(range_str)?;

    let (start, start_hash, end, end_hash) = match locator {
        Locator::HashlineRange { start, start_hash, end, end_hash } => {
            (start, start_hash, end, end_hash)
        }
        Locator::Hashline { line, hash } => (line, hash.clone(), line, hash),
        _ => {
            return Err(Error::InvalidLocator {
                input: range_str.to_string(),
                reason: "Copy requires a hashline range (e.g., \"[1:AB,5:CD]\") or hashline (e.g., \"3:AB\")".to_string(),
            });
        }
    };

    let file_content = crate::file::read_text_file(path)?;
    let lines = crate::file::split_lines(&file_content);

    // Validate bounds
    if start == 0 || start > lines.len() {
        return Err(Error::InvalidLocator {
            input: range_str.to_string(),
            reason: format!(
                "Range start {} out of bounds (file has {} lines)",
                start,
                lines.len()
            ),
        });
    }
    if end == 0 || end > lines.len() {
        return Err(Error::InvalidLocator {
            input: range_str.to_string(),
            reason: format!(
                "Range end {} out of bounds (file has {} lines)",
                end,
                lines.len()
            ),
        });
    }

    // Verify boundary hashes
    let actual_start_hash = hash_line(lines[start - 1]);
    if actual_start_hash != start_hash {
        return Err(Error::HashMismatch {
            path: path.to_path_buf(),
            line: start,
            expected: start_hash,
            actual: actual_start_hash,
            actual_content: lines[start - 1].to_string(),
        });
    }
    let actual_end_hash = hash_line(lines[end - 1]);
    if actual_end_hash != end_hash {
        return Err(Error::HashMismatch {
            path: path.to_path_buf(),
            line: end,
            expected: end_hash,
            actual: actual_end_hash,
            actual_content: lines[end - 1].to_string(),
        });
    }

    // Extract lines and join with newlines
    let copied: String = lines[start - 1..end].join("\n");

    // Store in daemon clipboard
    daemon_client
        .set_clipboard(Some(copied.clone()))
        .await
        .map_err(Error::ClientError)?;

    let line_count = end - start + 1;
    match format {
        OutputFormat::Json => {
            let output = serde_json::json!({
                "action": "copy",
                "file": path.to_string_lossy(),
                "range": format!("{}-{}", start, end),
                "lines": line_count,
                "content": &copied,
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        OutputFormat::Text => {
            println!(
                "Copied {} line(s) ({}-{}) from {}",
                line_count,
                start,
                end,
                path.display()
            );
            for (i, line) in copied.split('\n').enumerate() {
                println!("{}|{}", start + i, line);
            }
        }
    }

    Ok(())
}
