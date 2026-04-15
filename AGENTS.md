# AGENTS.md

AI Agent working guide. This document provides code structure and decision rules for AI agents.

## Directory Structure

```
.
├── flake.nix                  # Flake entry point
├── crates/                    # Rust workspace members
├── docs/                      # Project documentation
│   ├── agent-action-wizards/ # Step-by-step guides for AI agents
│   └── reference/            # CLI command reference
└── nix/
      ├── common.nix   # Core config (crate paths, dependencies)
      ├── packages.nix # Flake output packages
      └── dev/
            ├── shell.nix  # Development environment
            └── checks.nix # CI checks
```

## Common Tasks

- [Add workspace member](docs/agent-action-wizards/add-workspace-member.md)
- [CLI test walkthrough](docs/agent-action-wizards/aifed-cli-test-walkthrough.md)

## Testing aifed in `.playground/`

Use `.playground/` as the general scratch workspace for testing aifed during development. This includes CLI flows, edit behavior, LSP-driven operations, and other manual verification tasks.

```bash
cd .playground
../target/debug/aifed --skill
../target/debug/aifed <command>
```

The `.playground/` directory is in `.gitignore`. The [CLI test walkthrough](docs/agent-action-wizards/aifed-cli-test-walkthrough.md) provides example test cases, but `.playground/` is not limited to that guide.

## Dependency Management

When adding dependencies to any crate:
1. Add to `[workspace.dependencies]` in root `Cargo.toml`
2. Reference in crate's `Cargo.toml` with `workspace = true`

Example:
```toml
# Root Cargo.toml
[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }

# crates/my-app/Cargo.toml
[dependencies]
serde = { workspace = true }
```

## Verification Checklist

After modifying Nix files:
- `nixfmt <nix file>` - Format single file
- `nixfmt $(find nix/ -name "*.nix") flake.nix` - Format all Nix files at once
- `statix check .` - Static analysis (run from project root)

After modifying Rust code:
- `cargo clippy` - Lint check
- `cargo fmt` - Format check
- `cargo test` - Run tests
