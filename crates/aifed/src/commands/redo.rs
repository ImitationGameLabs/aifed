//! Redo command - redo the last undone edit for a file

use crate::diff::{apply_diffs, print_diffs};
use crate::error::{Error, Result};
use crate::hash::hash_file;
use crate::output::OutputFormat;
use aifed_daemon_client::DaemonClient;
use std::path::Path;

/// Execute the redo command
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

    let response = client
        .redo(&file_str, dry_run)
        .await
        .map_err(Error::ClientError)?;

    if !dry_run && !response.diffs.is_empty() {
        // Read current file
        let file_content = crate::file::read_text_file(file)?;

        // Verify hash if daemon provided one
        if !response.current_hash.is_empty() {
            let actual_hash = hash_file(file_content.as_bytes());
            if actual_hash != response.current_hash {
                return Err(Error::FileHashMismatch {
                    path: file.to_path_buf(),
                    expected: response.current_hash,
                    actual: actual_hash,
                });
            }
        }

        // Apply the diffs
        let mut lines = crate::file::split_lines_owned(&file_content);
        apply_diffs(&mut lines, &response.diffs);

        // Write file back
        crate::file::write_file(file, &lines)?;

        // Update daemon with new file hash via record_access
        let _ = client.record_access(&file_str).await;
    }

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
        OutputFormat::Text => {
            if dry_run {
                println!("Preview of redo for {}:", file.display());
            } else {
                println!("Redone changes for {}:", file.display());
            }
            print_diffs(&response.diffs);
        }
    }

    Ok(())
}
