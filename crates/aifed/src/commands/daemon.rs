//! Daemon management commands

use crate::args::DaemonCommands;
use crate::error::{Error, Result};
use crate::output::OutputFormat;
use aifed_common::{log_path, workspace::Workspace};
use aifed_daemon_client::DaemonClient;
use time::format_description;

pub async fn execute(
    cmd: &DaemonCommands,
    workspace: &Workspace,
    format: OutputFormat,
) -> Result<()> {
    // Discover a running daemon (does not spawn one). `None` means no live,
    // authentic daemon for this workspace.
    let client = DaemonClient::discover(workspace.root()).await;

    match cmd {
        DaemonCommands::Status => {
            let Some(client) = client.as_ref() else {
                match format {
                    OutputFormat::Text => {
                        println!("Daemon not running");
                        println!();
                        println!("Workspace: {}", workspace.root().display());
                        if let Ok(log) = log_path(workspace.root()) {
                            println!("Log: {}", log.display());
                        }
                    }
                    OutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::json!({
                                "status": "not_running",
                                "workspace": workspace.root().display().to_string(),
                                "log_path": log_path(workspace.root()).ok().map(|p| p.display().to_string())
                            })
                        );
                    }
                }
                return Ok(());
            };

            match client.status().await {
                Ok(status) => match format {
                    OutputFormat::Text => {
                        println!("Workspace: {}", status.workspace);
                        println!("Uptime: {}s", status.uptime_secs);
                        println!("Servers:");
                        let local_offset =
                            time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
                        for server in &status.servers {
                            let local_time = server.state.at().to_offset(local_offset);
                            let formatted = local_time
                                .format(
                                    &format_description::parse(
                                        "[year]-[month]-[day] [hour]:[minute]:[second]",
                                    )
                                    .unwrap(),
                                )
                                .unwrap();
                            let status_line = match server.state.reason() {
                                Some(reason) => {
                                    format!(
                                        "{} ({}) - {}",
                                        server.state.status_str(),
                                        formatted,
                                        reason
                                    )
                                }
                                None => format!("{} ({})", server.state.status_str(), formatted),
                            };
                            println!("  - {}: {}", server.language, status_line);
                        }
                        println!();
                        println!("Daemon Env:");
                        println!("  - Bin: {}", status.bin_path);
                        println!("  - Address: {}", status.address);
                        println!("  - Log: {}", status.log_path);
                    }
                    OutputFormat::Json => {
                        println!("{}", serde_json::to_string_pretty(&status).unwrap());
                    }
                },
                Err(e) => {
                    return Err(Error::ClientError(e));
                }
            }
            Ok(())
        }
        DaemonCommands::Stop { force } => {
            let Some(client) = client.as_ref() else {
                println!("Daemon not running");
                return Ok(());
            };

            // Get list of servers and stop them
            match client.list_servers().await {
                Ok(servers) => {
                    for server in servers.servers {
                        if let Err(e) = client.stop_server(&server.language, *force).await {
                            eprintln!("Failed to stop {}: {}", server.language, e);
                        }
                    }
                }
                Err(e) => {
                    return Err(Error::ClientError(e));
                }
            }

            // Shutdown daemon
            if let Err(e) = client.shutdown().await {
                return Err(Error::ClientError(e));
            }
            println!("Daemon stopped");
            Ok(())
        }
    }
}
