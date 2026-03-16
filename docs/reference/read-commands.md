# Read Commands

Commands for reading file information and content.

## `read` - Read File Content

Read file content with line hashes (default for AI editing).

### Usage

```
aifed read <FILE> [LOCATOR]
```

### Options

| Option          | Description                                        |
| --------------- | -------------------------------------------------- |
| `--no-hashes`   | Exclude line hashes (for exploration, not editing) |
| `--context <N>` | Show N lines of context around target              |
### Output Format (default)

```
1:AB|fn main() {
2:3K|    println!("hello");
3:7M|}
```

Note: Each line prefixed with `LINE:HASH` for use in edit commands. Hashes are included by default since AI needs them for safe editing.

### Position Arguments

Read commands use simple position specifiers (no hash verification needed since you're only reading):

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

# Read without hashes (for exploration, saves tokens)
aifed read main.rs --no-hashes

# JSON output
aifed read main.rs --json
```

### Large File Handling

For large files (>10,000 lines), consider:
- Using `START-END` to read specific sections
- Using `--context` to focus on relevant areas

## When to Use

- **Understanding code** - Read file content
- **Before editing** - Hashes included by default
- **Exploration** - Use `--no-hashes` to save tokens when not planning to edit
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

**Use `info` for:**
- Checking file size/line count before reading
- Quick file overview without content

## See Also

- [Edit Commands](edit-commands.md) - Using hashes in edits
- [Locator Reference](locator.md) - Understanding the locator format
- [LSP Integration](lsp.md) - Getting symbols via LSP
