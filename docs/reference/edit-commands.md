# Edit Commands

Commands for modifying file content.

## `edit` - Edit File Content

The unified command for all file edits: replace, insert, and delete.

### Usage

```
aifed edit <FILE> <OPERATION> <LOCATOR> [CONTENT]
aifed edit <FILE> [OPERATIONS...]    # Multiple operations via stdin/file
```

### Operations

| Operator | Syntax                  | Description                  |
| -------- | ----------------------- | ---------------------------- |
| `~`      | `~ <LOCATOR> <CONTENT>` | Replace content at locator   |
| `+`      | `+ <LOCATOR> <CONTENT>` | Insert content after locator |
| `-`      | `- <LOCATOR>`           | Delete content at locator    |

**Mnemonic:**
- `~` - Tilde suggests "modify" or "change" (used in regex, diff)
- `+` - Plus suggests "add" or "insert"
- `-` - Minus suggests "remove" or "delete"

### Locator Format

Edit commands use **hashline** locators to specify positions with verification.

| Format      | Example     | Description                                       |
| ----------- | ----------- | ------------------------------------------------- |
| `LINE:HASH` | `42:abc123` | Hashline - line + hash verification (recommended) |
| `HASH`      | `abc123`    | Hash only (content-based positioning)             |

**Virtual line:** The special hashline `0:000000` represents the position before the first line, used for inserting at the beginning of a file.

```bash
# Insert a copyright header at the very start of a file
aifed edit main.rs + 0:000000 "// Copyright 2026"
```

See [locator.md](locator.md) for detailed documentation on locators and hashline.

### Options

| Option          | Description                                       |
| --------------- | ------------------------------------------------- |
| `--file <FILE>` | Read operations from file (use `-` for stdin)     |
| `--auto-fmt`    | Auto-format after all operations                  |
| `--dry-run`     | Preview changes without applying                  |
| `--continue`    | Continue on individual operation failures         |
| `--force`       | Apply changes even if hash mismatch (use caution) |

### Examples

#### Single Operations

```bash
# Replace line 42 with hash verification
aifed edit main.rs ~ 42:abc123 "fn main() {"

# Insert after line 10
aifed edit main.rs + 10:abc123 "    println!(\"hello\");"

# Delete line 42
aifed edit main.rs - 42:abc123

# Insert at file beginning
aifed edit main.rs + 0:000000 "// Copyright 2026"
```

#### Batch Operations

```bash
# Multiple operations via heredoc
aifed edit main.rs <<EOF
~ 42:abc123 "fn main() {"
+ 10:def456 "    println!(\"hello\");"
- 15:ghi789
EOF

# From file
aifed edit main.rs --file ops.txt

# From stdin
cat ops.txt | aifed edit main.rs --file -

# Preview changes
aifed edit main.rs --file ops.txt --dry-run
```

#### With Options

```bash
# With auto-format
aifed edit main.rs ~ 42:abc123 "fn main(){" --auto-fmt

# Preview changes
aifed edit main.rs ~ 42:abc123 "fn main() {" --dry-run

# Continue on failures (best-effort mode)
aifed edit main.rs --file ops.txt --continue
```

### Content Input Methods

```bash
# Direct argument
aifed edit lib.rs ~ 42:abc123 "content"

# From stdin (single operation)
echo "content" | aifed edit lib.rs ~ 42:abc123 -

# Multi-line via heredoc
aifed edit lib.rs ~ 10-15 - <<EOF
fn new_func() -> Option<i32> {
    None
}
EOF
```

### Line Number Drift Solution

Batch edits use hashes instead of line numbers to avoid drift:

```
Original:
  L1: a
  L2: b
  L3: c

Edit operations:
  ~ abc123 "aa"    # Hash-based, valid
  - def456         # Hash-based, valid
  ~ ghi789 "cc"    # Hash-based, valid
```

Hashes are content-based, so they remain valid regardless of other edits.

### Failure Handling

- **Default:** Atomic (all-or-nothing) - if any operation fails, none are applied
- **With `--continue`:** Best-effort - continue on failures, report results

## Locator Quick Reference

**Note:** File path is a separate argument. Examples show full command-line context.

| Format      | Locator Only | Full Example        | Use Case                          |
| ----------- | ------------ | ------------------- | --------------------------------- |
| `LINE:HASH` | `42:abc123`  | `main.rs 42:abc123` | Default - safest, dual validation |
| `HASH`      | `abc123`     | `main.rs abc123`    | When line number unknown          |

**Virtual line** (`0:000000`) is a special hashline for inserting at file beginning.

See [locator.md](locator.md) for detailed documentation.

## See Also

- [Locator Reference](locator.md) - Detailed locator documentation
- [Read Commands](read-commands.md) - Getting hashes with info/read
- [History & Snapshots](history.md) - Undoing edits
