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

**Exit code:** 4

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

For LSP operations that require column-level precision (rename, hover, definition, references), use Symbol Locator instead of numeric columns.

### Why Symbol Locator?

LLMs cannot reliably count character positions. Numeric columns are error-prone:
- AI must count to the Nth character
- No verification mechanism
- Position changes with code modifications

Symbol Locator solves this by using semantic information from LSP.

### Format

| Format                 | Locator Only | Full Example         | Description         |
| ---------------------- | ------------ | -------------------- | ------------------- |
| `S<INDEX>:<NAME>`      | `S1:user`    | `main.rs S1:user`    | Symbol index + name |
| `LINE:S<INDEX>:<NAME>` | `15:S1:user` | `main.rs 15:S1:user` | With line context   |

- `INDEX` - Sequential number (1-based) for symbols on the line
- `NAME` - Symbol name for self-documentation and LSP verification

**No hash needed** - LSP validates symbol existence, so hash verification is redundant.

### Getting Symbol Locators

Use `read --symbols` to get symbol locators for a specific line:

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

Note: `S1:user` (variable, type `User`) and `S4:user` (parameter, e.g., type `&str`) are different symbols - renaming one does not affect the other. INDEX distinguishes position, LSP distinguishes identity.

### When to Use

| Locator Type   | Format        | Use Case                                               |
| -------------- | ------------- | ------------------------------------------------------ |
| Line Locator   | `LINE:HASH`   | Edit operations (`~`, `+`, `-`)                        |
| Symbol Locator | `SINDEX:NAME` | LSP operations (rename, hover, definition, references) |

### Example Usage

```bash
# LSP operations with Symbol Locator
aifed rename main.rs S1:user new_name
aifed hover main.rs S1:user
aifed definition main.rs S2:User
aifed references main.rs S4:user
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
L1:abc123  fn main() {
L2:def456      println!("hello");
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
