//! Daemon management commands

use crate::args::DaemonCommands;
use crate::error::{Error, Result};
use crate::output::OutputFormat;
use aifed_common::socket_path;
use aifed_daemon_client::DaemonClient;
use std::path::Path;

pub async fn execute(
    cmd: &DaemonCommands,
    socket: Option<&Path>,
    format: OutputFormat,
) -> Result<()> {
    let socket = match socket {
        Some(p) => p.to_path_buf(),
        None => {
            let cwd = std::env::current_dir()
                .map_err(|e| Error::InvalidIo { path: Path::new(".").to_path_buf(), source: e })?;
            socket_path(&cwd)?
        }
    };

    let client = DaemonClient::new(&socket);

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
                        for server in status.servers {
                            println!("  - {}: {:?}", server.language, server.state);
                        }
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
                    println!("Daemon stopped");
                }
                Err(e) => {
                    return Err(Error::ClientError(e));
                }
            }
            Ok(())
        }
    }
}
