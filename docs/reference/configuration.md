# Configuration

File-based runtime configuration for language/extension mapping and LSP server startup.

aifed resolves a file to a language from a **single source of truth**: a grammar-default extension table compiled into the binary (`GRAMMAR_DEFAULTS` in `crates/aifed/src/language.rs`), optionally overridden by `[[language]]` config overlays. `[[lsp]]` entries then reference a language by name to attach a server. This keeps the two concerns — *which extensions belong to a language* and *which server handles it* — decoupled.

## Current Scope

The current implementation supports:

- global config: `~/.config/aifed/config.toml` (or `$AIFED_CONFIG_DIR/config.toml`)
- project config: `aifed.toml`
- runtime merging for `[[lsp]]` and `[[language]]` entries

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

A config file has two optional array sections: `[[lsp]]` (server definitions) and `[[language]]` (extension overlays). A language can have an outline grammar without an LSP (e.g. Markdown), or an LSP without a grammar.

### `[[lsp]]` — language server

References a language **by name only**; file extensions are not declared here.

```toml
[[lsp]]
language = "rust"
root_markers = ["Cargo.toml"]
command = "rust-analyzer"
args = []
display_name = "rust-analyzer"
initialization_options = { checkOnSave = { command = "clippy" }, cargo = { allFeatures = true } }
```

| Field                    | Type           | Required | Description |
| ------------------------ | -------------- | -------- | ----------- |
| `language`               | string         | yes      | Language id; must match a language aifed can resolve (a grammar default or a `[[language]]` overlay) |
| `root_markers`           | string array   | no       | Workspace-root files that trigger daemon auto-start for the language |
| `command`                | string         | yes      | Executable used to launch the language server |
| `args`                   | string array   | no       | Extra arguments passed to the server process |
| `display_name`           | string         | no       | Human-readable server name for logs and status output |
| `initialization_options` | inline table / TOML value | no | JSON-like initialization options passed during LSP initialize |

### `[[language]]` — extension overlay

Layers on top of a language's grammar-default extensions (so you rarely need one for shipped languages like Rust):

```text
effective = (grammar_defaults ∪ additional_extensions) − exclude_extensions
```

For a language with no shipped grammar, `additional_extensions` is its full extension set — outline will resolve it but report "no outline grammar".

```toml
[[language]]
language = "markdown"
additional_extensions = ["mdown"]   # also treat .mdown as markdown
exclude_extensions = ["mdx"]        # stop treating .mdx as markdown
```

| Field                   | Type         | Required | Description |
| ----------------------- | ------------ | -------- | ----------- |
| `language`              | string       | yes      | Language id this overlay targets |
| `additional_extensions` | string array | no       | Extensions to add to the grammar defaults |
| `exclude_extensions`    | string array | no       | Default extensions to remove for this language |

### Validation Rules

- `language` must be unique within a single file's `[[lsp]]` list, and within its `[[language]]` list.
- `command` (on `[[lsp]]`) must not be empty.
- Unknown fields are rejected.
- Later config layers replace earlier entries for the same `language`, **per section**. Note: restating a `[[language]]` in the project wholesale-replaces the global one, so it resets any `exclude_extensions` you set globally — restate them if you want to keep them.

---

## Common Examples

### Minimal Rust setup

Rust's `rs` extension is a grammar default, so no `[[language]]` entry is needed:

```toml
# ~/.config/aifed/config.toml
[[lsp]]
language = "rust"
root_markers = ["Cargo.toml"]
command = "rust-analyzer"
```

### Override the Rust server per-project

```toml
# ./aifed.toml
[[lsp]]
language = "rust"
root_markers = ["Cargo.toml", "rust-project.json"]
command = "rust-analyzer"
args = ["--stdio"]
display_name = "rust-analyzer"
initialization_options = { checkOnSave = { command = "check" } }
```

### Add a custom language server

For a language aifed has no grammar for, declare it once in `[[language]]` (so the file resolves) and attach the server in `[[lsp]]`:

```toml
[[language]]
language = "nix"
additional_extensions = ["nix"]

[[lsp]]
language = "nix"
root_markers = ["flake.nix", "default.nix", "shell.nix"]
command = "nil"
display_name = "nil"
```

### Overlay a grammar language's extensions

```toml
[[language]]
language = "rust"
additional_extensions = ["rs2"]   # also treat .rs2 as rust
```

### Global defaults + project override

Global config:

```toml
# ~/.config/aifed/config.toml
[[lsp]]
language = "rust"
root_markers = ["Cargo.toml"]
command = "rust-analyzer"
```

Project config:

```toml
# ./aifed.toml
[[lsp]]
language = "rust"
root_markers = ["Cargo.toml", "rust-project.json"]
command = "rust-analyzer"
args = ["--stdio"]
```

Result: the project definition replaces the global Rust definition for that workspace.

---

## Detection and Startup Behavior

### Language resolution

When you run `aifed outline ...` or `aifed lsp ...`, aifed resolves the file's extension to a language using the grammar-default table with `[[language]]` overlays applied. Grammar languages take precedence on extension collisions; among config-only languages, the first-declared wins. Resolution is case-insensitive. With no config (or a corrupt one), outline still works using grammar defaults.

### Daemon auto-start

When a daemon starts for a workspace, it checks merged `root_markers` to decide which language servers to start eagerly. (Extension resolution is not involved on the daemon side — it receives a pre-resolved language id with each request.)

### On-demand start

If an LSP request targets a configured language whose server is not already running, the daemon will try to start it on demand before executing the request.

---

## Nix Integration

Installing via the Nix package alone behaves the same as any other installation: the default config is generated on first run. For a fully declarative setup, use the home-manager module, which symlinks a Nix-managed config to `~/.config/aifed/config.toml` via `xdg.configFile`:

```nix
programs.aifed = {
  enable = true;
  lspServers.rust = {
    language = "rust";
    command = "rust-analyzer";
    rootMarkers = [ "Cargo.toml" ];
    displayName = "rust-analyzer";
    initializationOptions = {
      checkOnSave.command = "clippy";
      cargo.allFeatures = true;
    };
  };
  # Optional: overlay grammar-default extensions (empty by default).
  languageOverlays.markdown = {
    language = "markdown";
    additionalExtensions = [ "mdown" ];
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
