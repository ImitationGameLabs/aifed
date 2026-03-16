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
- Import organization

### Supported Languages

LSP servers are configured per-language in `.aifed.toml`:

```toml
[lsp]
go = "gopls"
rust = "rust-analyzer"
typescript = "typescript-language-server --stdio"
```

---

## `diagnostics` - Get LSP Diagnostics

Get diagnostics (errors, warnings) for a file or workspace.

### Usage

```
aifed diagnostics [PATH]
```

### Options

| Option               | Description                                            |
| -------------------- | ------------------------------------------------------ |
| `--all`              | Include all diagnostics (not just errors)              |
| `--severity <LEVEL>` | Filter by severity: `error`, `warning`, `hint`, `info` |
| `--watch`            | Watch for diagnostic changes                           |

### Severity Levels

| Level     | Description                          |
| --------- | ------------------------------------ |
| `error`   | Must fix - compilation/runtime error |
| `warning` | Should fix - potential issue         |
| `hint`    | Suggestion - code improvement        |
| `info`    | Information only                     |

### Default Behavior

By default, shows only errors and warnings (not hints or info).

### Examples

```bash
# Get errors for current file
aifed diagnostics main.rs

# Get all diagnostics in workspace
aifed diagnostics --all

# Filter by severity
aifed diagnostics --severity error

# JSON output
aifed diagnostics --json

# Watch for changes
aifed diagnostics --watch
```

---

## `symbols` - Get Symbol Locators

Get Symbol Locators for specific lines. Used to prepare for LSP operations (rename, hover, definition, references).

### Usage

```
aifed symbols <FILE> <LINE|START-END>
```

### Options

| Option               | Description                                   |
| -------------------- | --------------------------------------------- |
| `--include-keywords` | Include language keywords (e.g., `let`, `fn`) |
| `--type <TYPE>`      | Filter by symbol type (language-specific)     |
| `--list-types`       | List available symbol types for the file      |
| `--include-private`  | Include private symbols                       |

### Position Arguments

| Argument    | Example         | Description                  |
| ----------- | --------------- | ---------------------------- |
| `LINE`      | `main.rs 15`    | Get symbols on specific line |
| `START-END` | `main.rs 10-20` | Get symbols on line range    |

Symbol types depend on the LSP server and language. Use `--list-types` to discover available types.
### Output Format (Text)

Default (identifiers only, excludes keywords):

```
15:3K|let config = load_config();
S1:config
S2:load_config
```

With `--include-keywords`:

```
15:3K|let config = load_config();
S1:let
S2:config
S3:load_config
```

Note: Symbol Locator index (`S1`, `S2`, etc.) is per-line, starting from 1. For line ranges, each line shows its symbols separately.
### Examples

```bash
# Get Symbol Locators for a specific line (default: identifiers only)
aifed symbols main.rs 15

# Include keywords for hover operations
aifed symbols main.rs 15 --include-keywords

# Get Symbol Locators for a line range
aifed symbols main.rs 10-20

# Filter by type
aifed symbols main.rs 15 --type func

# Include private symbols
aifed symbols main.rs 15 --include-private

# JSON output
aifed symbols main.rs 15 --json
```

### When to Use

- **LSP operations** - Get Symbol Locators for rename, hover, definition, references
- **Disambiguation** - When same symbol name appears multiple times on a line

**Note:** For file overview, use `read` command instead.
## `rename` - Rename Symbol

Rename a symbol across all references using LSP.

### Usage

```
aifed rename <FILE> <LINE:HASH> <SINDEX:NAME> <NEW_NAME>
```

### Options

| Option            | Description                                     |
| ----------------- | ----------------------------------------------- |
| `--dry-run`       | Preview changes without applying                |
| `--scope <SCOPE>` | Scope: `file`, `workspace` [default: workspace] |

### Scope Options

| Scope       | Description                       |
| ----------- | --------------------------------- |
| `file`      | Rename only in current file       |
| `workspace` | Rename across all files (default) |

### Arguments

| Argument        | Description                                     |
| --------------- | ----------------------------------------------- |
| `<FILE>`        | File path                                       |
| `<LINE:HASH>`   | Hashline locator for the line (e.g., `15:3K`)   |
| `<SINDEX:NAME>` | Symbol locator on that line (e.g., `S1:config`) |
| `<NEW_NAME>`    | New symbol name                                 |

The hashline ensures the line content hasn't drifted. Symbol locator index is per-line (1-based).

Get both values with: `aifed symbols <FILE> <LINE>`

### Examples

```bash
# Rename symbol (hashline from symbols output)
aifed rename main.rs 15:3K S1:config settings

# Preview changes
aifed rename main.rs 15:3K S1:config settings --dry-run

# Rename in file only
aifed rename main.rs 15:3K S1:config settings --scope file
```

### Conflict Handling

If the new name conflicts with an existing symbol, the command fails with an error. This prevents silent conflicts.

---

## `references` - Find References

Find all references to a symbol.

### Usage

```
aifed references <FILE> <LINE:HASH> <SINDEX:NAME>
```

### Options

| Option                  | Description                                     |
| ----------------------- | ----------------------------------------------- |
| `--include-declaration` | Include symbol declaration in results           |
| `--scope <SCOPE>`       | Scope: `file`, `workspace` [default: workspace] |

### Arguments

| Argument        | Description                                     |
| --------------- | ----------------------------------------------- |
| `<FILE>`        | File path                                       |
| `<LINE:HASH>`   | Hashline locator for the line (e.g., `15:3K`)   |
| `<SINDEX:NAME>` | Symbol locator on that line (e.g., `S1:config`) |

The hashline ensures the line content hasn't drifted. Symbol locator index is per-line (1-based).

Get both values with: `aifed symbols <FILE> <LINE>`

### Examples

```bash
# Find references (hashline from symbols output)
aifed references main.rs 15:3K S1:config

# Include declaration
aifed references main.rs 15:3K S1:config --include-declaration

# JSON output
aifed references main.rs 15:3K S1:config --json
```

---

## `definition` - Go to Definition

Find the definition of a symbol.

### Usage

```
aifed definition <FILE> <LINE:HASH> <SINDEX:NAME>
```

### Arguments

| Argument        | Description                                          |
| --------------- | ---------------------------------------------------- |
| `<FILE>`        | File path                                            |
| `<LINE:HASH>`   | Hashline locator for the line (e.g., `15:3K`)        |
| `<SINDEX:NAME>` | Symbol locator on that line (e.g., `S2:load_config`) |

The hashline ensures the line content hasn't drifted. Symbol locator index is per-line (1-based).

Get both values with: `aifed symbols <FILE> <LINE>`

### Examples

```bash
# Go to definition (hashline from symbols output)
aifed definition main.rs 15:3K S2:load_config

# JSON output
aifed definition main.rs 15:3K S2:load_config --json
```

---

## `hover` - Get Hover Information

Get type information and documentation for a symbol.

### Usage

```
aifed hover <FILE> <LINE:HASH> <SINDEX:NAME>
```

### Arguments

| Argument        | Description                                     |
| --------------- | ----------------------------------------------- |
| `<FILE>`        | File path                                       |
| `<LINE:HASH>`   | Hashline locator for the line (e.g., `15:3K`)   |
| `<SINDEX:NAME>` | Symbol locator on that line (e.g., `S1:config`) |

The hashline ensures the line content hasn't drifted. Symbol locator index is per-line (1-based).

Get both values with: `aifed symbols <FILE> <LINE>`

### Examples

```bash
# Get hover info (hashline from symbols output)
aifed hover main.rs 15:3K S1:config

# JSON output
aifed hover main.rs 15:3K S1:config --json
```

## See Also

- [Configuration](configuration.md) - Configuring LSP servers
- [Read Commands](read-commands.md) - Getting Symbol Locators
- [CLI Design Notes](../cli-design-notes.md) - LSP design rationale
