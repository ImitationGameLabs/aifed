use std::io::{self, IsTerminal, Read};
use std::path::Path;

use crate::batch;
use crate::error::{Error, Result};
use crate::indent::IndentSettings;
use crate::output::OutputFormat;
use aifed_common::load_registry_for_path;
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
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|_| Error::StdinNotAvailable)?;
    let operations = batch::parse_batch_operations(&input)?;
    let settings = match load_registry_for_path(path) {
        Ok(reg) => IndentSettings::from_registry(&reg, path),
        Err(e) => {
            eprintln!(
                "Warning: config load failed, indent directives will use file-detection: {e}"
            );
            IndentSettings::detecting()
        }
    };
    batch::execute_batch(path, operations, dry_run, format, daemon_client, &settings).await
}
