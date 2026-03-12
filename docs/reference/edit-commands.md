# Edit Commands

Commands for modifying file content.

## Locator Quick Reference

Edit commands use **locators** to specify positions. The primary locator format is **hashline**, which combines line numbers with content hashes for safe, deterministic edits.

| Format       | Example        | Description                                       |
| ------------ | -------------- | ------------------------------------------------- |
| `:LINE:HASH` | `:42:abc12345` | Hashline - line + hash verification (recommended) |
| `:HASH`      | `:abc12345`    | Hash only (content-based positioning)             |
| `:LINE`      | `:42`          | Line number only (no verification)                |
| `:START-END` | `:10-20`       | Line range                                        |

See [locator.md](locator.md) for detailed documentation on locators and hashline.

---

## `replace` - Replace Line(s)

Replace content at specified location with hash verification.

### Usage

```
aifed replace <FILE>:<LOCATOR> <CONTENT>
```

### Options

| Option       | Description                                       |
| ------------ | ------------------------------------------------- |
| `--auto-fmt` | Auto-format after replace                         |
| `--dry-run`  | Preview changes without applying                  |
| `--force`    | Apply changes even if hash mismatch (use caution) |

### Locator Formats

| Format       | Example               | Use Case                          |
| ------------ | --------------------- | --------------------------------- |
| `:LINE:HASH` | `main.go:42:abc12345` | Default - safest, dual validation |
| `:HASH`      | `main.go:abc12345`    | When line number unknown          |
| `:LINE`      | `main.go:42`          | When hash unavailable             |
| `:START-END` | `main.go:10-20`       | Multi-line replacement            |

### Examples

```bash
# Replace line 42 with hash verification
aifed replace main.go:42:abc12345 "func main() {"

# Replace using hash only
aifed replace main.go:abc12345 "func main() {"

# Replace multi-line range
aifed replace main.go:10-15 <<EOF
func newFunc() {
    return nil
}
EOF

# With auto-format
aifed replace main.go:42:abc12345 "func main(){" --auto-fmt

# Preview changes
aifed replace main.go:42:abc12345 "func main() {" --dry-run
```

### Content Input Methods

```bash
# Direct argument
aifed replace file.go:42:abc123 "content"

# From stdin
echo "content" | aifed replace file.go:42:abc123 -

# Multi-line via heredoc
aifed replace file.go:10-15 <<EOF
line 1
line 2
EOF
```

---

## `insert` - Insert New Line(s)

Insert new content before or after a specified line.

### Usage

```
aifed insert <FILE> --after <LOCATOR> <CONTENT>
aifed insert <FILE> --before <LOCATOR> <CONTENT>
```

### Options

| Option               | Description                      |
| -------------------- | -------------------------------- |
| `--after <LOCATOR>`  | Insert after specified line      |
| `--before <LOCATOR>` | Insert before specified line     |
| `--auto-fmt`         | Auto-format after insert         |
| `--dry-run`          | Preview changes without applying |

### Locator Formats for `--after`/`--before`

| Format      | Example       | Description                                      |
| ----------- | ------------- | ------------------------------------------------ |
| `LINE:HASH` | `10:abc12345` | Line number with hash verification (recommended) |
| `LINE`      | `10`          | Line number only (no verification)               |

Note: For insert, the locator format is `LINE:HASH` (no leading colon).

### Examples

```bash
# Insert after line 10 with hash verification
aifed insert main.go --after 10:abc12345 "    fmt.Println(\"hello\")"

# Insert before line 1 (prepend)
aifed insert main.go --before 1:def456 "// Copyright 2026"

# Insert multi-line content
aifed insert main.go --after 42:ghi789 <<EOF
func helper() {
    return 42
}
EOF

# With auto-format
aifed insert main.go --after 10:abc123 "    new code" --auto-fmt
```

---

## `delete` - Delete Line(s)

Delete content at specified location with hash verification.

### Usage

```
aifed delete <FILE>:<LOCATOR>
```

### Options

| Option      | Description                                       |
| ----------- | ------------------------------------------------- |
| `--dry-run` | Preview changes without applying                  |
| `--force`   | Apply changes even if hash mismatch (use caution) |

### Locator Formats

| Format       | Example               | Use Case                          |
| ------------ | --------------------- | --------------------------------- |
| `:LINE:HASH` | `main.go:42:abc12345` | Default - safest, dual validation |
| `:HASH`      | `main.go:abc12345`    | When line number unknown          |
| `:LINE`      | `main.go:42`          | When hash unavailable             |
| `:START-END` | `main.go:10-20`       | Multi-line deletion               |

### Examples

```bash
# Delete line 42 with hash verification
aifed delete main.go:42:abc12345

# Delete using hash only
aifed delete main.go:abc12345

# Delete multi-line range
aifed delete main.go:10-15

# Preview changes
aifed delete main.go:42:abc12345 --dry-run
```

---

## `edit` - Atomic Batch Operations

Apply multiple operations atomically. All succeed or all fail.

### Usage

```
aifed edit <FILE> [OPERATIONS]
aifed edit <FILE> --file <OPS_FILE>
```

### Input Format

One operation per line:

```
replace <LOCATOR> <CONTENT>
insert --after <LOCATOR> <CONTENT>
insert --before <LOCATOR> <CONTENT>
delete <LOCATOR>
```

Note: For edit, locator format is `LINE:HASH` (no leading colon).

### Options

| Option          | Description                                   |
| --------------- | --------------------------------------------- |
| `--file <FILE>` | Read operations from file (use `-` for stdin) |
| `--auto-fmt`    | Auto-format after all operations              |
| `--dry-run`     | Preview changes without applying              |
| `--continue`    | Continue on individual operation failures     |

### Line Number Drift Solution

Edit uses hashes instead of line numbers to avoid drift:

```
Original:
  L1: a
  L2: b
  L3: c

Edit operations:
  replace hash_a "aa"    # Hash-based, valid
  delete hash_b          # Hash-based, valid
  replace hash_c "cc"    # Hash-based, valid
```

Hashes are content-based, so they remain valid regardless of other edits.

### Examples

```bash
# Interactive edit operations
aifed edit main.go <<EOF
replace 42:abc12345 "func main() {"
insert --after 10:def456 "    fmt.Println(\"hello\")"
delete 15:ghi789
EOF

# From file
aifed edit main.go --file ops.txt

# From stdin
cat ops.txt | aifed edit main.go --file -

# Preview changes
aifed edit main.go --file ops.txt --dry-run

# Continue on failures
aifed edit main.go --file ops.txt --continue
```

### Failure Handling

- **Default:** Atomic (all-or-nothing) - if any operation fails, none are applied
- **With `--continue`:** Best-effort - continue on failures, report results

## See Also

- [Locator Reference](locator.md) - Detailed locator documentation
- [File Operations](file-operations.md) - Getting hashes with info/read
- [History & Snapshots](history.md) - Undoing edits
