// CLI Design Philosophy
// =====================
// The main `aifed --help` output should serve as a complete, self-contained skill
// for AI agents. An agent reading this help alone should be able to:
//
// 1. Understand the core workflow (read → get hashes → edit with verification)
// 2. Know all available operators and their meanings
// 3. Parse the output format (LINE:HASH|CONTENT)
// 4. Use locators correctly (LINE:HASH, 0:00 virtual line)
// 5. Execute common operations from examples
//
// Therefore, when adding new features, ensure the main --help remains comprehensive.
// Subcommand help (--help) can provide additional detail but shouldn't be required
// for basic usage.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// AI-First Editor - A text editor designed for AI agents
///
/// aifed uses hashlines (LINE:HASH) to ensure deterministic, verifiable edits.
/// This prevents AI agents from making edits based on stale file state.
///
/// WORKFLOW:
///   1. Read file to get current hashes: aifed read <FILE>
///   2. Edit with hash verification: aifed edit <FILE> <OP> <LINE:HASH> [CONTENT]
///   3. Hash mismatch = file changed, re-read and retry
///      Tip: Use line range (e.g., "10-20") to re-read only nearby lines
///
/// OUTPUT FORMAT (aifed read):
///   LINE:HASH|CONTENT
///   - LINE: 1-based line number
///   - HASH: 2-char content hash (base32hex, characters 0-9 A-V)
///   - CONTENT: the actual line text
///     Example: "42:3K|fn main() {"
///
/// EDIT OPERATORS:
///   =  Replace line at locator
///   +  Insert new line after locator
///   -  Delete line at locator
///
/// LOCATORS:
///   LINE:HASH  Standard hashline (e.g., "42:3K")
///   0:00       Virtual line for inserting at file beginning
///
/// BATCH MODE:
///   Multiple operations can be provided via stdin (heredoc).
///   All operations must succeed, or none are applied (atomic).
///
/// EXAMPLES:
///   ```bash
///   # Single edit
///   aifed read main.rs              # Get hashes for all lines
///   aifed read main.rs 10-20        # Read lines 10-20
///   aifed edit main.rs = 42:3K "new content"    # Replace line 42
///   aifed edit main.rs + 10:AB "inserted line"  # Insert after line 10
///   aifed edit main.rs - 15:7M                  # Delete line 15
///   aifed edit main.rs + 0:00 "// header"       # Insert at file beginning
///
///   # Batch edit (heredoc)
///   aifed edit main.rs <<EOF
///   = 1:AB "modified"
///   + 10:3K "inserted"
///   - 15:7M
///   EOF
///   ```
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
    ///
    /// Batch mode (from stdin or file):
    ///   ```bash
    ///   aifed edit main.rs <<EOF
    ///   = 42:AB "new content"
    ///   + 10:3K "inserted line"
    ///   - 15:7M
    ///   EOF
    ///   ```
    Edit {
        /// File to edit
        file: PathBuf,

        /// Operation: = (replace), + (insert after), - (delete)
        /// Optional when using stdin for batch mode
        #[arg(value_name = "OP")]
        operation: Option<String>,

        /// Locator: LINE:HASH (e.g., "42:AB") or use 0:00 to insert at file beginning
        /// Optional when using stdin for batch mode
        #[arg(value_name = "LOCATOR")]
        locator: Option<String>,

        /// Content for replace/insert operations (use - for stdin)
        #[arg(value_name = "CONTENT")]
        content: Option<String>,

        /// Preview changes without applying
        #[arg(long)]
        dry_run: bool,
    },
}

impl Args {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
