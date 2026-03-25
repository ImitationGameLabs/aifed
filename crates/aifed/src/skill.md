# aifed - AI-First Editor

aifed uses hashlines (LINE:HASH) to ensure deterministic, verifiable edits.
This prevents AI agents from making edits based on stale file state.

## WORKFLOW

1. Read file to get current hashes: aifed read <FILE>
2. Edit with hash verification via heredoc: aifed edit <FILE> <<'EOF' ... EOF
3. Hash mismatch = file changed, re-read and retry
   Tip: Use line range (e.g., "10-20") to re-read only nearby lines

## OUTPUT FORMAT (aifed read)

LINE:HASH|CONTENT
- LINE: 1-based line number
- HASH: 2-char content hash (base32hex, characters 0-9 A-V)
- CONTENT: the actual line text
  Example: "42:3K|fn main() {"

## EDIT OPERATORS

=   Replace line at locator
+   Insert new line after locator
-   Delete line at locator (supports range: `- [START:HASH,END:HASH]`)

## LINE LOCATORS

LINE:HASH                Standard hashline (e.g., "42:3K")
0:00                     Virtual line for inserting at file beginning
[START:HASH,END:HASH]    Range locator for delete (e.g., "[2:AA,89:BB]" deletes lines 2-89, boundary hash verified)

## STRING ESCAPING (JSON-style)

Content in double quotes supports JSON escape sequences:
- `\"` → `"` (double quote)
- `\\` → `\` (backslash)
- `\n` → newline
- `\t` → tab
- `\r` → carriage return
- `\uXXXX` → Unicode character

Example: `"code: println!(\"hello\");"` becomes `code: println!("hello");`

## BATCH MODE

All edits use heredoc syntax. Multiple operations can be provided in one heredoc.
All operations must succeed, or none are applied (atomic).

## EDITING TIPS

When editing multiple locations in the same file:
- Prefer batch edit over sequential single edits
- Sequential edits cause line number shifts, requiring re-read before each edit
- Batch edit processes all operations against the same file state
- For symbol renaming, use `aifed lsp rename` which handles all occurrences

Example: Adding a parameter to a function and updating 3 call sites
- Bad: 4 sequential `aifed edit` commands (line numbers shift after each insert/delete)
- Good: 1 batch edit with all 4 operations (processed against same file state)

## TOOL CONSISTENCY

Do not mix aifed with other file editing toolsets (built-in file tools, shell commands like `cat`/`sed`, etc.).

Alternating between toolsets breaks integrity checks on both sides. Modifications made through one tool are invisible to the other, causing verification failures, requiring re-reads, and wasting tokens.

## LSP COMMANDS (requires running daemon)

aifed lsp symbols <FILE> <LINE>       - Get symbol locators for a line
aifed lsp diag <FILE>                 - Get diagnostics
aifed lsp hover <FILE> <LINE:HASH> <SINDEX:NAME>   - Get hover info
aifed lsp def <FILE> <LINE:HASH> <SINDEX:NAME>     - Go to definition
aifed lsp refs <FILE> <LINE:HASH> <SINDEX:NAME>    - Find references
aifed lsp complete <FILE> <LINE:HASH> <SINDEX:NAME> - Get completions
aifed lsp rename <FILE> <LINE:HASH> <SINDEX:NAME> <NAME> - Rename symbol

## DAEMON COMMANDS

aifed daemon status   - Check daemon status
aifed daemon stop     - Stop daemon

Notes:
- `daemon status` shows bin path, log path, and socket path for troubleshooting
- Daemon auto-starts when needed (on first command requiring it)
- Daemon auto-exits after idle timeout (default: 30 minutes)

## HISTORY COMMANDS (requires running daemon)

aifed history <FILE> [--count N] [--stat]   - View edit history
aifed undo <FILE> [--dry-run]               - Undo last edit
aifed redo <FILE> [--dry-run]               - Redo last undone edit

Notes:
- History is stored in daemon memory (not persisted to disk)
- History is tracked independently per file
- Restarting the daemon clears all history

Options:
  --count N   Limit number of history entries
  --stat      Show compact summary instead of detailed diffs
  --dry-run   Preview changes without applying

## EXAMPLES

```bash
# Read file
aifed read main.rs              # Get hashes for all lines
aifed read main.rs [10,20]      # Read lines 10-20

# Single edit - use heredoc with 'EOF' to prevent shell expansion
aifed edit main.rs <<'EOF'
= 42:3K "new content"
EOF

# Insert after line 10
aifed edit main.rs <<'EOF'
+ 10:AB "inserted line"
EOF

# Delete line 15
aifed edit main.rs <<'EOF'
- 15:7M
EOF

# Insert at file beginning
aifed edit main.rs <<'EOF'
+ 0:00 "// header"
EOF

# Batch edit - multiple operations in one heredoc
aifed edit main.rs <<'EOF'
= 1:AB "modified"
+ 10:3K "inserted"
- 15:7M
EOF

# Range delete - delete lines 10-50 with boundary hash verification
aifed edit main.rs <<'EOF'
- [10:AB,50:CD]
EOF

# Content with JSON escaping
aifed edit main.rs <<'EOF'
= 1:AB "println!(\"result: {}\", value);"
+ 5:CD "{\"key\": \"value\"}"
EOF

# LSP operations (requires running daemon)
aifed daemon status
aifed lsp symbols src/main.rs 10      # Get symbols: S1:fn S2:main
aifed lsp hover src/main.rs 10:3K S2:main
aifed lsp def src/main.rs 10:3K S2:main
aifed lsp rename src/main.rs 10:3K S2:args cli_args  # Rename all occurrences

# History operations (requires running daemon)
aifed history src/main.rs             # View all edit history
aifed history src/main.rs --count 5   # View last 5 entries
aifed history src/main.rs --stat      # View compact summary
aifed undo src/main.rs                # Undo last edit
aifed undo src/main.rs --dry-run      # Preview undo without applying
aifed redo src/main.rs                # Redo last undone edit
```
