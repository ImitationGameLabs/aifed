# Configuration

Configuration commands and file format for aifed.

## `config` - Manage Configuration

View or modify aifed configuration.

### Usage

```
aifed config list
aifed config get <KEY>
aifed config set <KEY> <VALUE>
aifed config init
```

### Subcommands

| Subcommand          | Description                        |
| ------------------- | ---------------------------------- |
| `list`              | List all configuration values      |
| `get <KEY>`         | Get a specific configuration value |
| `set <KEY> <VALUE>` | Set a configuration value          |
| `init`              | Initialize configuration file      |

### Examples

```bash
# Initialize config file
aifed config init

# View current config
aifed config list

# Get specific value
aifed config get edit.auto_format

# Set auto-format
aifed config set edit.auto_format true

# JSON output
aifed config list --json
```

---

## `init` - Initialize Project

Initialize aifed for the current project.

### Usage

```
aifed init [PATH]
```

### Options

| Option    | Description                      |
| --------- | -------------------------------- |
| `--force` | Overwrite existing configuration |

### Examples

```bash
# Initialize in current directory
aifed init

# Initialize in specific directory
aifed init ./myproject

# Overwrite existing config
aifed init --force
```

---

## Configuration File

Default location: `.aifed.toml` in project root, or `~/.config/aifed/config.toml` globally.

### Full Configuration Example

```toml
[edit]
auto_format = true
hash_enabled = true

[format]
# Per-language formatters
rust = "rustfmt"
go = "gofmt"
javascript = "prettier --stdin-filepath"

[lsp]
# Per-language LSP servers
rust = "rust-analyzer"
go = "gopls"
typescript = "typescript-language-server --stdio"

[history]
enabled = true
max_entries = 100

[snapshot]
dir = ".aifed/snapshots"
max_snapshots = 10
max_age = "7d"
```

### Configuration Sections

#### `[edit]`

| Key            | Type | Default | Description              |
| -------------- | ---- | ------- | ------------------------ |
| `auto_format`  | bool | `false` | Auto-format after edits  |
| `hash_enabled` | bool | `true`  | Enable hash verification |

#### `[format]`

Per-language formatter configuration.

```toml
[format]
rust = "rustfmt"
go = "gofmt"
javascript = "prettier --stdin-filepath"
python = "black -"
```

#### `[lsp]`

Per-language LSP server configuration.

```toml
[lsp]
rust = "rust-analyzer"
go = "gopls"
typescript = "typescript-language-server --stdio"
python = "pylsp"
```

#### `[history]`

| Key           | Type   | Default | Description                      |
| ------------- | ------ | ------- | -------------------------------- |
| `enabled`     | bool   | `true`  | Enable edit history              |
| `max_entries` | number | `100`   | Maximum history entries per file |

#### `[snapshot]`

| Key             | Type   | Default            | Description                |
| --------------- | ------ | ------------------ | -------------------------- |
| `dir`           | string | `.aifed/snapshots` | Snapshot storage directory |
| `max_snapshots` | number | `10`               | Maximum snapshots per file |
| `max_age`       | string | `7d`               | Maximum snapshot age       |

---

## Configuration Layers

Configuration is loaded in layers, with later layers overriding earlier ones:

| Priority    | Layer                 | Location                      |
| ----------- | --------------------- | ----------------------------- |
| 1 (lowest)  | Built-in defaults     | -                             |
| 2           | Global config         | `~/.config/aifed/config.toml` |
| 3           | Project config        | `.aifed.toml`                 |
| 4           | Environment variables | `AIFED_*`                     |
| 5 (highest) | CLI flags             | `--option`                    |

### Example Resolution

```bash
# Built-in default
edit.auto_format = false

# Global config (~/.config/aifed/config.toml)
edit.auto_format = true

# Project config (.aifed.toml)
# (not set)

# Environment variable
AIFED_AUTO_FORMAT=false

# CLI flag
--auto-format
```

Result: `--auto-format` wins (highest priority)

---

## Environment Variables

| Variable            | Equivalent   |
| ------------------- | ------------ |
| `AIFED_CONFIG`      | `--config`   |
| `AIFED_NO_COLOR`    | `--no-color` |
| `AIFED_JSON`        | `--json`     |
| `AIFED_QUIET`       | `--quiet`    |
| `AIFED_AUTO_FORMAT` | `--auto-fmt` |

---

## Per-Language Settings

### Formatter Configuration

Specify the formatter command for each language:

```toml
[format]
# Simple command
rust = "rustfmt"

# Command with flags
go = "gofmt -s"

# Using stdin
javascript = "prettier --stdin-filepath $FILE"
```

### LSP Configuration

Specify the LSP server for each language:

```toml
[lsp]
rust = "rust-analyzer"
go = "gopls"
typescript = "typescript-language-server --stdio"
python = "pylsp"
```

### Language Detection

aifed detects language using a cascading approach:

1. File extension (fastest)
2. shebang for scripts
3. LSP detection (fallback)

## See Also

- [CLI Overview](cli-overview.md) - Environment variables and exit codes
- [LSP Integration](lsp.md) - LSP server configuration
- [Utilities](utilities.md) - Formatter usage
