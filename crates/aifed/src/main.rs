mod args;
mod batch;
mod commands;
mod diff;
mod error;
mod file;
mod hash;
mod locator;
mod output;
mod text_edit;

use crate::args::{Args, Commands, DaemonCommands};
use crate::error::{Error, Result};
use crate::output::{OutputFormat, format_error};
use aifed_common::workspace::{Workspace, detect_workspace};
use aifed_daemon_client::DaemonClient;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Daemon requirement level for different commands
enum DaemonRequirement {
    /// Must have daemon, error if unavailable
    Required,
    /// Best effort - warning + degraded mode if unavailable
    Optional,
    /// No daemon needed
    None,
}

fn daemon_requirement(cmd: &Commands) -> DaemonRequirement {
    match cmd {
        Commands::Info { .. } => DaemonRequirement::None,
        Commands::Daemon(daemon_cmd) => match daemon_cmd {
            DaemonCommands::Status | DaemonCommands::Stop { .. } => DaemonRequirement::None,
        },
        Commands::Read { .. } | Commands::Edit { .. } => DaemonRequirement::Optional,
        Commands::Lsp { .. }
        | Commands::History { .. }
        | Commands::Undo { .. }
        | Commands::Redo { .. } => DaemonRequirement::Required,
    }
}

#[tokio::main]
async fn main() {
    // Handle --skill before clap parsing (to avoid requiring subcommand)
    if std::env::args().any(|arg| arg == "--skill") {
        const AGENT_SKILL: &str = include_str!("skill.md");
        println!("{}", AGENT_SKILL);
        return;
    }

    let args = Args::parse_args();
    let format = if args.json { OutputFormat::Json } else { OutputFormat::Text };

    let result = run(args, format).await;

    if let Err(e) = result {
        eprintln!("{}", format_error(&e, format));
        std::process::exit(1);
    }
}

async fn run(args: Args, format: OutputFormat) -> Result<()> {
    // Try to detect workspace and ensure daemon is running based on command requirements
    let cwd = std::env::current_dir()
        .map_err(|e| Error::InvalidIo { path: PathBuf::from("."), source: e })?;
    let workspace = detect_workspace(&cwd);

    let daemon_client = match &workspace {
        Some(ws) => match daemon_requirement(&args.command) {
            DaemonRequirement::None => None,
            DaemonRequirement::Optional => ensure_daemon_optional(ws).await,
            DaemonRequirement::Required => ensure_daemon(ws).await,
        },
        None => None,
    };

    match args.command {
        // Commands that don't need workspace - daemon_client is optional
        Commands::Read { file, locator, no_hashes, context } => {
            commands::read(
                &file,
                locator.as_deref(),
                no_hashes,
                context,
                format,
                daemon_client.as_ref(),
            )
            .await
        }
        Commands::Info { file } => commands::info(&file, format),
        Commands::Edit { file, operation, locator, content, dry_run } => {
            commands::edit(
                &file,
                operation.as_deref(),
                locator.as_deref(),
                content.as_deref(),
                dry_run,
                format,
                daemon_client.as_ref(),
            )
            .await
        }

        // Commands that require workspace
        Commands::Daemon(cmd) => {
            let ws = workspace.ok_or(Error::LightweightMode)?;
            commands::daemon(&cmd, &ws, format).await
        }
        Commands::Lsp(cmd) => {
            let ws = workspace.ok_or(Error::LightweightMode)?;
            let client = daemon_client
                .ok_or_else(|| Error::DaemonNotRunning { workspace: ws.root().to_path_buf() })?;
            commands::lsp(&cmd, &client, format).await
        }
        Commands::History { file, count, stat } => {
            let ws = workspace.ok_or(Error::LightweightMode)?;
            let client = daemon_client
                .ok_or_else(|| Error::DaemonNotRunning { workspace: ws.root().to_path_buf() })?;
            commands::history(&file, count, stat, &client, format).await
        }
        Commands::Undo { file, dry_run } => {
            let ws = workspace.ok_or(Error::LightweightMode)?;
            let client = daemon_client
                .ok_or_else(|| Error::DaemonNotRunning { workspace: ws.root().to_path_buf() })?;
            commands::undo(&file, dry_run, &client, format).await
        }
        Commands::Redo { file, dry_run } => {
            let ws = workspace.ok_or(Error::LightweightMode)?;
            let client = daemon_client
                .ok_or_else(|| Error::DaemonNotRunning { workspace: ws.root().to_path_buf() })?;
            commands::redo(&file, dry_run, &client, format).await
        }
    }
}

/// Ensure daemon is running for workspace, starting it if necessary.
/// Returns a DaemonClient on success, or None on failure (for Required commands,
/// the caller should error out if this returns None).
async fn ensure_daemon(workspace: &Workspace) -> Option<DaemonClient> {
    let socket_path = match workspace.socket_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Warning: could not determine socket path: {}", e);
            return None;
        }
    };

    let client = DaemonClient::new(&socket_path);

    // Already running? Send heartbeat to keep it alive
    if client.is_running().await {
        let _ = client.heartbeat().await;
        return Some(client);
    }

    // Need to start daemon
    match spawn_daemon(workspace, &socket_path) {
        Ok(()) => {
            // Wait for daemon to be ready
            match wait_for_daemon(&client, Duration::from_secs(5)).await {
                Ok(()) => {
                    let _ = client.heartbeat().await;
                    Some(client)
                }
                Err(e) => {
                    eprintln!("Warning: daemon did not start in time: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            eprintln!("Warning: could not start daemon: {}", e);
            None
        }
    }
}

/// Try to start daemon, print warning and return None on failure.
/// For Optional commands that can work without daemon.
async fn ensure_daemon_optional(workspace: &Workspace) -> Option<DaemonClient> {
    let socket_path = match workspace.socket_path() {
        Ok(p) => p,
        Err(_) => {
            print_daemon_unavailable_warning();
            return None;
        }
    };

    let client = DaemonClient::new(&socket_path);

    // Already running? Send heartbeat to keep it alive
    if client.is_running().await {
        let _ = client.heartbeat().await;
        return Some(client);
    }

    // Try to start daemon
    match spawn_daemon(workspace, &socket_path) {
        Ok(()) => match wait_for_daemon(&client, Duration::from_secs(5)).await {
            Ok(()) => {
                let _ = client.heartbeat().await;
                Some(client)
            }
            Err(_) => {
                print_daemon_unavailable_warning();
                None
            }
        },
        Err(_) => {
            print_daemon_unavailable_warning();
            None
        }
    }
}

fn print_daemon_unavailable_warning() {
    eprintln!("Warning: daemon unavailable. The following features are disabled:");
    eprintln!("  - Edit history tracking (undo/redo)");
    eprintln!("  - Concurrent modification detection");
    eprintln!("File operations will proceed without these protections.");
}

/// Spawn daemon process for the given workspace.
fn spawn_daemon(workspace: &Workspace, socket_path: &Path) -> std::io::Result<()> {
    std::process::Command::new("aifed-daemon")
        .arg("--workspace")
        .arg(workspace.root())
        .arg("--socket")
        .arg(socket_path)
        .arg("--idle-timeout-secs")
        .arg("1800")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}

/// Wait for daemon to become ready by polling is_running().
async fn wait_for_daemon(client: &DaemonClient, timeout: Duration) -> std::io::Result<()> {
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if client.is_running().await {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "daemon did not start within timeout"))
}
