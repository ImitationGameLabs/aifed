//! CLI argument parsing

use clap::Parser;
use std::path::PathBuf;

/// aifed-daemon - Background daemon for aifed (one workspace per instance)
#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Args {
    /// Path to the workspace directory
    #[arg(long)]
    pub workspace: PathBuf,

    /// Custom socket path (default: ~/.cache/aifed/&lt;name&gt;-&lt;hash16&gt;.sock)
    #[arg(long)]
    pub socket: Option<PathBuf>,

    /// Idle timeout in seconds before automatic shutdown (default: 1800 = 30 minutes)
    #[arg(long, default_value = "1800")]
    pub idle_timeout_secs: u64,

    /// Log level: trace, debug, info, warn, error
    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// Custom log file path (default: ~/.local/state/aifed/logs/&lt;name&gt;-&lt;hash16&gt;.log)
    /// Ignored if --log-stderr is set.
    #[arg(long)]
    pub log_file: Option<PathBuf>,

    /// Only output logs to stderr, do not write to log file.
    /// When set, --log-file is ignored.
    #[arg(long)]
    pub log_stderr: bool,
}
