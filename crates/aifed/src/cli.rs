use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// AI-First Editor - A text editor designed for AI agents
#[derive(Parser, Debug)]
#[command(name = "aifed", version, about, long_about = None, disable_help_flag = true, disable_version_flag = true)]
pub struct Cli {
    /// Output in JSON format
    #[arg(long, global = true)]
    pub json: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Print help (see a summary with '--help')
    #[arg(long, global = true, action = clap::ArgAction::Help)]
    help: Option<bool>,

    /// Print version
    #[arg(long, action = clap::ArgAction::Version)]
    version: Option<bool>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(verbatim_doc_comment)]
    /// Read file content with hashlines
    ///
    /// Output format: LINE:HASH|CONTENT
    ///   LINE    - 1-based line number
    ///   HASH    - 2-char content hash (base32hex)
    ///   CONTENT - line text
    ///   Separated by : (LINE:HASH) and | (HASH|CONTENT)
    Read {
        /// File to read
        file: PathBuf,

        /// Line number or range (e.g., "42" or "10-20")
        #[arg(value_name = "LOCATOR")]
        locator: Option<String>,

        /// Read without hash prefixes
        #[arg(long)]
        no_hashes: bool,

        /// Number of context lines to show around the target
        #[arg(long, value_name = "N")]
        context: Option<usize>,
    },

    /// Get file metadata
    Info {
        /// File to inspect
        file: PathBuf,
    },

    #[command(verbatim_doc_comment)]
    /// Edit file content with hashline verification
    ///
    /// Operations:
    ///   =  Replace line at locator
    ///   +  Insert new line after locator
    ///   -  Delete line at locator
    Edit {
        /// File to edit
        file: PathBuf,

        /// Operation: = (replace), + (insert after), - (delete)
        #[arg(value_name = "OP")]
        operation: String,

        /// Locator: LINE:HASH (e.g., "42:AB") or use 0:00 to insert at file beginning
        #[arg(value_name = "LOCATOR")]
        locator: String,

        /// Content for replace/insert operations (use - for stdin)
        #[arg(value_name = "CONTENT")]
        content: Option<String>,

        /// Preview changes without applying
        #[arg(long)]
        dry_run: bool,
    },
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}