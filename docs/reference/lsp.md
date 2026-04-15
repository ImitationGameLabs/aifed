# LSP Integration

Commands for Language Server Protocol integration.

## Overview

aifed integrates with LSP servers to provide code intelligence features:

- Diagnostics (errors, warnings)
- Symbol navigation
- Rename refactoring
- Find references
- Go to definition
- Hover information
- Completions

All LSP commands are under the `lsp` subcommand and require a running daemon.

### Supported Languages

LSP servers are configured per-language in `aifed.toml`:

```toml
[lsp]
go = "gopls"
rust = "rust-analyzer"
typescript = "typescript-language-server --stdio"
```

---

## Command Overview

| Command        | Shorthand | Description                     |
| -------------- | --------- | ------------------------------- |
| `lsp symbols`  | -         | Get symbol locators for a line  |
| `lsp diag`     | -         | Get diagnostics for file        |
| `lsp hover`    | -         | Get hover information at symbol |
| `lsp def`      | -         | Go to definition                |
| `lsp refs`     | -         | Find references                 |
| `lsp complete` | -         | Get completions at symbol       |
| `lsp rename`   | -         | Rename symbol                   |

### Common Option

All LSP commands support `--json` for JSON output.

---

## `lsp symbols` - Get Symbol Locators

Get Symbol Locators for specific lines. Used to prepare for LSP operations (rename, hover, definition, references).

### Usage

```
aifed lsp symbols <FILE> <LINE|RANGE>
```

### Arguments

| Argument  | Example   | Description                  |
| --------- | --------- | ---------------------------- |
| `<FILE>`  | `main.rs` | File path                    |
| `<LINE>`  | `15`      | Get symbols on specific line |
| `<RANGE>` | `10-20`   | Get symbols on line range    |

### Output Format (Text)

```
15:3K|let config = load_config();
S1:config
S2:load_config
```

Note: Symbol Locator index (`S1`, `S2`, etc.) is per-line, starting from 1. For line ranges, each line shows its symbols separately.

### Examples

```bash
# Get Symbol Locators for a specific line
aifed lsp symbols main.rs 15

# Get Symbol Locators for a line range
aifed lsp symbols main.rs 10-20

# JSON output
aifed lsp symbols main.rs 15 --json
```

### When to Use

- **LSP operations** - Get Symbol Locators for rename, hover, definition, references
- **Disambiguation** - When same symbol name appears multiple times on a line

**Note:** For file overview, use `read` command instead.

---

## `lsp diag` - Get Diagnostics

Get diagnostics (errors, warnings) for a file.

### Usage

```
aifed lsp diag <FILE>
```

### Arguments

| Argument | Description |
| -------- | ----------- |
| `<FILE>` | File path   |

### Examples

```bash
# Get diagnostics for a file
aifed lsp diag main.rs

# JSON output
aifed lsp diag main.rs --json
```

---

## `lsp hover` - Get Hover Information

Get type information and documentation for a symbol.

### Usage

```
aifed lsp hover <FILE> <HASHLINE> <SYMBOL>
```

### Arguments

| Argument     | Description                        |
| ------------ | ---------------------------------- |
| `<FILE>`     | File path                          |
| `<HASHLINE>` | Hashline locator (e.g., `15:3K`)   |
| `<SYMBOL>`   | Symbol locator (e.g., `S1:config`) |

The hashline ensures the line content hasn't drifted. Symbol locator index is per-line (1-based).

Get both values with: `aifed lsp symbols <FILE> <LINE>`

### Examples

```bash
# Get hover info (hashline from symbols output)
aifed lsp hover main.rs 15:3K S1:config

# JSON output
aifed lsp hover main.rs 15:3K S1:config --json
```

---

## `lsp def` - Go to Definition

Find the definition of a symbol.

### Usage

```
aifed lsp def <FILE> <HASHLINE> <SYMBOL>
```

### Arguments

| Argument     | Description                             |
| ------------ | --------------------------------------- |
| `<FILE>`     | File path                               |
| `<HASHLINE>` | Hashline locator (e.g., `15:3K`)        |
| `<SYMBOL>`   | Symbol locator (e.g., `S2:load_config`) |

The hashline ensures the line content hasn't drifted. Symbol locator index is per-line (1-based).

Get both values with: `aifed lsp symbols <FILE> <LINE>`

### Examples

```bash
# Go to definition (hashline from symbols output)
aifed lsp def main.rs 15:3K S2:load_config

# JSON output
aifed lsp def main.rs 15:3K S2:load_config --json
```

---

## `lsp refs` - Find References

Find all references to a symbol.

### Usage

```
aifed lsp refs <FILE> <HASHLINE> <SYMBOL>
```

### Arguments

| Argument     | Description                        |
| ------------ | ---------------------------------- |
| `<FILE>`     | File path                          |
| `<HASHLINE>` | Hashline locator (e.g., `15:3K`)   |
| `<SYMBOL>`   | Symbol locator (e.g., `S1:config`) |

The hashline ensures the line content hasn't drifted. Symbol locator index is per-line (1-based).

Get both values with: `aifed lsp symbols <FILE> <LINE>`

### Examples

```bash
# Find references (hashline from symbols output)
aifed lsp refs main.rs 15:3K S1:config

# JSON output
aifed lsp refs main.rs 15:3K S1:config --json
```

---

## `lsp complete` - Get Completions

Get completion suggestions at a symbol position.

### Usage

```
aifed lsp complete <FILE> <HASHLINE> <SYMBOL>
```

### Arguments

| Argument     | Description                        |
| ------------ | ---------------------------------- |
| `<FILE>`     | File path                          |
| `<HASHLINE>` | Hashline locator (e.g., `15:3K`)   |
| `<SYMBOL>`   | Symbol locator (e.g., `S1:config`) |

### Examples

```bash
# Get completions (hashline from symbols output)
aifed lsp complete main.rs 15:3K S1:config

# JSON output
aifed lsp complete main.rs 15:3K S1:config --json
```

---

## `lsp rename` - Rename Symbol

Rename a symbol across all references using LSP.

### Usage

```
aifed lsp rename <FILE> <HASHLINE> <SYMBOL> <NEW_NAME> [--dry-run]
```

### Arguments

| Argument     | Description                        |
| ------------ | ---------------------------------- |
| `<FILE>`     | File path                          |
| `<HASHLINE>` | Hashline locator (e.g., `15:3K`)   |
| `<SYMBOL>`   | Symbol locator (e.g., `S1:config`) |
| `<NEW_NAME>` | New symbol name                    |

The hashline ensures the line content hasn't drifted. Symbol locator index is per-line (1-based).

Get both values with: `aifed lsp symbols <FILE> <LINE>`

### Output

- Text output shows a summary followed by one diff section per changed file.
- Each file section uses a single file header plus contextual `-old/+new` hunk lines.
- Paths are shown relative to the workspace root when possible, with absolute fallback.
- `--dry-run` previews the rename without writing files.
- `--json` keeps the raw structured response from the daemon.

### Examples

```bash
# Rename symbol (hashline from symbols output)
aifed lsp rename main.rs 15:3K S1:config settings

# Preview the rename diff without applying it
aifed lsp rename main.rs 15:3K S1:config settings --dry-run

# JSON output
aifed lsp rename main.rs 15:3K S1:config settings --json
```

### Conflict Handling

If the new name conflicts with an existing symbol, the command fails with an error. This prevents silent conflicts.

---

## See Also

- [Configuration](configuration.md) - Configuring LSP servers
- [Read Commands](read-commands.md) - Getting Symbol Locators
- [Locator Format](locator.md) - Understanding hashlines and symbol locators
- [CLI Design Notes](../cli-design-notes.md) - LSP design rationale
