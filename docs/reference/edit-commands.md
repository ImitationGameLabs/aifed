# Edit Commands

Commands for modifying file content.

## `edit` - Edit File Content

The unified command for all file edits: replace, insert, and delete.

### Usage

```
aifed edit <FILE> [--dry-run]   # Operations via stdin (heredoc)
```

All edit operations are provided via stdin using heredoc syntax.

### Operations

| Operator | Syntax                  | Description                                       |
| -------- | ----------------------- | ------------------------------------------------- |
| `=`      | `= <LOCATOR> <CONTENT>` | Replace content at locator                        |
| `+`      | `+ <LOCATOR> <CONTENT>` | Insert content after locator                      |
| `-`      | `- <LOCATOR>`           | Delete content at locator (supports range delete) |

**Mnemonic:**
- `=` - Equals suggests "assignment" (replace X with Y)
- `+` - Plus suggests "add" or "insert"
- `-` - Minus suggests "remove" or "delete"

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

**Example:**
```bash
# Double quotes inside content
aifed edit main.rs <<'EOF'
= 42:AB "println!(\"hello\");"
EOF
# Result: println!("hello");

# JSON string as content
aifed edit config.rs <<'EOF'
+ 10:CD "{\"key\": \"value\"}"
EOF
# Result: {"key": "value"}
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
= 42:AB "fn main() {"
EOF

# Insert after line 10
aifed edit main.rs <<'EOF'
+ 10:AB "    println!(\"hello\");"
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
= 42:AB "fn main() {"
+ 10:3K "    println!(\"hello\");"
- 15:7M
EOF
```

#### Range Delete

```bash
# Delete lines 10-50 (inclusive), with boundary hash verification
aifed edit main.rs <<'EOF'
- [10:AB,50:CD]
EOF

# Range delete combined with other operations
aifed edit main.rs <<'EOF'
- [2:AA,89:BB]
+ 1:HH "new header"
EOF
```

#### JSON Escaping

```bash
# Content with embedded quotes
aifed edit main.rs <<'EOF'
= 42:AB "code: println!(\"result: {}\", value);"
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
= 42:AB "fn main() {"
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
  = AB "aa"    # Hash-based, valid
  - 3K         # Hash-based, valid
  = 7M "cc"    # Hash-based, valid
```

Hashes are content-based, so they remain valid regardless of other edits.

### Failure Handling

Batch operations are **atomic** (all-or-nothing): if any operation fails, none are applied.

## See Also

- [Locator Reference](locator.md) - Detailed locator documentation
- [Read Commands](read-commands.md) - Getting hashes with info/read
- [History & Recovery](history.md) - Undoing edits
