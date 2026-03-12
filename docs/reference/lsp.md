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
aifed diagnostics main.go

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

| Option              | Description                                           |
| ------------------- | ----------------------------------------------------- |
| `--type <TYPE>`     | Filter by symbol type: `func`, `type`, `var`, `const` |
| `--include-private` | Include private symbols                               |

### Symbol Types

| Type     | Description                  |
| -------- | ---------------------------- |
| `func`   | Functions and methods        |
| `type`   | Classes, interfaces, structs |
| `var`    | Variables                    |
| `const`  | Constants                    |
| `module` | Modules and namespaces       |

### Output Format (Text)

```
L10: func main()
L25: func process(data []byte) error
L40: type Config struct
L55: var defaultConfig Config
```

### Examples

```bash
# List all symbols
aifed symbols main.go

# Filter by type
aifed symbols main.go --type func

# Include private symbols
aifed symbols main.go --include-private

# JSON output
aifed symbols main.go --json
```

---

## `rename` - Rename Symbol

Rename a symbol across all references using LSP.

### Usage

```
aifed rename <FILE>:<LINE>:<COL> <NEW_NAME>
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

Use `LINE:COL` for symbol position (1-based).

### Examples

```bash
# Rename by position
aifed rename main.go:15:10 newName

# Rename by symbol name
aifed rename main.go --symbol oldFunc newFunc

# Preview changes
aifed rename main.go:15:10 newName --dry-run

# Rename in file only
aifed rename main.go:15:10 newName --scope file
```

### Conflict Handling

If the new name conflicts with an existing symbol, the command fails with an error. This prevents silent conflicts.

---

## `references` - Find References

Find all references to a symbol.

### Usage

```
aifed references <FILE>:<LINE>:<COL>
aifed references <FILE> --symbol <NAME>
```

### Options

| Option                  | Description                                     |
| ----------------------- | ----------------------------------------------- |
| `--include-declaration` | Include symbol declaration in results           |
| `--scope <SCOPE>`       | Scope: `file`, `workspace` [default: workspace] |

### Examples

```bash
# Find references by position
aifed references main.go:15:10

# Find references by symbol name
aifed references main.go --symbol processConfig

# Include declaration
aifed references main.go:15:10 --include-declaration

# JSON output
aifed references main.go:15:10 --json
```

---

## `definition` - Go to Definition

Find the definition of a symbol.

### Usage

```
aifed definition <FILE>:<LINE>:<COL>
```

### Position Format

Use `LINE:COL` for symbol position (1-based).

### Examples

```bash
# Go to definition
aifed definition main.go:25:15

# JSON output
aifed definition main.go:25:15 --json
```

---

## `hover` - Get Hover Information

Get type information and documentation for a symbol.

### Usage

```
aifed hover <FILE>:<LINE>:<COL>
```

### Position Format

Use `LINE:COL` for symbol position (1-based).

### Examples

```bash
# Get hover info
aifed hover main.go:15:10

# JSON output
aifed hover main.go:15:10 --json
```

---

## `organize-imports` - Organize Imports

Organize and clean up imports in a file.

### Usage

```
aifed organize-imports <FILE>
```

### Options

| Option      | Description                      |
| ----------- | -------------------------------- |
| `--dry-run` | Preview changes without applying |

### Examples

```bash
# Organize imports
aifed organize-imports main.go

# Preview changes
aifed organize-imports main.go --dry-run
```

## LSP Performance

### Startup Time

LSP servers can take 1-3 seconds to start. aifed uses:

- **Lazy initialization** - Start on first LSP command
- **Keep-alive** - Keep running for the session
- **Configurable timeout** - Adjust via configuration

### Large Files

For files > 10,000 lines:
- Diagnostics may be slower
- Consider using `--scope file` for targeted operations

## See Also

- [Configuration](configuration.md) - Configuring LSP servers
- [File Operations](file-operations.md) - Getting symbols via info command
- [CLI Design Notes](../cli-design-notes.md) - LSP design rationale
