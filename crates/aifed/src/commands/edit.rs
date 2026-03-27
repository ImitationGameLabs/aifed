use std::io::{self, IsTerminal, Read};
use std::path::Path;

use crate::batch;
use crate::error::{Error, Result};
use crate::output::OutputFormat;
use aifed_daemon_client::DaemonClient;

/// Execute the edit command (batch mode from stdin only)
pub async fn execute(
    path: &Path,
    dry_run: bool,
    format: OutputFormat,
    daemon_client: Option<&DaemonClient>,
) -> Result<()> {
    if !path.exists() {
        return Err(Error::FileNotFound {
            path: crate::file::to_absolute(path),
            cwd: std::env::current_dir().unwrap_or_default(),
        });
    }

    // Read operations from stdin
    if io::stdin().is_terminal() {
        return Err(Error::StdinNotAvailable);
    }
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).map_err(|_| Error::StdinNotAvailable)?;
    let operations = batch::parse_batch_operations(&input)?;
    batch::execute_batch(path, operations, dry_run, format, daemon_client).await
}
