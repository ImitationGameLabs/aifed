# Utilities

Utility commands for comparing and formatting files.

## `diff` - Show File Differences

Show differences between versions or files.

### Usage

```
aifed diff <FILE> [--from <TAG>] [--to <TAG>]
aifed diff <FILE1> <FILE2>
```

### Options

| Option          | Description                           |
| --------------- | ------------------------------------- |
| `--stat`        | Show diffstat only                    |
| `--unified <N>` | Lines of unified context [default: 3] |

### Comparison Modes

| Mode                | Syntax                                  | Description                       |
| ------------------- | --------------------------------------- | --------------------------------- |
| Snapshot to current | `diff <FILE> --from <TAG>`              | Compare snapshot to current state |
| Between snapshots   | `diff <FILE> --from <TAG1> --to <TAG2>` | Compare two snapshots             |
| Between files       | `diff <FILE1> <FILE2>`                  | Compare two files                 |

### Examples

```bash
# Compare snapshot to current
aifed diff main.rs --from before-refactor

# Compare two snapshots
aifed diff main.rs --from v1 --to v2

# Compare two files
aifed diff old.rs new.rs

# Show diffstat only
aifed diff main.rs --from before-refactor --stat

# More context lines
aifed diff main.rs --from before-refactor --unified 5

# JSON output
aifed diff main.rs --from before-refactor --json
```

### Output Format

```
--- main.rs (before-refactor)
+++ main.rs (current)
@@ -42,7 +42,7 @@
 fn main() {
-    println!("old");
+    println!("new");
 }
```

---

## `format` - Format File(s)

Format file(s) using configured formatter.

### Usage

```
aifed format <FILE>
aifed format <PATH> --recursive
```

### Options

| Option      | Description                        |
| ----------- | ---------------------------------- |
| `--check`   | Check formatting without modifying |
| `--dry-run` | Preview changes                    |

### Formatter Configuration

Formatters are configured per-language in `.aifed.toml`:

```toml
[format]
go = "gofmt"
rust = "rustfmt"
javascript = "prettier --stdin-filepath"
python = "black -"
```

### Examples

```bash
# Format single file
aifed format main.rs

# Format directory recursively
aifed format src/ --recursive

# Check formatting (exit 1 if not formatted)
aifed format main.rs --check

# Preview changes
aifed format main.rs --dry-run

# JSON output
aifed format main.rs --json
```

### Check Mode

Use `--check` for CI/CD pipelines:

```bash
# In CI script
if ! aifed format src/ --recursive --check; then
    echo "Files are not formatted!"
    exit 1
fi
```

---

## `version` - Show Version

### Usage

```
aifed version
```

### Examples

```bash
aifed version
# Output: aifed 0.1.0

# JSON output
aifed version --json
# Output: {"version": "0.1.0"}
```

## Common Workflows

### Pre-commit Check

```bash
# Check formatting
aifed format . --recursive --check

# Check diagnostics
aifed diagnostics --all
```

### Post-edit Verification

```bash
# After editing
aifed edit main.rs ~ 42:AB "new code" --auto-fmt

# Check for errors
aifed diagnostics main.rs

# View changes
aifed diff main.rs --from before-edit
```

### Compare Before/After

```bash
# Create snapshot
aifed snapshot create main.rs --tag before

# Make changes
aifed edit main.rs ~ 42:AB "new code"

# View diff
aifed diff main.rs --from before

# Restore if needed
aifed snapshot restore main.rs --tag before
```

## See Also

- [History & Snapshots](history.md) - Creating and managing snapshots
- [Configuration](configuration.md) - Configuring formatters
- [LSP Integration](lsp.md) - Using diagnostics
