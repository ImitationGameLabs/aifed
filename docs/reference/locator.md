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
42:AB
```

- `42` - Line number (human-readable, helps locate quickly)
- `AB` - 2-character content hash (verification)

**Virtual Line:** The special value `0:00` represents the position before the first line, used for inserting at the beginning of a file:

```bash
# Insert at file beginning
aifed edit main.rs + 0:00 "// Copyright 2026"
```

### `[START:HASH,END:HASH]` - Hashline Range

Range locator for deleting multiple lines. Only boundary hashes are verified.

```
[10:AB,50:CD]
```

- `10` - Start line number
- `AB` - Start line content hash
- `50` - End line number (inclusive)
- `CD` - End line content hash

**Usage:** Only supported with delete (`-`) operation.

```bash
# Delete lines 10-50 (inclusive)
aifed edit main.rs - [10:AB,50:CD]
```

### `HASH` - Hash Only

Content-based positioning without line number.

```
AB
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

aifed uses xxHash64 with base32hex encoding for line identification.

### Format

```
42:AB
```

- `42` - Line number
- `AB` - 2-character hash (10 bits, base32hex encoded)

### Algorithm

- xxHash64 hashes the raw line content (whitespace preserved)
- Top 10 bits of the hash are extracted
- Encoded as 2 characters using base32hex (`0-9`, `A-V`)

### base32hex Character Set

```
0123456789ABCDEFGHIJKLMNOPQRSTUV
```

- Digits first to avoid 0/O, 1/I/L confusion
- 32 characters = 5 bits/char, 2 characters = 10 bits

## Hash Mismatch Behavior

When the provided hash doesn't match the current line content:

```
Error: Hash mismatch
  File: main.rs
  Expected hash: AB
  Actual hash: 3K
  Actual content: fn main() {
  Hint: Run 'aifed read main.rs' to get current hashes
```

### Resolution Options

1. **Re-read the file** - Get current hashes with `aifed read <FILE>`

## Format Summary

**Note:** File path is a separate command argument, not part of the locator. Examples below show full command-line context for clarity.

### Edit Locators (for `edit` command)

| Format        | Syntax                  | Locator Only    | Full Example            | Use Case            |
| ------------- | ----------------------- | --------------- | ----------------------- | ------------------- |
| Hashline      | `LINE:HASH`             | `42:AB`         | `main.rs 42:AB`         | Default, safest     |
| HashlineRange | `[START:HASH,END:HASH]` | `[10:AB,50:CD]` | `main.rs [10:AB,50:CD]` | Range delete        |
| Hash only     | `HASH`                  | `AB`            | `main.rs AB`            | Line number unknown |

**Virtual line** (`0:00`) is a special hashline value for inserting at file beginning.

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

Use `lsp symbols` command with a line number to get both the hashline and symbol locators:

```bash
aifed lsp symbols main.rs 15
```

Output:
```
15:3K|let config = load_config();
S1:config
S2:load_config
```

The output provides everything needed for LSP operations: `15:3K` (hashline) and `S1:config` (symbol locator).

### When to Use

| Locator Type   | Format        | Use Case                                               |
| -------------- | ------------- | ------------------------------------------------------ |
| Line Locator   | `LINE:HASH`   | Edit operations (`~`, `+`, `-`)                        |
| Symbol Locator | `SINDEX:NAME` | Symbol identification (used with Line Locator for LSP) |

For LSP operations, both locators are required: `LINE:HASH` identifies the line, `SINDEX:NAME` identifies the symbol on that line.

### Example Usage

```bash
# LSP operations require both hashline and symbol locator
aifed lsp rename main.rs 15:3K S1:config settings
aifed lsp hover main.rs 15:3K S2:load_config
aifed lsp def main.rs 15:3K S2:load_config
aifed lsp refs main.rs 15:3K S1:config
```

---

## Usage in Commands

Locators are used with the `edit` command:

```bash
# Replace with LINE:HASH format
aifed edit main.rs = 42:AB "new content"

# Insert after a line
aifed edit main.rs + 10:AB "new line"

# Insert at file beginning (virtual line)
aifed edit main.rs + 0:00 "// Copyright 2026"

# Delete a line
aifed edit main.rs - 42:AB

# Batch operations
aifed edit main.rs <<EOF
~ 42:AB "new content"
+ 10:3K "another line"
- 15:7M
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
1:AB|fn main() {
2:3K|    println!("hello");
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
