//! Undo command - undo the last edit for a file

use crate::diff::{apply_diffs, print_diffs};
use crate::error::{Error, Result};
use crate::hash::hash_file;
use crate::output::OutputFormat;
use aifed_daemon_client::DaemonClient;
use std::path::Path;

/// Execute the undo command
pub async fn execute(
    file: &Path,
    dry_run: bool,
    client: &DaemonClient,
    format: OutputFormat,
) -> Result<()> {
    let canonical = file
        .canonicalize()
        .map_err(|e| Error::InvalidIo { path: file.to_path_buf(), source: e })?;
    let file_str = canonical.to_string_lossy().to_string();

    let response = client.undo(&file_str, dry_run).await.map_err(Error::ClientError)?;

    if !dry_run && !response.diffs.is_empty() {
        // Read current file (as raw bytes for hash verification)
        let file_bytes = std::fs::read(file)
            .map_err(|e| Error::InvalidIo { path: file.to_path_buf(), source: e })?;

        // Verify hash if daemon provided one
        if !response.current_hash.is_empty() {
            let actual_hash = hash_file(&file_bytes);
            if actual_hash != response.current_hash {
                return Err(Error::FileHashMismatch {
                    path: file.to_path_buf(),
                    expected: response.current_hash,
                    actual: actual_hash,
                });
            }
        }

        // Convert to string for processing
        let file_content = String::from_utf8(file_bytes.clone())
            .map_err(|e| Error::InvalidEncoding { path: file.to_path_buf(), source: e })?;

        // Apply the diffs
        let original_had_trailing_newline = file_content.ends_with('\n');
        let mut lines: Vec<String> = file_content.lines().map(|s| s.to_string()).collect();
        apply_diffs(&mut lines, &response.diffs);

        // Write file back
        crate::file::write_file(file, &lines, original_had_trailing_newline)?;

        // Update daemon with new file hash via record_access
        let _ = client.record_access(&file_str).await;
    }

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
        OutputFormat::Text => {
            if dry_run {
                println!("Preview of undo for {}:", file.display());
            } else {
                println!("Undone changes for {}:", file.display());
            }
            print_diffs(&response.diffs);
        }
    }

    Ok(())
}
