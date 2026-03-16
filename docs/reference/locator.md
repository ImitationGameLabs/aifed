# Locator Reference

The locator is aifed's positioning mechanism for safe, deterministic edits.

## What is a Locator?

A locator identifies a specific position in a file. aifed uses different locator formats for different commands:

- **Read Locators** - Used with `read` command to specify what to read
- **Edit Locators** - Used with `edit` command to specify where to edit

## Edit Locators

Edit commands use **hashline** as the primary locator format, which combines line numbers with content hashes to ensure edits are applied at the correct location.

### Why Line + Hash?

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

### `LINE:HASH` - Hashline

The recommended format for edit operations.

```
42:abc123
```

- `42` - Line number (human-readable, helps locate quickly)
- `abc123` - 6-character content hash (verification)

**Virtual Line:** The special value `0:000000` represents the position before the first line, used for inserting at the beginning of a file:

```bash
# Insert at file beginning
aifed edit main.rs + 0:000000 "// Copyright 2026"
```

### `HASH` - Hash Only

Content-based positioning without line number.

```
abc123
```

**When to use:** When line number is unknown or you want pure content-based positioning.

---

## Read Locators

Read commands use simpler formats since no verification is needed (you're just reading, not modifying).

### `LINE` - Line Number

Read a specific line.

```
15
```

### `START-END` - Line Range

Read a range of lines.

```
10-20
```

## Hash Algorithm

aifed uses content hashing for line identification. The exact algorithm is TBD pending benchmark results.

### Options Being Considered

| Option                    | Hash Length | Trade-offs                     |
| ------------------------- | ----------- | ------------------------------ |
| xxHash64 (hex)            | 6 chars     | Fast, sufficient               |
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
  File: main.rs
  Expected hash: abc123
  Actual hash: def456
  Actual content: fn main() {
  Hint: Run 'aifed info main.rs' to get current hashes
```

### Resolution Options

1. **Re-read the file** - Get current hashes with `aifed read <FILE>`
2. **Use `--force` flag** - Apply anyway (use with caution)

## Format Summary

**Note:** File path is a separate command argument, not part of the locator. Examples below show full command-line context for clarity.

### Edit Locators (for `edit` command)

| Format    | Syntax      | Locator Only | Full Example        | Use Case            |
| --------- | ----------- | ------------ | ------------------- | ------------------- |
| Hashline  | `LINE:HASH` | `42:abc123`  | `main.rs 42:abc123` | Default, safest     |
| Hash only | `HASH`      | `abc123`     | `main.rs abc123`    | Line number unknown |

**Virtual line** (`0:000000`) is a special hashline value for inserting at file beginning.

### Read Locators (for `read` command)

| Format | Syntax      | Locator Only | Full Example    | Use Case            |
| ------ | ----------- | ------------ | --------------- | ------------------- |
| Line   | `LINE`      | `42`         | `main.rs 42`    | Read specific line  |
| Range  | `START-END` | `10-20`      | `main.rs 10-20` | Read multiple lines |

---

## Symbol Locator

For LSP operations that require column-level precision (rename, hover, definition, references), use Symbol Locator combined with a hashline.

### Why Symbol Locator?

LLMs cannot reliably count character positions. Numeric columns are error-prone:
- AI must count to the Nth character
- No verification mechanism
- Position changes with code modifications

Symbol Locator solves this by using semantic information from LSP.

### Format

Symbol Locator format:

```
S<INDEX>:<NAME>
```

- `INDEX` - Sequential number (1-based) for symbols on that line
- `NAME` - Symbol name for self-documentation and LSP verification

**Usage in LSP commands:** Symbol Locator is always paired with a hashline:

```
aifed <CMD> <FILE> <LINE:HASH> <SINDEX:NAME>
```

### Getting Symbol Locators

Use `symbols` command with a line number to get both the hashline and symbol locators:

```bash
aifed symbols main.rs 15
```

Output:
```
15:def456  let config = load_config();
    S1:config
    S2:load_config
```

The output provides everything needed for LSP operations: `15:def456` (hashline) and `S1:config` (symbol locator).

### When to Use

| Locator Type   | Format        | Use Case                                               |
| -------------- | ------------- | ------------------------------------------------------ |
| Line Locator   | `LINE:HASH`   | Edit operations (`~`, `+`, `-`)                        |
| Symbol Locator | `SINDEX:NAME` | Symbol identification (used with Line Locator for LSP) |

For LSP operations, both locators are required: `LINE:HASH` identifies the line, `SINDEX:NAME` identifies the symbol on that line.

### Example Usage

```bash
# LSP operations require both hashline and symbol locator
aifed rename main.rs 15:def456 S1:config settings
aifed hover main.rs 15:def456 S2:load_config
aifed definition main.rs 15:def456 S2:load_config
aifed references main.rs 15:def456 S1:config
```

---

## Usage in Commands

Locators are used with the `edit` command:

```bash
# Replace with LINE:HASH format
aifed edit main.rs ~ 42:abc123 "new content"

# Insert after a line
aifed edit main.rs + 10:abc123 "new line"

# Insert at file beginning (virtual line)
aifed edit main.rs + 0:000000 "// Copyright 2026"

# Delete a line
aifed edit main.rs - 42:abc123

# Batch operations
aifed edit main.rs <<EOF
~ 42:abc123 "new content"
+ 10:def456 "another line"
- 15:ghi789
EOF
```

## Getting Hashes

Use `read`:

```bash
# Read file (hashes included by default)
aifed read main.rs
```

Output format (text):
```
1:abc123  fn main() {
2:def456      println!("hello");
```

Note: The `LINE:HASH` format matches the locator syntax for easy copy-paste into edit commands.

## See Also

- [Edit Commands](edit-commands.md) - Using locators in edits
- [Read Commands](read-commands.md) - Getting hashes with info/read
- [CLI Design Notes](../cli-design-notes.md) - Design rationale

## References

The hashline concept was inspired by:
- [oh-my-pi: Hashline Edit Mode](https://deepwiki.com/can1357/oh-my-pi/8.1-hashline-edit-mode)
- [The Harness Problem](https://blog.can.ac/2026/02/12/the-harness-problem) - Discusses the challenges of AI editing files and the need for deterministic positioning
