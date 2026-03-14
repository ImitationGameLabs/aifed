# Read Commands

Commands for reading file information and content.

## `read` - Read File Content

Read file content with line hashes (default for AI editing).

### Usage

```
aifed read <FILE> [LOCATOR]
```

### Options

| Option          | Description                                            |
| --------------- | ------------------------------------------------------ |
| `--no-hashes`   | Exclude line hashes (rarely needed for AI)             |
| `--context <N>` | Show N lines of context around target                  |
| `--symbols`     | Include Symbol Locators for LSP operations (on-demand) |

### Output Format (default)

```
L1:abc123  fn main() {
L2:def456      println!("hello");
L3:ghi789  }
```

Note: Each line prefixed with `LINE:HASH` for use in edit commands. Hashes are included by default since AI needs them for safe editing.

### Position Arguments

| Argument    | Example         | Description        |
| ----------- | --------------- | ------------------ |
| None        | `main.rs`       | Read entire file   |
| `LINE`      | `main.rs 15`    | Read specific line |
| `START-END` | `main.rs 10-20` | Read line range    |

### Examples

```bash
# Read entire file with hashes (default)
aifed read main.rs

# Read specific line range
aifed read main.rs 10-20

# Read specific line with context
aifed read main.rs 15 --context 5

# Read without hashes (rarely needed)
aifed read main.rs --no-hashes

# JSON output
aifed read main.rs --json
```

### Large File Handling

For large files (>10,000 lines), consider:
- Using `START-END` to read specific sections
- Using `--context` to focus on relevant areas

### `read --symbols` - Read with Symbol Locators

Get Symbol Locators for LSP operations. This is an on-demand feature - use it when you need column-level precision without counting characters.

```bash
aifed read main.rs 15 --symbols
```

Output:
```
L15:def456  let user: User = get_user(user);
            S1:user
            S2:User
            S3:get_user
            S4:user
```

Note: Same-named symbols (e.g., `S1:user` and `S4:user`) are independent - renaming one does not affect the other. See [Locator Reference](locator.md) for details.

**Output format:**
- `L15:def456` - Line Locator (for edit operations)
- `S1:user` - Symbol Locator (for LSP operations)

**When to use:**
- Preparing for LSP operations: rename, hover, definition, references
- When you need to target a specific symbol on a line
- Avoiding character counting for column positions

**Token efficiency:**
- Regular `read` (without `--symbols`) is more token-efficient for edit operations
- Use `--symbols` only when you need Symbol Locators for LSP commands

**Example workflow:**
```bash
# 1. Read file to understand code
aifed read main.rs 15

# 2. If you need LSP operation, get Symbol Locators
aifed read main.rs 15:abc123 --symbols

# 3. Use Symbol Locator in LSP command
aifed hover main.rs S1:user
```

### When to Use

- **Understanding code** - Read file content
- **Before editing** - Hashes included by default
- **Focused reading** - Use ranges for specific sections


## `info` - Get File Metadata

Get file metadata without content.

### Usage

```
aifed info <FILE>
```

### Options

| Option   | Description           |
| -------- | --------------------- |
| `--json` | Output in JSON format |

### Output Format (Text)

```
File: main.rs
Lines: 150
Size: 3.2 KB
```

### Output Format (JSON)

```json
{
  "path": "main.rs",
  "lines": 150,
  "size": 3276
}
```

### Examples

```bash
# Basic metadata
aifed info main.rs

# JSON output
aifed info main.rs --json
```

### When to Use

- **Quick overview** - Check file size, line count
- **Planning** - Understand file scale before reading
- **Filtering** - Decide which files to process

## `read` vs `info`

| Command | Purpose      | Output                   |
| ------- | ------------ | ------------------------ |
| `read`  | File content | Content with line hashes |
| `info`  | Metadata     | Path, lines, size, stats |

**Use `read` for:**
- Reading actual file content
- Getting hashes for editing
- Preparing for LSP operations (`--symbols`)

**Use `info` for:**
- Checking file size/line count before reading
- Quick file overview without content

## See Also

- [Edit Commands](edit-commands.md) - Using hashes in edits
- [Locator Reference](locator.md) - Understanding the locator format
- [LSP Integration](lsp.md) - Getting symbols via LSP
