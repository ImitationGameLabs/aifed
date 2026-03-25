//! Daemon management commands

use crate::args::DaemonCommands;
use crate::error::{Error, Result};
use crate::output::OutputFormat;
use aifed_daemon_client::DaemonClient;
use time::format_description;

pub async fn execute(
    cmd: &DaemonCommands,
    client: &DaemonClient,
    format: OutputFormat,
) -> Result<()> {
    match cmd {
        DaemonCommands::Status => {
            if !client.is_running().await {
                println!("Daemon not running");
                return Ok(());
            }

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
                        println!("  - Socket: {}", status.socket_path);
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
            if !client.is_running().await {
                println!("Daemon not running");
                return Ok(());
            }

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
