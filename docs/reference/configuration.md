# Configuration

File-based runtime configuration for LSP language detection and server startup.

## Current Scope

The current implementation supports:

- built-in LSP defaults
- user config: `~/.config/aifed/config.toml`
- project config: `aifed.toml`
- runtime merging for LSP language definitions

The current implementation does **not** yet support:

- `aifed config ...` management commands
- `aifed init`
- formatter/history/edit configuration
- environment-variable or CLI overrides for config values

For now, edit the TOML files directly.

---

## Configuration Layers

Configuration is loaded in this order, with later layers replacing earlier entries that use the same `language` value:

| Priority    | Layer             | Location                      |
| ----------- | ----------------- | ----------------------------- |
| 1 (lowest)  | Built-in defaults | -                             |
| 2           | Global config     | `~/.config/aifed/config.toml` |
| 3 (highest) | Project config    | `aifed.toml`                  |

Today, the built-in default is Rust + `rust-analyzer`.

---

## File Format

Use one `[[lsp]]` object per language server definition.

```toml
[[lsp]]
language = "rust"
file_extensions = ["rs"]
root_markers = ["Cargo.toml"]
command = "rust-analyzer"
args = []
display_name = "rust-analyzer"
initialization_options = { checkOnSave = { command = "clippy" }, cargo = { allFeatures = true } }
```

### Fields

| Field                    | Type           | Required | Description |
| ------------------------ | -------------- | -------- | ----------- |
| `language`               | string         | yes      | Language id used by aifed and LSP requests |
| `file_extensions`        | string array   | yes      | Extensions used by CLI LSP commands to map files to a language |
| `root_markers`           | string array   | no       | Workspace-root files that trigger daemon auto-start for the language |
| `command`                | string         | yes      | Executable used to launch the language server |
| `args`                   | string array   | no       | Extra arguments passed to the server process |
| `display_name`           | string         | no       | Human-readable server name for logs and status output |
| `initialization_options` | inline table / TOML value | no | JSON-like initialization options passed during LSP initialize |

### Validation Rules

- `language` must be unique within a single file.
- `command` must not be empty.
- Unknown fields are rejected.
- Later config layers replace earlier entries for the same `language`.

---

## Common Examples

### Override the built-in Rust server

Use this when you want to change command, args, root markers, or extension mapping for Rust.

```toml
[[lsp]]
language = "rust"
file_extensions = ["rs"]
root_markers = ["Cargo.toml", "rust-project.json"]
command = "rust-analyzer"
args = ["--stdio"]
display_name = "rust-analyzer"
initialization_options = { checkOnSave = { command = "check" } }
```

### Add a custom language server

This is the main workaround path for languages not supported by built-in defaults.

```toml
[[lsp]]
language = "nix"
file_extensions = ["nix"]
root_markers = ["flake.nix", "default.nix", "shell.nix"]
command = "nil"
args = []
display_name = "nil"
```

### Global defaults + project override

Global config:

```toml
# ~/.config/aifed/config.toml
[[lsp]]
language = "rust"
file_extensions = ["rs"]
root_markers = ["Cargo.toml"]
command = "rust-analyzer"
```

Project config:

```toml
# ./aifed.toml
[[lsp]]
language = "rust"
file_extensions = ["rs"]
root_markers = ["Cargo.toml", "rust-project.json"]
command = "rust-analyzer"
args = ["--stdio"]
```

Result: the project definition replaces the global Rust definition for that workspace.

---

## Detection and Startup Behavior

### CLI language detection

When you run `aifed lsp ...`, the CLI maps the target file to a language using the merged `file_extensions` list.

### Daemon auto-start

When a daemon starts for a workspace, it checks merged `root_markers` to decide which language servers to start eagerly.

### On-demand start

If an LSP request targets a configured language whose server is not already running, the daemon will try to start it on demand before executing the request.

This makes custom language entries useful even when you only configure file extensions and command details.

---

## Suggested Locations

- Use `~/.config/aifed/config.toml` for machine-wide or user-wide defaults.
- Use `aifed.toml` for project-specific overrides and workarounds that should stay with the repository.

`aifed.toml` also continues to act as a workspace-root marker.

## See Also

- [LSP Integration](lsp.md) - LSP commands and behavior
- [CLI Overview](cli-overview.md) - Workspace detection
