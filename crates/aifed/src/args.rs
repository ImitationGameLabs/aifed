// CLI Design Philosophy
// =====================
// - `--help` provides quick command reference
// - `--skill` provides detailed usage guide
//
// When adding new features, update skill.md for documentation.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// AI-First Editor - A text editor designed for AI agents
///
/// Uses hashlines for deterministic, verifiable edits.
///
/// IMPORTANT: Run `aifed --skill` before any read/write operations.
#[derive(Parser, Debug)]
#[command(
    name = "aifed",
    version,
    about,
    verbatim_doc_comment,
    disable_help_flag = true,
    disable_version_flag = true
)]
pub struct Args {
    /// Output in JSON format
    #[arg(long, global = true)]
    pub json: bool,

    /// Print skill document for AI agents
    #[arg(long, global = true, action = clap::ArgAction::SetTrue)]
    pub skill: Option<bool>,

    /// Print help
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
    /// Operations are read from stdin:
    ///   ```bash
    ///   aifed edit main.rs <<'EOF'
    ///   = 42:AB "new content"
    ///   + 10:3K "inserted line"
    ///   - 15:7M
    ///   - [20:XX,30:YY]
    ///   EOF
    ///   ```
    ///
    /// Operations:
    ///   =  Replace line at locator
    ///   +  Insert new line after locator
    ///   -  Delete line at locator (or range with [start:end])
    Edit {
        /// File to edit
        file: PathBuf,

        /// Preview changes without applying
        #[arg(long)]
        dry_run: bool,
    },

    /// Daemon management commands
    #[command(subcommand)]
    Daemon(DaemonCommands),

    /// LSP operations (requires running daemon)
    #[command(subcommand)]
    Lsp(LspCommands),

    /// View edit history for a file
    History {
        /// File path
        file: PathBuf,

        /// Number of entries to show
        #[arg(long, value_name = "N")]
        count: Option<usize>,

        /// Show compact summary instead of detailed diffs
        #[arg(long)]
        stat: bool,
    },

    /// Undo the last edit for a file
    Undo {
        /// File path
        file: PathBuf,

        /// Preview changes without applying
        #[arg(long)]
        dry_run: bool,
    },

    /// Redo the last undone edit for a file
    Redo {
        /// File path
        file: PathBuf,

        /// Preview changes without applying
        #[arg(long)]
        dry_run: bool,
    },

    /// Copy lines from file to clipboard
    Copy {
        /// File to copy from
        file: PathBuf,

        /// Hashline range (e.g., "[1:AB,5:CD]")
        range: String,
    },

    /// Paste clipboard content to file
    Paste {
        /// File to paste into
        file: PathBuf,

        /// Hashline position where to insert (e.g., "10:AB")
        position: String,
    },

    /// Show clipboard content
    Clipboard,
}

/// Daemon management commands
#[derive(Subcommand, Debug)]
pub enum DaemonCommands {
    /// Check daemon status
    Status,

    /// Stop daemon
    Stop {
        /// Force stop
        #[arg(long)]
        force: bool,
    },
}

/// LSP operations
///
/// Uses locator format for precise positioning:
/// - LINE:HASH - e.g., "15:3K" - verifies line content
/// - SINDEX:NAME - e.g., "S1:config" - locates symbol on line
#[derive(Subcommand, Debug)]
pub enum LspCommands {
    /// Get symbol locators for a line
    ///
    /// Outputs the line with hashline and symbol locators:
    ///   15:3K|let config = load_config();
    ///   S1:config
    ///   S2:load_config
    Symbols {
        /// File path
        file: PathBuf,
        /// Line number or range (e.g., "15" or "10-20")
        #[arg(value_name = "LINE|RANGE")]
        locator: String,
    },

    /// Get diagnostics for file
    Diag {
        /// File path
        file: PathBuf,
    },

    /// Get hover information at symbol
    Hover {
        /// File path
        file: PathBuf,
        /// Hashline locator (e.g., "15:3K")
        hashline: String,
        /// Symbol locator (e.g., "S1:config")
        symbol: String,
    },

    /// Go to definition
    Def {
        /// File path
        file: PathBuf,
        /// Hashline locator (e.g., "15:3K")
        hashline: String,
        /// Symbol locator (e.g., "S1:config")
        symbol: String,
    },

    /// Find references
    Refs {
        /// File path
        file: PathBuf,
        /// Hashline locator (e.g., "15:3K")
        hashline: String,
        /// Symbol locator (e.g., "S1:config")
        symbol: String,
    },

    /// Get completions at symbol
    Complete {
        /// File path
        file: PathBuf,
        /// Hashline locator (e.g., "15:3K")
        hashline: String,
        /// Symbol locator (e.g., "S1:config")
        symbol: String,
    },

    /// Rename symbol
    Rename {
        /// File path
        file: PathBuf,
        /// Hashline locator (e.g., "15:3K")
        hashline: String,
        /// Symbol locator (e.g., "S1:config")
        symbol: String,
        /// New name for the symbol
        new_name: String,
        /// Show preview without applying changes
        #[arg(long)]
        dry_run: bool,
    },
}

impl Args {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
