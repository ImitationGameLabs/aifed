//! History command - view edit history for a file

use crate::edit_view::{EditRow, changed_rows_from_diffs};
use crate::error::{Error, Result};
use crate::output::OutputFormat;
use aifed_common::{HistoryEntryDto, LineDiffDto};
use aifed_daemon_client::DaemonClient;
use std::path::Path;

/// Default number of history entries to display
const DEFAULT_COUNT: usize = 5;

/// Execute the history command
pub async fn execute(
    file: &Path,
    count: Option<usize>,
    stat: bool,
    client: &DaemonClient,
    format: OutputFormat,
) -> Result<()> {
    // Use canonical path to ensure consistency with daemon
    let canonical = file
        .canonicalize()
        .map_err(|e| Error::InvalidIo { path: file.to_path_buf(), source: e })?;
    let file_str = canonical.to_string_lossy().to_string();
    let response = client
        .get_history(&file_str, count.or(Some(DEFAULT_COUNT)))
        .await
        .map_err(Error::ClientError)?;

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
        OutputFormat::Text => {
            if response.entries.is_empty() {
                println!("No history for {}", file.display());
                return Ok(());
            }

            if stat {
                print_stat(&response.entries);
            } else {
                print_verbose(&response.entries);
            }
        }
    }

    Ok(())
}

/// Print verbose history output in unified diff format
fn print_verbose(entries: &[HistoryEntryDto]) {
    for entry in entries {
        println!("#{} @ {}", entry.id, format_timestamp(&entry.timestamp));
        println!("{}", entry.summary);
        println!();

        // Group consecutive line changes and print in diff format
        for diff in &entry.diffs {
            print_diff_hunk(diff);
        }
        println!();
    }
}

/// Print a single diff hunk
/// Intentionally plain `+content`/`-content` (no hashline), diverging from
/// `edit_view::render_rows`.
fn print_diff_hunk(diff: &LineDiffDto) {
    let rows = changed_rows_from_diffs(std::slice::from_ref(diff));
    if rows.is_empty() {
        return;
    }
    println!("@@ {} @@", diff.line_num);
    for row in &rows {
        match row {
            EditRow::Insert { new_content, .. } => println!("+{}", new_content),
            EditRow::Delete { old_content, .. } => println!("-{}", old_content),
            EditRow::Equal { .. } => {}
        }
    }
}

/// Print compact stat output
fn print_stat(entries: &[HistoryEntryDto]) {
    for entry in entries {
        println!(
            "#{} {} {}",
            entry.id,
            format_timestamp(&entry.timestamp),
            entry.summary
        );
    }
}

/// Format timestamp for display
fn format_timestamp(ts: &time::OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        ts.year(),
        ts.month() as u8,
        ts.day(),
        ts.hour(),
        ts.minute(),
        ts.second()
    )
}
