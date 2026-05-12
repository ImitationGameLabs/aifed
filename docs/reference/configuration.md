# Configuration

File-based runtime configuration for LSP language detection and server startup.

## Current Scope

The current implementation supports:

- global config: `~/.config/aifed/config.toml` (or `$AIFED_CONFIG_DIR/config.toml`)
- project config: `aifed.toml`
- runtime merging for LSP language definitions

The current implementation does **not** yet support:

- `aifed config ...` management commands
- `aifed init`
- formatter/history/edit configuration

For now, edit the TOML files directly.

---

## Configuration Layers

Configuration is loaded in this order, with later layers replacing earlier entries that use the same `language` value:

| Priority    | Layer          | Location                                      |
| ----------- | -------------- | --------------------------------------------- |
| 1 (lowest)  | Global config  | `~/.config/aifed/config.toml` or `$AIFED_CONFIG_DIR/config.toml` |
| 2 (highest) | Project config | `aifed.toml`                                  |

On first run, aifed creates a default global config with Rust/rust-analyzer if no config file exists yet. You can edit this file or replace it entirely with your own setup.

---

## Environment Variables

| Variable            | Description                                          |
| ------------------- | ---------------------------------------------------- |
| `AIFED_CONFIG_DIR`  | Override the global config directory. The binary looks for `$AIFED_CONFIG_DIR/config.toml` instead of `~/.config/aifed/config.toml`. |

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

### Minimal Rust setup

```toml
# ~/.config/aifed/config.toml
[[lsp]]
language = "rust"
file_extensions = ["rs"]
root_markers = ["Cargo.toml"]
command = "rust-analyzer"
```

### Override the Rust server per-project

```toml
# ./aifed.toml
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

## Nix Integration

Installing via the Nix package alone behaves the same as any other installation: the default config is generated on first run. For a fully declarative setup, use the home-manager module, which symlinks a Nix-managed config to `~/.config/aifed/config.toml` via `xdg.configFile`:

```nix
programs.aifed = {
  enable = true;
  lspServers.rust = {
    language = "rust";
    command = "rust-analyzer";
    fileExtensions = [ "rs" ];
    rootMarkers = [ "Cargo.toml" ];
    displayName = "rust-analyzer";
    initializationOptions = {
      checkOnSave.command = "clippy";
      cargo.allFeatures = true;
    };
  };
};
```

---

## Suggested Locations

- Use `~/.config/aifed/config.toml` for machine-wide or user-wide defaults.
- Use `aifed.toml` for project-specific overrides and workarounds that should stay with the repository.

`aifed.toml` also continues to act as a workspace-root marker.

## See Also

- [LSP Integration](lsp.md) - LSP commands and behavior
- [CLI Overview](cli-overview.md) - Workspace detection
