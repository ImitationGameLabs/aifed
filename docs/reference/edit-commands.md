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

| Format      | Example | Description                                       |
| ----------- | ------- | ------------------------------------------------- |
| `LINE:HASH` | `42:AB` | Hashline - line + hash verification (recommended) |
| `HASH`      | `AB`    | Hash only (content-based positioning)             |

**Virtual line:** The special hashline `0:00` represents the position before the first line, used for inserting at the beginning of a file.

```bash
# Insert a copyright header at the very start of a file
aifed edit main.rs + 0:00 "// Copyright 2026"
```

See [locator.md](locator.md) for detailed documentation on locators and hashline.

### Options

| Option          | Description                                   |
| --------------- | --------------------------------------------- |
| `--file <FILE>` | Read operations from file (use `-` for stdin) |
| `--auto-fmt`    | Auto-format after all operations              |
| `--dry-run`     | Preview changes without applying              |
| `--continue`    | Continue on individual operation failures     |

### Examples

#### Single Operations

```bash
# Replace line 42 with hash verification
aifed edit main.rs ~ 42:AB "fn main() {"

# Insert after line 10
aifed edit main.rs + 10:AB "    println!(\"hello\");"

# Delete line 42
aifed edit main.rs - 42:AB

# Insert at file beginning
aifed edit main.rs + 0:00 "// Copyright 2026"
```

#### Batch Operations

```bash
# Multiple operations via heredoc
aifed edit main.rs <<EOF
~ 42:AB "fn main() {"
+ 10:3K "    println!(\"hello\");"
- 15:7M
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
aifed edit main.rs ~ 42:AB "fn main(){" --auto-fmt

# Preview changes
aifed edit main.rs ~ 42:AB "fn main() {" --dry-run

# Continue on failures (best-effort mode)
aifed edit main.rs --file ops.txt --continue
```

### Content Input Methods

```bash
# Direct argument
aifed edit lib.rs ~ 42:AB "content"

# From stdin (single operation)
echo "content" | aifed edit lib.rs ~ 42:AB -

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
  ~ AB "aa"    # Hash-based, valid
  - 3K         # Hash-based, valid
  ~ 7M "cc"    # Hash-based, valid
```

Hashes are content-based, so they remain valid regardless of other edits.

### Failure Handling

- **Default:** Atomic (all-or-nothing) - if any operation fails, none are applied
- **With `--continue`:** Best-effort - continue on failures, report results

## Locator Quick Reference

**Note:** File path is a separate argument. Examples show full command-line context.

| Format      | Locator Only | Full Example    | Use Case                          |
| ----------- | ------------ | --------------- | --------------------------------- |
| `LINE:HASH` | `42:AB`      | `main.rs 42:AB` | Default - safest, dual validation |
| `HASH`      | `AB`         | `main.rs AB`    | When line number unknown          |

**Virtual line** (`0:00`) is a special hashline for inserting at file beginning.

See [locator.md](locator.md) for detailed documentation.

## See Also

- [Locator Reference](locator.md) - Detailed locator documentation
- [Read Commands](read-commands.md) - Getting hashes with info/read
- [History & Snapshots](history.md) - Undoing edits
