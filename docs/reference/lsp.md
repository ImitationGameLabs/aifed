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

## `symbols` - List Symbols in File

List all symbols (functions, types, variables) in a file.

### Usage

```
aifed symbols <FILE>
```

### Options

| Option              | Description                               |
| ------------------- | ----------------------------------------- |
| `--type <TYPE>`     | Filter by symbol type (language-specific) |
| `--list-types`      | List available symbol types for the file  |
| `--include-private` | Include private symbols                   |

Symbol types depend on the LSP server and language. Use `--list-types` to discover available types.

### Output Format (Text)

```
L10: fn main()
L25: fn process(data: &[u8]) -> Result<()>
L40: struct Config
L55: static DEFAULT_CONFIG: Config
```

### Examples

```bash
# List all symbols
aifed symbols main.rs

# List available symbol types
aifed symbols main.rs --list-types

# Filter by type
aifed symbols main.rs --type func

# Include private symbols
aifed symbols main.rs --include-private

# JSON output
aifed symbols main.rs --json
```

---

## `rename` - Rename Symbol

Rename a symbol across all references using LSP.

### Usage

```
aifed rename <FILE> <SYMBOL_LOCATOR> <NEW_NAME>
aifed rename <FILE> --symbol <NAME> <NEW_NAME>
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

### Position Format

Use Symbol Locator (AI-friendly, no counting required):

| Format             | Example      | Description                  |
| ------------------ | ------------ | ---------------------------- |
| `SINDEX:NAME`      | `S1:user`    | Symbol locator (recommended) |
| `LINE:SINDEX:NAME` | `15:S1:user` | With line context            |

Get symbol locators with: `aifed read <FILE> <LINE> --symbols`

### Examples

```bash
# Rename by Symbol Locator
aifed rename main.rs S1:user new_name

# Rename by symbol name
aifed rename main.rs --symbol oldFunc newFunc

# Preview changes
aifed rename main.rs S1:user new_name --dry-run

# Rename in file only
aifed rename main.rs S1:user new_name --scope file
```

### Conflict Handling

If the new name conflicts with an existing symbol, the command fails with an error. This prevents silent conflicts.

---

## `references` - Find References

Find all references to a symbol.

### Usage

```
aifed references <FILE> <SYMBOL_LOCATOR>
aifed references <FILE> --symbol <NAME>
```

### Options

| Option                  | Description                                     |
| ----------------------- | ----------------------------------------------- |
| `--include-declaration` | Include symbol declaration in results           |
| `--scope <SCOPE>`       | Scope: `file`, `workspace` [default: workspace] |

### Position Format

Use Symbol Locator (AI-friendly, no counting required):

| Format             | Example      | Description                  |
| ------------------ | ------------ | ---------------------------- |
| `SINDEX:NAME`      | `S1:user`    | Symbol locator (recommended) |
| `LINE:SINDEX:NAME` | `15:S1:user` | With line context            |

Get symbol locators with: `aifed read <FILE> <LINE> --symbols`

### Examples

```bash
# Find references by Symbol Locator
aifed references main.rs S4:user

# Find references by symbol name
aifed references main.rs --symbol processConfig

# Include declaration
aifed references main.rs S4:user --include-declaration

# JSON output
aifed references main.rs S4:user --json
```

---

## `definition` - Go to Definition

Find the definition of a symbol.

### Usage

```
aifed definition <FILE> <SYMBOL_LOCATOR>
```

### Position Format

Use Symbol Locator (AI-friendly, no counting required):

| Format             | Example      | Description                  |
| ------------------ | ------------ | ---------------------------- |
| `SINDEX:NAME`      | `S1:user`    | Symbol locator (recommended) |
| `LINE:SINDEX:NAME` | `15:S1:user` | With line context            |

Get symbol locators with: `aifed read <FILE> <LINE> --symbols`

### Examples

```bash
# Go to definition
aifed definition main.rs S2:User

# JSON output
aifed definition main.rs S2:User --json
```

---

## `hover` - Get Hover Information

Get type information and documentation for a symbol.

### Usage

```
aifed hover <FILE> <SYMBOL_LOCATOR>
```

### Position Format

Use Symbol Locator (AI-friendly, no counting required):

| Format             | Example      | Description                  |
| ------------------ | ------------ | ---------------------------- |
| `SINDEX:NAME`      | `S1:user`    | Symbol locator (recommended) |
| `LINE:SINDEX:NAME` | `15:S1:user` | With line context            |

Get symbol locators with: `aifed read <FILE> <LINE> --symbols`

### Examples

```bash
# Get hover info
aifed hover main.rs S1:user

# JSON output
aifed hover main.rs S1:user --json
```

## See Also

- [Configuration](configuration.md) - Configuring LSP servers
- [Read Commands](read-commands.md) - Getting Symbol Locators
- [CLI Design Notes](../cli-design-notes.md) - LSP design rationale
