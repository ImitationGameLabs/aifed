# File Operations

Commands for reading file information and content.

## `info` - Get File Information

Get structured file information including line hashes.

### Usage

```
aifed info <FILE>
```

### Options

| Option        | Description                                |
| ------------- | ------------------------------------------ |
| `--no-hashes` | Exclude line hashes (rarely needed for AI) |
| `--symbols`   | Include symbol list                        |
| `--imports`   | Include import list                        |
| `--stats`     | Include file statistics                    |

### Output Format (Text)

```
File: main.go
Lines: 150
Hashes:
  L1:abc123  package main
  L2:def456  import "fmt"
  ...
```

Note: Output format is `LINE:HASH` for display. When using in edit commands, prepend a colon: `main.go:1:abc123`.

### Output Format (JSON)

```json
{
  "path": "main.go",
  "lines": 150,
  "hashes": {
    "1": "abc123",
    "2": "def456"
  },
  "symbols": ["main", "process"]
}
```

### Examples

```bash
# Basic info with hashes (default)
aifed info main.go

# Full info including symbols
aifed info main.go --symbols --imports --stats

# JSON for AI consumption
aifed info main.go --json

# Exclude hashes (rarely needed)
aifed info main.go --no-hashes
```

### When to Use

- **Before editing** - Get hashes for safe edits
- **Planning changes** - Understand file structure
- **Debugging** - Check current file state

---

## `read` - Read File Content

Read file content with line hashes (default for AI editing).

### Usage

```
aifed read <FILE>[:LINE]
aifed read <FILE>[:START-END]
```

### Options

| Option          | Description                                |
| --------------- | ------------------------------------------ |
| `--no-hashes`   | Exclude line hashes (rarely needed for AI) |
| `--context <N>` | Show N lines of context around target      |

### Output Format (default)

```
L1:abc123  package main
L2:def456  import "fmt"
L3:ghi789
```

Note: Each line prefixed with `LINE:HASH` for use in edit commands. Hashes are included by default since AI needs them for safe editing.

### Position Suffixes

| Suffix       | Example         | Description        |
| ------------ | --------------- | ------------------ |
| None         | `main.go`       | Read entire file   |
| `:LINE`      | `main.go:15`    | Read specific line |
| `:START-END` | `main.go:10-20` | Read line range    |

### Examples

```bash
# Read entire file with hashes (default)
aifed read main.go

# Read specific line range
aifed read main.go:10-20

# Read specific line with context
aifed read main.go:15 --context 5

# Read without hashes (rarely needed)
aifed read main.go --no-hashes

# JSON output
aifed read main.go --json
```

### Large File Handling

For large files (>10,000 lines), consider:
- Using `:START-END` to read specific sections
- Using `--context` to focus on relevant areas

### When to Use

- **Understanding code** - Read file content
- **Before editing** - Hashes included by default
- **Focused reading** - Use ranges for specific sections

## `info` vs `read`

| Command | Purpose             | Output                          |
| ------- | ------------------- | ------------------------------- |
| `info`  | Metadata about file | Hashes, symbols, imports, stats |
| `read`  | Actual file content | File content with hashes        |

**Use `info` for:**
- Getting file-level metadata
- Understanding file structure (symbols, imports)
- Checking file statistics

**Use `read` for:**
- Understanding code content
- Reading specific sections
- Getting content + hashes for editing (default)

## See Also

- [Edit Commands](edit-commands.md) - Using hashes in edits
- [Locator Reference](locator.md) - Understanding the locator format
- [LSP Integration](lsp.md) - Getting symbols via LSP
