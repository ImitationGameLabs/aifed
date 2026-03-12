# AGENTS.md

AI Agent working guide. This document provides code structure and decision rules for AI agents.

## Directory Structure

```
.
├── flake.nix              # Flake entry point
├── crates/                # Rust workspace members
├── docs/                  # Project documentation
└── nix/
      ├── common.nix       # Core config (crate paths, dependencies)
      ├── packages.nix     # Flake output packages
      └── dev/
            ├── shell.nix  # Development environment
            └── checks.nix # CI checks
```

## Common Tasks

For adding workspace members, see [docs/add-workspace-member.md](docs/add-workspace-member.md).

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
