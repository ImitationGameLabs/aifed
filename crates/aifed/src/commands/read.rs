use std::path::Path;

use crate::error::{Error, Result};
use crate::hash::hash_line;
use crate::locator::Locator;
use crate::output::{HashedLine, OutputFormat, format_lines};
use aifed_daemon_client::DaemonClient;

/// Execute the read command
pub async fn execute(
    path: &Path,
    locator_str: Option<&str>,
    no_hashes: bool,
    context: Option<usize>,
    format: OutputFormat,
    daemon_client: Option<&DaemonClient>,
) -> Result<()> {
    if !path.exists() {
        return Err(Error::FileNotFound {
            path: crate::file::to_absolute(path),
            cwd: std::env::current_dir().unwrap_or_default(),
        });
    }

    let content = crate::file::read_text_file(path)?;

    // Record access with daemon (for history tracking)
    // Use canonical path to ensure consistency
    if let Some(client) = daemon_client {
        let canonical = path
            .canonicalize()
            .map_err(|e| Error::InvalidIo { path: path.to_path_buf(), source: e })?;
        let file_str = canonical.to_string_lossy().to_string();
        let _ = client.record_access(&file_str).await;
    }

    let lines = crate::file::split_lines(&content);

    // Determine which lines to show
    let (start, end) = if let Some(loc_str) = locator_str {
        let locator = Locator::parse(loc_str)?;

        match locator {
            Locator::Hashline { line, .. } | Locator::Line(line) => {
                if line == 0 || line > lines.len() {
                    return Err(Error::InvalidLocator {
                        input: loc_str.to_string(),
                        reason: format!("Line {} out of range (1-{})", line, lines.len()),
                    });
                }

                if let Some(ctx) = context {
                    let ctx_start = line.saturating_sub(ctx).max(1);
                    let ctx_end = (line + ctx).min(lines.len());
                    (ctx_start, ctx_end)
                } else {
                    (line, line)
                }
            }
            Locator::LineRange { start, end } | Locator::HashlineRange { start, end, .. } => {
                if start > lines.len() {
                    return Err(Error::InvalidLocator {
                        input: loc_str.to_string(),
                        reason: format!("Range start {} out of range (1-{})", start, lines.len()),
                    });
                }
                let actual_end = end.min(lines.len());
                (start, actual_end)
            }
        }
    } else {
        // No locator: show all lines
        (1, lines.len())
    };

    // Build output lines
    let output_lines: Vec<HashedLine> = (start..=end)
        .map(|line_num| {
            let content = lines[line_num - 1].to_string();
            let hash = hash_line(&content);
            HashedLine { line: line_num, hash: Some(hash), content }
        })
        .collect();

    if output_lines.is_empty() {
        return Ok(());
    }

    println!("{}", format_lines(&output_lines, format, no_hashes));
    Ok(())
}
