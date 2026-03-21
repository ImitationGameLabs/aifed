# aifed - AI-First Editor

aifed uses hashlines (LINE:HASH) to ensure deterministic, verifiable edits.
This prevents AI agents from making edits based on stale file state.

## WORKFLOW

1. Read file to get current hashes: aifed read <FILE>
2. Edit with hash verification: aifed edit <FILE> <OP> <LINE:HASH> [CONTENT]
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

Multiple operations can be provided via stdin (heredoc).
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

## EXAMPLES

```bash
# Single edit
aifed read main.rs              # Get hashes for all lines
aifed read main.rs 10-20        # Read lines 10-20
aifed edit main.rs = 42:3K "new content"    # Replace line 42
aifed edit main.rs + 10:AB "inserted line"  # Insert after line 10
aifed edit main.rs - 15:7M                  # Delete line 15
aifed edit main.rs + 0:00 "// header"       # Insert at file beginning

# Batch edit (heredoc) - use 'EOF' to prevent shell expansion
aifed edit main.rs <<'EOF'
= 1:AB "modified"
+ 10:3K "inserted"
- 15:7M
EOF

# Range delete - delete lines 10-50 with boundary hash verification
aifed edit main.rs - [10:AB,50:CD]

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
```
