mod args;
mod batch;
mod commands;
mod error;
mod file;
mod hash;
mod locator;
mod output;

use crate::args::{Args, Commands};
use crate::error::{Error, Result};
use crate::output::{OutputFormat, format_error};
use aifed_common::workspace::{Workspace, detect_workspace};
use aifed_daemon_client::DaemonClient;
use std::path::Path;

#[tokio::main]
async fn main() {
    let args = Args::parse_args();
    let format = if args.json { OutputFormat::Json } else { OutputFormat::Text };

    let result = run(args, format).await;

    if let Err(e) = result {
        eprintln!("{}", format_error(&e, format));
        std::process::exit(1);
    }
}

async fn run(args: Args, format: OutputFormat) -> Result<()> {
    match args.command {
        // Commands that don't need workspace
        Commands::Read { file, locator, no_hashes, context } => {
            commands::read(&file, locator.as_deref(), no_hashes, context, format)
        }
        Commands::Info { file } => commands::info(&file, format),
        Commands::Edit { file, operation, locator, content, dry_run } => commands::edit(
            &file,
            operation.as_deref(),
            locator.as_deref(),
            content.as_deref(),
            dry_run,
            format,
        ),

        // Commands that require workspace
        Commands::Daemon(cmd) => {
            let ws = require_workspace()?;
            let client = DaemonClient::new(&ws.socket_path()?);
            commands::daemon(&cmd, &client, format).await
        }
        Commands::Lsp(cmd) => {
            let ws = require_workspace()?;
            let client = DaemonClient::new(&ws.socket_path()?);
            commands::lsp(&cmd, &client, format).await
        }
    }
}

/// Require a workspace to be detected from current directory.
fn require_workspace() -> Result<Workspace> {
    let cwd = std::env::current_dir()
        .map_err(|e| Error::InvalidIo { path: Path::new(".").to_path_buf(), source: e })?;
    detect_workspace(&cwd).ok_or(Error::LightweightMode)
}
