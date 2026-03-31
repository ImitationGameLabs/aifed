//! Clipboard command - show clipboard content

use crate::error::Result;
use crate::output::OutputFormat;
use aifed_daemon_client::DaemonClient;

/// Execute the clipboard command
pub async fn execute(client: &DaemonClient, format: OutputFormat) -> Result<()> {
    let content = client.get_clipboard().await.map_err(crate::error::Error::ClientError)?;

    match content {
        Some(content) => match format {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "content": content,
                });
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            }
            OutputFormat::Text => {
                println!("{}", content);
            }
        },
        None => match format {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "content": null,
                });
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            }
            OutputFormat::Text => {
                println!("Clipboard is empty");
            }
        },
    }

    Ok(())
}
