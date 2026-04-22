# Edit Commands

Commands for modifying file content.

## `edit` - Edit File Content

The unified command for all file edits: delete and insert. Replacement is expressed as delete plus insert.

### Usage

```
aifed edit <FILE> [--dry-run]   # Operations via stdin (heredoc)
```

All edit operations are provided via stdin using heredoc syntax.

### Operations

| Operator | Syntax                    | Description                                               |
| -------- | ------------------------- | --------------------------------------------------------- |
| `+`      | `+ <LOCATOR> <CONTENT...>` | Insert one or more lines after locator                    |
| `-`      | `- <LOCATOR>`             | Delete content at locator (supports range delete)         |

**Mnemonic:**
- `+` - Plus suggests "add" or "insert"
- `-` - Minus suggests "remove" or "delete"

### Replacement Pattern

Replacement is written as delete plus insert:

```bash
# Replace one line
aifed edit main.rs <<'EOF'
- 42:AB
+ 42:AB "fn main() {"
EOF
```

For range replacement, the documented canonical form is to reuse the deleted range end hashline:

```bash
aifed edit main.rs <<'EOF'
- [10:AB,14:CD]
+ 14:CD "new line 1" "new line 2"
EOF
```

Because edits are resolved atomically against the original file state, insertion after the line before the range or after a deleted line inside the range may also work. Documentation and examples intentionally standardize on `+ END:HASH ...` to keep the mental model simple for LLMs.

### Locator Format

Edit commands use **hashline** locators to specify positions with verification.

| Format                  | Example         | Description                                           |
| ----------------------- | --------------- | ----------------------------------------------------- |
| `LINE:HASH`             | `42:AB`         | Hashline - line + hash verification (recommended)     |
| `[START:HASH,END:HASH]` | `[10:AB,50:CD]` | Range delete - deletes lines START to END (inclusive) |

**Virtual line:** The special hashline `0:00` represents the position before the first line, used for inserting at the beginning of a file.

```bash
# Insert a copyright header at the very start of a file
aifed edit main.rs <<'EOF'
+ 0:00 "// Copyright 2026"
EOF
```

See [locator.md](locator.md) for detailed documentation on locators and hashline.

### String Escaping (JSON-style)

Content in double quotes supports JSON escape sequences:

| Escape   | Result             |
| -------- | ------------------ |
| `\"`     | `"` (double quote) |
| `\\`     | `\` (backslash)    |
| `\n`     | newline            |
| `\t`     | tab                |
| `\r`     | carriage return    |
| `\uXXXX` | Unicode character  |

**Rules:**
- `\n` is rejected. Each content payload is exactly one line.
- A single `+` can carry multiple quoted payloads, inserted in order.

**Example:**
```bash
# Double quotes inside content
aifed edit main.rs <<'EOF'
+ 42:AB "println!(\"hello\");"
EOF
# Result: println!("hello");

# Insert multiple lines with one locator
aifed edit main.rs <<'EOF'
+ 10:CD "{\"key\": \"value\"}" "{\"key\": \"value2\"}"
EOF
```

### Options

| Option      | Description                      |
| ----------- | -------------------------------- |
| `--dry-run` | Preview changes without applying |

### Examples

#### Single Operations

```bash
# Replace line 42 with hash verification
aifed edit main.rs <<'EOF'
- 42:AB
+ 42:AB "fn main() {"
EOF

# Insert after line 10
aifed edit main.rs <<'EOF'
+ 10:AB "    println!(\"hello\");"
EOF

# Insert multiple lines after line 10
aifed edit main.rs <<'EOF'
+ 10:AB "    println!(\"hello\");" "    println!(\"world\");"
EOF

# Delete line 42
aifed edit main.rs <<'EOF'
- 42:AB
EOF

# Insert at file beginning
aifed edit main.rs <<'EOF'
+ 0:00 "// Copyright 2026"
EOF
```

#### Batch Operations

```bash
# Multiple operations in one heredoc
aifed edit main.rs <<'EOF'
- [42:AB,42:AB]
+ 42:AB "fn main() {"
+ 10:3K "    println!(\"hello\");" "    println!(\"world\");"
- 15:7M
EOF
```

#### Range Delete and Range Replacement

```bash
# Delete lines 10-50 (inclusive), with boundary hash verification
aifed edit main.rs <<'EOF'
- [10:AB,50:CD]
EOF

# Range replacement using the canonical end-anchor form
aifed edit main.rs <<'EOF'
- [2:AA,89:BB]
+ 89:BB "new header" "second header line"
EOF
```

#### JSON Escaping

```bash
# Content with embedded quotes
aifed edit main.rs <<'EOF'
- 42:AB
+ 42:AB "code: println!(\"result: {}\", value);"
EOF

# JSON content
aifed edit config.json <<'EOF'
+ 10:CD "{\"name\": \"test\", \"value\": 123}"
EOF
```

#### With Options

```bash
# Preview changes
aifed edit main.rs --dry-run <<'EOF'
- 42:AB
+ 42:AB "fn main() {"
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
  - 1:AB
  + 1:AB "aa"
  - 3:7M
  + 3:7M "cc"
```

Hashes are content-based, so they remain valid regardless of other edits.

### Failure Handling

Batch operations are **atomic** (all-or-nothing): if any operation fails, none are applied.

## See Also

- [Locator Reference](locator.md) - Detailed locator documentation
- [Read Commands](read-commands.md) - Getting hashes with info/read
- [History & Recovery](history.md) - Undoing edits
