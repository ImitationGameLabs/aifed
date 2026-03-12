# Locator Reference

The locator is aifed's positioning mechanism for safe, deterministic edits.

## What is a Locator?

A locator identifies a specific position in a file for editing operations.

aifed uses **hashline** as its primary locator format, which combines line numbers with content hashes to ensure edits are applied at the correct location.

## Why Line + Hash?

Traditional line-number-only positioning has problems:

| Issue            | Line Numbers Only                     | Line + Hash                          |
| ---------------- | ------------------------------------- | ------------------------------------ |
| Drift            | Line numbers change when files change | Hash identifies exact content        |
| Concurrent edits | Unexpected modifications              | Hash mismatch catches conflicts      |
| Stale cache      | AI's cached state becomes invalid     | Self-validating with current content |

**Benefits:**
- **Deterministic** - Hash identifies exact content
- **Self-validating** - Mismatch means file changed
- **AI-friendly** - AI gets hashes when reading, includes them when editing
- **Human-readable** - Line number visible for debugging

## Locator Formats

### `:LINE:HASH` - Hashline (Default)

The recommended format for most use cases, also known as **hashline**.

```
main.go:42:abc12345
```

- `42` - Line number (human-readable, helps locate quickly)
- `abc12345` - 8-character content hash (verification)

**When to use:** Default choice for most edits. Provides both human-readability and safety.

### `:HASH` - Hash Only

Content-based positioning without line number.

```
main.go:abc12345
```

**When to use:** When line number is unknown or you want pure content-based positioning.

### `:LINE` - Line Number Only

Positioning without hash verification.

```
main.go:42
```

**When to use:** When hash is unavailable and you accept the risk of potential drift.

### `:START-END` - Line Range

For multi-line operations.

```
main.go:10-20
```

**When to use:** Replacing or deleting multiple lines.

## Hash Algorithm

aifed uses content hashing for line identification. The exact algorithm is TBD pending benchmark results.

### Options Being Considered

| Option                    | Hash Length | Trade-offs                     |
| ------------------------- | ----------- | ------------------------------ |
| xxHash64 (hex)            | 8 chars     | Lower collision, longer output |
| oh-my-pi style (xxHash32) | 2 chars     | Compact, higher collision rate |

**oh-my-pi approach** (reference: [Hashline Edit Mode](https://deepwiki.com/can1357/oh-my-pi/8.1-hashline-edit-mode)):
- Uses xxHash32, truncates to lowest byte
- Maps to 2-char string using 16-char alphabet
- Strips all whitespace before hashing
- For symbol-only lines, mixes in line number as seed

See [CLI Design Notes](../cli-design-notes.md) for the full comparison and decision rationale.

## Hash Mismatch Behavior

When the provided hash doesn't match the current line content:

```
Error: Hash mismatch
  File: main.go
  Expected hash: abc12345
  Actual hash: def67890
  Actual content: func main() {
  Hint: Run 'aifed info main.go' to get current hashes
```

**Exit code:** 4

### Resolution Options

1. **Re-read the file** - Get current hashes with `aifed read <FILE>`
2. **Use `--force` flag** - Apply anyway (use with caution)

## Format Summary

| Format    | Syntax       | Example               | Verification       |
| --------- | ------------ | --------------------- | ------------------ |
| Line+Hash | `:LINE:HASH` | `main.go:42:abc12345` | Full (recommended) |
| Hash only | `:HASH`      | `main.go:abc12345`    | Content-based      |
| Line only | `:LINE`      | `main.go:42`          | None               |
| Range     | `:START-END` | `main.go:10-20`       | None               |

## Usage in Commands

Locators are used in edit commands:

```bash
# replace - uses :LINE:HASH format
aifed replace main.go:42:abc12345 "new content"

# insert - uses LINE:HASH format (no leading colon)
aifed insert main.go --after 10:abc12345 "new line"

# delete - uses :LINE:HASH format
aifed delete main.go:42:abc12345

# edit - uses LINE:HASH format
aifed edit main.go <<EOF
replace 42:abc12345 "new content"
delete 15:ghi789
EOF
```

## Getting Hashes

Use `info` or `read`:

```bash
# Get file info with hashes
aifed info main.go

# Read file (hashes included by default)
aifed read main.go
```

Output format (text):
```
L1:abc123  package main
L2:def456  import "fmt"
```

Note: The `LINE:HASH` format matches the locator syntax for easy copy-paste into edit commands.

## See Also

- [Edit Commands](edit-commands.md) - Using locators in edits
- [File Operations](file-operations.md) - Getting hashes with info/read
- [CLI Design Notes](../cli-design-notes.md) - Design rationale

## References

The hashline concept was inspired by:
- [oh-my-pi: Hashline Edit Mode](https://deepwiki.com/can1357/oh-my-pi/8.1-hashline-edit-mode)
- [The Harness Problem](https://blog.can.ac/2026/02/12/the-harness-problem) - Discusses the challenges of AI editing files and the need for deterministic positioning
